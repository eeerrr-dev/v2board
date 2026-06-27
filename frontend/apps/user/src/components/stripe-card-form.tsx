import { useCallback, useMemo } from 'react';
import { CardElement, Elements, useElements, useStripe } from '@stripe/react-stripe-js';
import { loadStripe } from '@stripe/stripe-js/pure';
import type { StripeCardElementOptions, Token } from '@stripe/stripe-js';

type StripeToken = Pick<Token, 'id'>;

const STRIPE_CARD_OPTIONS: StripeCardElementOptions = {
  style: {
    base: {
      color: '#32325d',
      fontFamily: '"Helvetica Neue", Helvetica, sans-serif',
      fontSmoothing: 'antialiased',
      fontSize: '16px',
      '::placeholder': {
        color: '#aab7c4',
      },
    },
    invalid: {
      color: '#fa755a',
      iconColor: '#fa755a',
    },
  },
};

interface StripeCardFormProps {
  publicKey: string;
  onToken: (token: StripeToken | null) => void;
  onError?: (message: string | null) => void;
}

export function StripeCardForm({ publicKey, onToken, onError }: StripeCardFormProps) {
  const stripePromise = useMemo(() => loadStripe(publicKey), [publicKey]);

  return (
    <Elements stripe={stripePromise}>
      <StripeCardElement onError={onError} onToken={onToken} />
    </Elements>
  );
}

function StripeCardElement({
  onToken,
  onError,
}: Pick<StripeCardFormProps, 'onToken' | 'onError'>) {
  const stripe = useStripe();
  const elements = useElements();
  const tokenize = useCallback(() => {
    const card = elements?.getElement(CardElement);
    if (!stripe || !card) return;

    // Keep the legacy checkout contract: every Stripe CardElement change attempts
    // tokenization, and the parent only enables checkout once a token id exists.
    void stripe.createToken(card).then((result) => {
      if (result.error?.message) {
        onToken(null);
        onError?.(result.error.message);
        return;
      }
      onError?.(null);
      onToken(result.token ?? null);
    });
  }, [elements, onError, onToken, stripe]);

  return <CardElement options={STRIPE_CARD_OPTIONS} onChange={tokenize} />;
}
