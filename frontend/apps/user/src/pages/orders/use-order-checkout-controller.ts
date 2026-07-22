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
  useStripePaymentIntent,
  useUserInfo,
  userKeys,
} from '@/lib/queries';
import { confirmDialog } from '@v2board/ui/confirm-dialog';
import type { StripePaymentFormHandle } from '@/components/stripe-payment-form';
import { toast } from '@v2board/app-shell/toast';

export interface OrderCheckoutController {
  /** Resolved order; absent while loading or after a failed detail request. */
  order: Order | undefined;
  /** Query-level loading gate for the full-page spinner (isPending, not isFetching). */
  isLoading: boolean;
  orderError: string | null;
  retryOrder: () => void;
  /** The order is still awaiting payment (status 0). */
  isPending: boolean;
  paymentMethods: PaymentMethod[];
  paymentMethodsState: {
    isPending: boolean;
    error: string | null;
    isEmpty: boolean;
    retry: () => void;
  };
  effectiveMethodId: number | undefined;
  /** True only when the effective method still exists in the current query result. */
  canCheckout: boolean;
  selectMethod: (id: number) => void;
  isStripePayment: boolean;
  stripePaymentIntent: {
    public_key: string;
    client_secret: string;
    amount: number;
    currency: string;
  } | null;
  stripePreparation: {
    isPending: boolean;
    error: string | null;
    retry: () => void;
  };
  stripePaymentRef: Ref<StripePaymentFormHandle>;
  paymentComplete: boolean;
  setPaymentComplete: (complete: boolean) => void;
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
// Stripe uses a server-owned PaymentIntent and Payment Element; no card token ever
// passes through application code.
export function useOrderCheckoutController(tradeNo: string | undefined): OrderCheckoutController {
  const { t } = useTranslation();
  const {
    data: order,
    error: orderQueryError,
    isPending: isOrderPending,
    refetch: refetchOrder,
  } = useOrder(tradeNo);
  const queryClient = useQueryClient();
  // Fetch payment methods in parallel with the order instead of chaining behind it:
  // /user/order/getPaymentMethod has no data dependency on the order, so fire it while
  // the order status is still unknown (first load) or the order is pending — the states
  // that render the method list. Once the order is known-settled the list is never shown,
  // so skip the request. This removes the checkout-page request waterfall while still not
  // firing for a cached, already-settled order.
  const paymentsQuery = usePaymentMethods({
    enabled: Boolean(tradeNo) && (order === undefined || order.status === 0),
  });
  // Old componentDidMount dispatches order/detail, then user/getUserInfo, then comm/config.
  useUserInfo({ refetchOnMount: 'always' });
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const cancelMutation = useCancelOrderMutation();
  const checkout = useCheckoutOrderMutation();
  const [methodId, setMethodId] = useState<number | undefined>();
  const [qrcodeRequested, setQrcodeRequested] = useState(false);
  const [payUrl, setPayUrl] = useState<string | undefined>();
  const [pollTradeNo, setPollTradeNo] = useState<string | null>(null);
  const [completedStripeIntent, setCompletedStripeIntent] = useState<string | null>(null);
  const [stripeConfirming, setStripeConfirming] = useState(false);
  const stripeConfirmingRef = useRef(false);
  const stripePaymentRef = useRef<StripePaymentFormHandle>(null);
  // useOrderStatus owns the 3s self-stopping poll cadence (it stops once the order leaves
  // the pending state or the check errors); this controller only decides whether to poll
  // at all, via enabled.
  const pollOrderStatus = Boolean(tradeNo && order && pollTradeNo === tradeNo);
  const orderStatusQuery = useOrderStatus(tradeNo, { enabled: pollOrderStatus });
  const currencySymbol = comm?.currency_symbol;
  const currency = comm?.currency;
  const paymentMethods = order ? (paymentsQuery.data ?? []) : [];
  const paymentMethodsError =
    paymentsQuery.error instanceof Error
      ? paymentsQuery.error.message || t(($) => $.common.error_title)
      : paymentsQuery.error
        ? t(($) => $.common.error_title)
        : null;
  const hasLoadedOrder = Boolean(order);
  const isLoading = isOrderPending;

  // The original waits 3s before starting /user/order/check, then TanStack Query owns the
  // 3s refetch cadence. Bootstrapping straight off [tradeNo, hasLoadedOrder] — with no
  // once-per-value ref guard — keeps the timer alive across React 19 StrictMode's
  // mount → cleanup → mount double-invoke: the surviving mount re-arms the timer the
  // discarded mount's cleanup cleared, where an inverted ref would have short-circuited it
  // and never started the poll for a cached order.
  useEffect(() => {
    if (!tradeNo || !hasLoadedOrder) return;
    const timer = window.setTimeout(() => setPollTradeNo(tradeNo), 3000);
    return () => window.clearTimeout(timer);
  }, [tradeNo, hasLoadedOrder]);

  // Payment settled (gateway poll flips the order out of pending, or a free /
  // balance-covered order returns immediately): refresh the order plus the two account
  // records it just moved — balance (info) and the subscription (subscribe) it extended.
  // The original left both stale until a full reload.
  const refreshAfterPayment = useCallback(() => {
    void refetchOrder();
    void queryClient.invalidateQueries({ queryKey: userKeys.info });
    void queryClient.invalidateQueries({ queryKey: userKeys.subscribe });
  }, [queryClient, refetchOrder]);

  useEffect(() => {
    const status = orderStatusQuery.data;
    if (status === undefined || status === 0) return;
    // The original poll success only hides the QR modal; it leaves payUrl in state.
    // Manual modal cancel is the path that clears it. useOrderStatus owns stopping the
    // poll once the order leaves the pending state.
    refreshAfterPayment();
  }, [refreshAfterPayment, orderStatusQuery.data]);

  const isPending = order?.status === 0;
  const qrcodeVisible = Boolean(qrcodeRequested && isPending && (orderStatusQuery.data ?? 0) === 0);

  // A selected gateway can disappear after an operator configuration refresh. Resolve the
  // effective method from the current query result on every render, falling back to the
  // first current method instead of retaining a now-invalid id.
  const selectedPayment =
    paymentMethods.find((payment) => payment.id === methodId) ?? paymentMethods[0];
  const effectiveMethodId = selectedPayment?.id;
  const canCheckout = Boolean(
    tradeNo &&
    order &&
    isPending &&
    selectedPayment &&
    !paymentsQuery.isPending &&
    !paymentMethodsError,
  );
  // A balance-covered/free order must go through the ordinary checkout endpoint,
  // which performs the immediate server-side settlement before consulting a
  // gateway. It neither needs nor can create a positive-amount PaymentIntent.
  const isStripePayment = Boolean(
    canCheckout && order && selectedPayment?.payment === 'StripeCredit' && order.total_amount > 0,
  );

  // Preparing is idempotent on the server and reuses the order's PaymentIntent. It starts
  // when Stripe becomes the effective method so Payment Element is ready before submit.
  const stripeIntentQuery = useStripePaymentIntent(tradeNo, effectiveMethodId, {
    enabled: isStripePayment,
  });
  // TanStack Query intentionally retains the previous successful data when a
  // refetch fails. A client secret is not ordinary display data: once another
  // method has superseded it, confirming it can charge an intent the order will
  // refuse to settle. Only expose a fully current, idle query result.
  const stripePaymentIntent =
    canCheckout && !stripeIntentQuery.isFetching && !stripeIntentQuery.error
      ? (stripeIntentQuery.data ?? null)
      : null;
  const paymentComplete =
    stripePaymentIntent !== null && completedStripeIntent === stripePaymentIntent.client_secret;
  const stripeClientSecret = stripePaymentIntent?.client_secret ?? null;
  const setPaymentComplete = useCallback(
    (complete: boolean) => {
      setCompletedStripeIntent(complete ? stripeClientSecret : null);
    },
    [stripeClientSecret],
  );

  // The handling fee is a Tier-2 display estimate derived from the selected
  // method; the server's settled `handling_amount` is what actually charges.
  // The bundled poll-success refetch replaces the order detail without re-running
  // getPaymentMethod, so a paid (non-pending) order has no locally injected fee.
  const fee = order?.status === 0 ? calculatePreHandlingAmount(order, selectedPayment) : 0;

  const onPay = async () => {
    // Never let an empty/loading/failed or stale payment-method result reach the checkout
    // endpoint. This also narrows effectiveMethodId to number without a type assertion.
    if (!tradeNo || !order || !canCheckout || effectiveMethodId === undefined) return;
    if (isStripePayment) {
      if (stripeConfirmingRef.current) return;
      stripeConfirmingRef.current = true;
      setStripeConfirming(true);
      try {
        const result = await stripePaymentRef.current?.confirm();
        if (!result || result.error) {
          toast.error(result?.error ?? t(($) => $.order.credit_card_check));
          return;
        }
        setPollTradeNo(tradeNo);
        toast.loading(
          t(($) => $.order.stripe_verifying),
          { duration: 5000 },
        );
      } catch (error) {
        toast.error(error instanceof Error ? error.message : t(($) => $.common.error_title));
      } finally {
        stripeConfirmingRef.current = false;
        setStripeConfirming(false);
      }
      return;
    }
    checkout.mutate(
      {
        trade_no: tradeNo,
        method_id: effectiveMethodId,
      },
      {
        onSuccess: (result) => {
          // §9.3: the checkout result is a closed discriminated union; an
          // unmappable gateway response is a 400 payment_gateway_unsupported
          // problem surfaced through the shared MutationCache error path.
          switch (result.kind) {
            case 'qr_code':
              setQrcodeRequested(true);
              setPayUrl(result.payload);
              break;
            case 'redirect':
              window.location.href = result.url;
              toast.info(t(($) => $.order.redirecting_checkout));
              break;
            case 'settled':
              // Free / balance-covered order (backend total_amount <= 0): it settles
              // immediately with no gateway, so there is no QR or redirect. Confirm it
              // and refresh the order + account state so the result card and balance
              // render at once.
              toast.success(t(($) => $.order.success));
              refreshAfterPayment();
              break;
          }
        },
      },
    );
  };

  const runCancel = () => {
    const cancelTradeNo = order?.trade_no;
    if (!cancelTradeNo) return;
    void confirmDialog({
      title: t(($) => $.common.attention),
      description: t(($) => $.order.cancel_confirm),
      confirmText: t(($) => $.order.cancel),
      confirmButtonProps: { loading: cancelMutation.isPending },
      onConfirm: () => cancelMutation.mutateAsync(cancelTradeNo),
    });
  };

  return {
    order,
    isLoading,
    orderError: orderQueryError instanceof Error ? orderQueryError.message : null,
    retryOrder: () => {
      void refetchOrder();
    },
    isPending,
    paymentMethods,
    paymentMethodsState: {
      isPending: paymentsQuery.isPending,
      error: paymentMethodsError,
      isEmpty: !paymentsQuery.isPending && !paymentMethodsError && paymentMethods.length === 0,
      retry: () => {
        void paymentsQuery.refetch();
      },
    },
    effectiveMethodId,
    canCheckout,
    selectMethod: (id: number) => {
      setMethodId(id);
      // Payment Element remounts when a method is revisited; its previous
      // completeness signal is no longer evidence that the new form is complete.
      setCompletedStripeIntent(null);
    },
    isStripePayment,
    stripePaymentIntent,
    stripePreparation: {
      isPending: stripeIntentQuery.isPending,
      error: stripeIntentQuery.error instanceof Error ? stripeIntentQuery.error.message : null,
      retry: () => {
        void stripeIntentQuery.refetch();
      },
    },
    stripePaymentRef,
    paymentComplete,
    setPaymentComplete,
    onPay,
    isCheckoutPending: checkout.isPending || stripeIntentQuery.isFetching || stripeConfirming,
    qrcode: {
      visible: qrcodeVisible,
      payUrl,
      close: () => {
        setQrcodeRequested(false);
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
  if (order.total_amount <= 0) return 0;
  const percent = method?.handling_fee_percent ?? 0;
  const fixed = method?.handling_fee_fixed ?? 0;
  return order.total_amount * (percent / 100) + fixed;
}
