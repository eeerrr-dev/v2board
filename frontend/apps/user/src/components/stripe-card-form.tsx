import { useImperativeHandle, useMemo, type Ref } from 'react';
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

export interface StripeCardFormHandle {
  /**
   * Tokenize the current card once, at submit time. Resolves to null when Stripe
   * has not loaded yet or the card is invalid/declined so the caller can surface
   * the error and abort before hitting /payment.
   */
  tokenize: () => Promise<StripeToken | null>;
}

interface StripeCardFormProps {
  publicKey: string;
  /** Fires with the CardElement's `complete` flag so the parent can gate checkout. */
  onCompleteChange?: (complete: boolean) => void;
  ref?: Ref<StripeCardFormHandle>;
}

export function StripeCardForm({ publicKey, onCompleteChange, ref }: StripeCardFormProps) {
  const stripePromise = useMemo(() => loadStripe(publicKey), [publicKey]);

  return (
    <Elements stripe={stripePromise}>
      <StripeCardElement ref={ref} onCompleteChange={onCompleteChange} />
    </Elements>
  );
}

function StripeCardElement({
  ref,
  onCompleteChange,
}: Pick<StripeCardFormProps, 'onCompleteChange' | 'ref'>) {
  const stripe = useStripe();
  const elements = useElements();

  // Tokenize on demand instead of on every keystroke: onPay awaits this once, right
  // before /payment. The token id handed to checkout is byte-identical to the legacy
  // createToken().token.id contract — only the trigger point moves to submit.
  useImperativeHandle(
    ref,
    () => ({
      tokenize: async () => {
        const card = elements?.getElement(CardElement);
        if (!stripe || !card) return null;
        const result = await stripe.createToken(card);
        if (result.error) return null;
        return result.token ?? null;
      },
    }),
    [elements, stripe],
  );

  return (
    <CardElement
      options={STRIPE_CARD_OPTIONS}
      onChange={(event) => onCompleteChange?.(event.complete)}
    />
  );
}
