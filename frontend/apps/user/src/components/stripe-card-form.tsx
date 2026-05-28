import { useEffect, useRef, useState } from 'react';

type StripeToken = { id: string };
type StripeChangeEvent = {
  complete: boolean;
  error?: { message?: string };
};
type StripeCardElement = {
  mount: (selector: HTMLElement) => void;
  unmount: () => void;
  destroy?: () => void;
  on: (event: 'change', handler: (event: StripeChangeEvent) => void) => void;
};
type StripeElements = {
  create: (type: 'card', options: Record<string, unknown>) => StripeCardElement;
};
type StripeInstance = {
  elements: () => StripeElements;
  createToken: (
    card: StripeCardElement,
  ) => Promise<{ token?: StripeToken; error?: { message?: string } }>;
};

declare global {
  interface Window {
    Stripe?: (publicKey: string) => StripeInstance;
  }
}

const STRIPE_SRC = 'https://js.stripe.com/v3';
let stripeScriptPromise: Promise<void> | null = null;

function loadStripeScript() {
  if (window.Stripe) return Promise.resolve();
  if (!stripeScriptPromise) {
    stripeScriptPromise = new Promise((resolve, reject) => {
      const existing = document.querySelector<HTMLScriptElement>(`script[src="${STRIPE_SRC}"]`);
      if (existing) {
        existing.addEventListener('load', () => resolve(), { once: true });
        existing.addEventListener('error', () => reject(new Error('Failed to load Stripe.js')), {
          once: true,
        });
        return;
      }
      const script = document.createElement('script');
      script.src = STRIPE_SRC;
      script.async = true;
      script.onload = () => resolve();
      script.onerror = () => reject(new Error('Failed to load Stripe.js'));
      document.body.appendChild(script);
    });
  }
  return stripeScriptPromise;
}

export function StripeCardForm({
  publicKey,
  onToken,
  onError,
}: {
  publicKey: string;
  onToken: (token: StripeToken | null) => void;
  onError?: (message: string | null) => void;
}) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let mounted = true;
    let card: StripeCardElement | null = null;

    setLoading(true);
    onToken(null);
    onError?.(null);

    void loadStripeScript()
      .then(() => {
        if (!mounted || !mountRef.current || !window.Stripe) return;
        const stripe = window.Stripe(publicKey);
        const elements = stripe.elements();
        card = elements.create('card', {
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
        });
        card.mount(mountRef.current);
        card.on('change', (event) => {
          if (event.error?.message) {
            onToken(null);
            onError?.(event.error.message);
            return;
          }
          onError?.(null);
          if (!event.complete || !card) {
            onToken(null);
            return;
          }
          void stripe.createToken(card).then((result) => {
            if (!mounted) return;
            if (result.error?.message) {
              onToken(null);
              onError?.(result.error.message);
              return;
            }
            onToken(result.token ?? null);
          });
        });
        if (mounted) setLoading(false);
      })
      .catch((error: Error) => {
        if (!mounted) return;
        setLoading(false);
        onToken(null);
        onError?.(error.message);
      });

    return () => {
      mounted = false;
      if (card) {
        card.unmount();
        card.destroy?.();
      }
    };
  }, [onError, onToken, publicKey]);

  return (
    <div className="StripeElement">
      {loading && <div className="font-size-sm text-muted">Loading...</div>}
      <div ref={mountRef} className={loading ? 'hidden' : ''} />
    </div>
  );
}
