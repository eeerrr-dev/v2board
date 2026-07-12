import { useImperativeHandle, useMemo, type Ref } from 'react';
import { Elements, PaymentElement, useElements, useStripe } from '@stripe/react-stripe-js';
import { loadStripe } from '@stripe/stripe-js/pure';
import type { Appearance, StripeElementsOptions } from '@stripe/stripe-js';

const PAYMENT_APPEARANCE: Appearance = {
  theme: 'stripe',
  labels: 'floating',
  variables: {
    borderRadius: '8px',
    fontFamily: 'inherit',
  },
};

type StripePromise = ReturnType<typeof loadStripe>;
const stripePromises = new Map<string, StripePromise>();

function getStripePromise(publicKey: string): StripePromise {
  const cached = stripePromises.get(publicKey);
  if (cached) return cached;
  const promise = loadStripe(publicKey);
  stripePromises.set(publicKey, promise);
  return promise;
}

export interface StripePaymentResult {
  status?: 'succeeded' | 'processing';
  error?: string;
}

export interface StripePaymentFormHandle {
  confirm: () => Promise<StripePaymentResult>;
}

interface StripePaymentFormProps {
  publicKey: string;
  clientSecret: string;
  returnUrl: string;
  onCompleteChange?: (complete: boolean) => void;
  ref?: Ref<StripePaymentFormHandle>;
}

export function StripePaymentForm({
  publicKey,
  clientSecret,
  returnUrl,
  onCompleteChange,
  ref,
}: StripePaymentFormProps) {
  // Stripe.js is initialized outside React's render lifecycle and cached once per
  // account key. This remains stable under StrictMode and supports multiple
  // operator-configured Stripe gateways without falling back to one global key.
  const stripePromise = getStripePromise(publicKey);
  const options = useMemo<StripeElementsOptions>(
    () => ({ clientSecret, appearance: PAYMENT_APPEARANCE, loader: 'auto' }),
    [clientSecret],
  );

  return (
    <Elements stripe={stripePromise} options={options}>
      <StripePaymentElement ref={ref} returnUrl={returnUrl} onCompleteChange={onCompleteChange} />
    </Elements>
  );
}

function StripePaymentElement({
  ref,
  returnUrl,
  onCompleteChange,
}: Pick<StripePaymentFormProps, 'returnUrl' | 'onCompleteChange' | 'ref'>) {
  const stripe = useStripe();
  const elements = useElements();

  useImperativeHandle(
    ref,
    () => ({
      confirm: async () => {
        if (!stripe || !elements) return { error: 'Stripe is still loading' };
        const result = await stripe.confirmPayment({
          elements,
          confirmParams: { return_url: returnUrl },
          redirect: 'if_required',
        });
        if (result.error) {
          return { error: result.error.message ?? 'Payment could not be confirmed' };
        }
        const status = result.paymentIntent.status;
        if (status === 'succeeded' || status === 'processing') {
          return { status };
        }
        return { error: `Unexpected Stripe payment status: ${status}` };
      },
    }),
    [elements, returnUrl, stripe],
  );

  return (
    <PaymentElement
      options={{ layout: { type: 'tabs', defaultCollapsed: false } }}
      onChange={(event) => onCompleteChange?.(event.complete)}
    />
  );
}
