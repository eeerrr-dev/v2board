import { useCallback, useEffect, useRef, useState, type Ref } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { Order, PaymentMethod } from '@v2board/types';
import {
  useCheckoutOrderMutation,
  useCommConfig,
  useOrder,
  useOrderStatus,
  usePaymentMethods,
  useCancelOrderMutation,
  useStripePublicKey,
  useUserInfo,
  userKeys,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import type { StripeCardFormHandle } from '@/components/stripe-card-form';
import { toast } from '@/lib/toast';

export interface OrderCheckoutController {
  /** Resolved order (with a `{ plan: {} }` fallback while the detail loads). */
  order: Order;
  /** Query-level loading gate for the full-page spinner (isPending, not isFetching). */
  isLoading: boolean;
  /** The order is still awaiting payment (status 0). */
  isPending: boolean;
  paymentMethods: PaymentMethod[] | undefined;
  effectiveMethodId: number | undefined;
  selectMethod: (id: number) => void;
  isStripePayment: boolean;
  stripePublicKey: string | null;
  stripeCardRef: Ref<StripeCardFormHandle>;
  cardComplete: boolean;
  setCardComplete: (complete: boolean) => void;
  onPay: () => Promise<void>;
  isCheckoutPending: boolean;
  qrcode: { visible: boolean; payUrl: string | undefined; close: () => void };
  cancel: { run: () => void; isPending: boolean };
  /** Effective handling fee in cents (server value wins, else derived from the method). */
  fee: number;
  currencySymbol: string | undefined;
  currency: string | undefined;
}

// Authored V2Board — order checkout behavior controller. Owns the order/payment/Stripe
// queries, the self-stopping order-status poll bootstrap, the payment-settlement
// reconciliation, and the onPay/cancel orchestration; the page keeps pure rendering.
// The request/redirect payloads (save/checkout/cancel, the Stripe card token) stay
// byte-identical — only where the loading/poll lifecycle lives changes.
export function useOrderCheckoutController(tradeNo: string | undefined): OrderCheckoutController {
  const { t } = useTranslation();
  const orderQuery = useOrder(tradeNo);
  const queryClient = useQueryClient();
  // Fetch payment methods in parallel with the order instead of chaining behind it:
  // /user/order/getPaymentMethod has no data dependency on the order, so fire it while
  // the order status is still unknown (first load) or the order is pending — the states
  // that render the method list. Once the order is known-settled the list is never shown,
  // so skip the request. This removes the checkout-page request waterfall while still not
  // firing for a cached, already-settled order.
  const paymentsQuery = usePaymentMethods({
    enabled: Boolean(tradeNo) && (orderQuery.data === undefined || orderQuery.data.status === 0),
  });
  // Old componentDidMount dispatches order/detail, then user/getUserInfo, then comm/config.
  useUserInfo({ refetchOnMount: 'always' });
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const cancelMutation = useCancelOrderMutation();
  const checkout = useCheckoutOrderMutation();
  const { mutateAsync: checkoutOrder } = checkout;
  const [methodId, setMethodId] = useState<number | undefined>();
  const [qrcodeVisible, setQrcodeVisible] = useState(false);
  const [payUrl, setPayUrl] = useState<string | undefined>();
  const [pollOrderStatus, setPollOrderStatus] = useState(false);
  // Stripe tokenizes at submit time (see onPay), so the controller only tracks whether the
  // CardElement reports itself complete — enough to gate the checkout button.
  const [cardComplete, setCardComplete] = useState(false);
  const stripeCardRef = useRef<StripeCardFormHandle>(null);
  // useOrderStatus owns the 3s self-stopping poll cadence (it stops once the order leaves
  // the pending state or the check errors); this controller only decides whether to poll
  // at all, via enabled.
  const orderStatusQuery = useOrderStatus(tradeNo, { enabled: pollOrderStatus });
  const currencySymbol = comm?.currency_symbol;
  const currency = comm?.currency;
  const paymentMethods = orderQuery.data ? paymentsQuery.data : undefined;
  const hasLoadedOrder = Boolean(orderQuery.data);
  const isLoading = orderQuery.isPending;

  // The original waits 3s before starting /user/order/check, then TanStack Query owns the
  // 3s refetch cadence. Bootstrapping straight off [tradeNo, hasLoadedOrder] — with no
  // once-per-value ref guard — keeps the timer alive across React 19 StrictMode's
  // mount → cleanup → mount double-invoke: the surviving mount re-arms the timer the
  // discarded mount's cleanup cleared, where an inverted ref would have short-circuited it
  // and never started the poll for a cached order.
  useEffect(() => {
    if (!tradeNo || !hasLoadedOrder) return;
    const timer = window.setTimeout(() => setPollOrderStatus(true), 3000);
    return () => {
      window.clearTimeout(timer);
      setPollOrderStatus(false);
    };
  }, [tradeNo, hasLoadedOrder]);

  // Payment settled (gateway poll flips the order out of pending, or a free /
  // balance-covered order returns immediately): refresh the order plus the two account
  // records it just moved — balance (info) and the subscription (subscribe) it extended.
  // The original left both stale until a full reload.
  const refreshAfterPayment = useCallback(() => {
    void orderQuery.refetch();
    void queryClient.invalidateQueries({ queryKey: userKeys.info });
    void queryClient.invalidateQueries({ queryKey: userKeys.subscribe });
  }, [orderQuery.refetch, queryClient]);

  useEffect(() => {
    const status = orderStatusQuery.data;
    if (status === undefined || status === 0) return;
    setQrcodeVisible(false);
    // The original poll success only hides the QR modal; it leaves payUrl in state.
    // Manual modal cancel is the path that clears it. useOrderStatus owns stopping the
    // poll once the order leaves the pending state.
    refreshAfterPayment();
  }, [refreshAfterPayment, orderStatusQuery.data]);

  useEffect(() => {
    // The bundled poll success only hides the QR modal once the order leaves the pending
    // (status 0) state.
    if (orderQuery.data?.status !== 0) setQrcodeVisible(false);
  }, [orderQuery.data?.status]);

  const effectiveMethodId = methodId ?? paymentMethods?.[0]?.id;
  const selectedPayment = paymentMethods?.find((p) => p.id === effectiveMethodId);
  const isStripePayment = selectedPayment?.payment === 'StripeCredit';

  // The original only fetches the Stripe public key once a Stripe method is selected and
  // never refetches it, so cache it forever behind the selected method.
  const stripeQuery = useStripePublicKey(
    effectiveMethodId === undefined ? undefined : String(effectiveMethodId),
    { enabled: isStripePayment },
  );
  const stripePublicKey = stripeQuery.data ?? null;

  // pre_handling_amount from the server wins; otherwise derive the fee from the selected
  // method. The bundled poll-success refetch replaces the order detail without re-running
  // getPaymentMethod, so a paid (non-pending) order has no locally injected fee.
  const currentOrder = orderQuery.data;
  const fee = !currentOrder
    ? 0
    : (currentOrder.pre_handling_amount ??
      (currentOrder.status === 0 ? calculatePreHandlingAmount(currentOrder, selectedPayment) : 0));

  const order = (orderQuery.data ?? { plan: {} }) as Order;
  const isPending = order.status === 0;

  const onPay = async () => {
    if (!tradeNo) return;
    let token: string | undefined;
    if (isStripePayment) {
      const stripeToken = await stripeCardRef.current?.tokenize();
      if (!stripeToken) {
        toast.error(t('order.credit_card_check'));
        return;
      }
      token = stripeToken.id;
    }
    try {
      const result = await checkoutOrder({
        trade_no: tradeNo,
        method: effectiveMethodId as number,
        token,
      });
      if (isStripePayment) {
        toast.loading(t('order.stripe_verifying'), { duration: 5000 });
        return;
      }
      if (result.type === 0) {
        setQrcodeVisible(true);
        setPayUrl(typeof result.data === 'string' ? result.data : undefined);
      } else if (result.type === 1 && typeof result.data === 'string') {
        window.location.href = result.data;
        toast.info(t('order.redirecting_checkout'));
      } else if (result.type === -1) {
        // Free / balance-covered order (backend total_amount <= 0): it settles
        // immediately with no gateway, so there is no QR or redirect. Without this
        // branch onPay fell through silently. Confirm it and refresh the order + account
        // state so the result card and balance render at once.
        toast.success(t('order.success'));
        refreshAfterPayment();
      }
    } catch {
      // The mutation tracks its own error/pending state; swallow here to keep the
      // checkout button restored after a failed /payment request.
    }
  };

  const runCancel = () => {
    const cancelTradeNo = order.trade_no;
    if (!cancelTradeNo) return;
    void confirmDialog({
      title: t('common.attention'),
      description: t('order.cancel_confirm'),
      confirmText: t('order.cancel'),
      confirmButtonProps: { loading: cancelMutation.isPending },
      onConfirm: () => cancelMutation.mutateAsync(cancelTradeNo),
    });
  };

  return {
    order,
    isLoading,
    isPending,
    paymentMethods,
    effectiveMethodId,
    selectMethod: (id: number) => setMethodId(id),
    isStripePayment,
    stripePublicKey,
    stripeCardRef,
    cardComplete,
    setCardComplete,
    onPay,
    isCheckoutPending: checkout.isPending,
    qrcode: {
      visible: qrcodeVisible,
      payUrl,
      close: () => {
        setQrcodeVisible(false);
        setPayUrl(undefined);
      },
    },
    cancel: { run: runCancel, isPending: cancelMutation.isPending },
    fee,
    currencySymbol,
    currency,
  };
}

function calculatePreHandlingAmount(order: Order, method?: PaymentMethod) {
  return order.total_amount > 0 && (method?.handling_fee_fixed || method?.handling_fee_percent)
    ? order.total_amount * ((method.handling_fee_percent as number) / 100) +
        (method.handling_fee_fixed as number)
    : 0;
}
