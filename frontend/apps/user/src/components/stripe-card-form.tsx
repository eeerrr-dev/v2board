import { useEffect, useRef } from 'react';

type StripeToken = { id: string };
type StripeChangeEvent = {
  complete: boolean;
  error?: { message?: string };
};
type StripeCardElement = {
  mount: (selector: HTMLElement) => void;
  unmount?: () => void;
  destroy?: () => void;
  on: (event: 'change', handler: (event: StripeChangeEvent) => void) => void;
};
type StripeElements = {
  create: (type: 'card', options?: Record<string, unknown>) => StripeCardElement;
};
type StripeInstance = {
  _registerWrapper?: (info: { name: string; version: string; startTime: number }) => void;
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
const STRIPE_SRC_PATTERN = /^https:\/\/js\.stripe\.com\/v3\/?(\?.*)?$/;
const STRIPE_CARD_OPTIONS = {
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
let stripeScriptPromise: Promise<void> | null = null;

function findStripeScript() {
  const scripts = document.querySelectorAll<HTMLScriptElement>(`script[src^="${STRIPE_SRC}"]`);
  for (const script of scripts) {
    if (STRIPE_SRC_PATTERN.test(script.src)) return script;
  }
  return null;
}

function loadStripeScript() {
  if (window.Stripe) return Promise.resolve();
  if (!stripeScriptPromise) {
    stripeScriptPromise = new Promise((resolve, reject) => {
      const existing = findStripeScript();
      if (existing) {
        existing.addEventListener(
          'load',
          () => {
            if (window.Stripe) resolve();
            else reject(new Error('Stripe.js not available'));
          },
          { once: true },
        );
        existing.addEventListener('error', () => reject(new Error('Failed to load Stripe.js')), {
          once: true,
        });
        return;
      }
      const script = document.createElement('script');
      script.src = STRIPE_SRC;
      script.onload = () => {
        if (window.Stripe) resolve();
        else reject(new Error('Stripe.js not available'));
      };
      script.onerror = () => reject(new Error('Failed to load Stripe.js'));
      const target = document.head || document.body;
      if (!target) {
        reject(new Error('Expected document.body not to be null. Stripe.js requires a <body> element.'));
        return;
      }
      target.appendChild(script);
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

  useEffect(() => {
    let mounted = true;
    let card: StripeCardElement | null = null;
    const startTime = Date.now();

    void loadStripeScript()
      .then(() => {
        if (!mounted || !mountRef.current || !window.Stripe) return;
        const stripe = window.Stripe(publicKey);
        stripe._registerWrapper?.({
          name: 'stripe-js',
          version: '1.38.1',
          startTime,
        });
        const elements = stripe.elements();
        card = elements.create('card', STRIPE_CARD_OPTIONS);
        card.mount(mountRef.current);
        // The original ignores the change event and calls createToken on every
        // change (no event.complete/error guard), reporting the result each time.
        card.on('change', () => {
          if (!card) return;
          void stripe.createToken(card).then((result) => {
            if (!mounted) return;
            if (result.error?.message) {
              onToken(null);
              onError?.(result.error.message);
              return;
            }
            onError?.(null);
            onToken(result.token ?? null);
          });
        });
      })
      .catch((error: Error) => {
        if (!mounted) return;
        onError?.(error.message);
      });

    return () => {
      mounted = false;
      if (card) {
        if (card.destroy) card.destroy();
        else card.unmount?.();
      }
    };
  }, [onError, onToken, publicKey]);

  return <div ref={mountRef} />;
}
