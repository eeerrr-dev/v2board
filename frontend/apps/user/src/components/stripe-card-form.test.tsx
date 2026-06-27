import type { ReactNode } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { StripeCardForm } from './stripe-card-form';

const stripeMocks = vi.hoisted(() => ({
  cardElement: { kind: 'card-element' },
  cardElementProps: null as null | {
    onChange?: () => void;
    options?: unknown;
  },
  createToken: vi.fn(),
  elementsStripe: null as unknown,
  getElement: vi.fn(),
  loadStripe: vi.fn(),
}));

vi.mock('@stripe/stripe-js/pure', () => ({
  loadStripe: stripeMocks.loadStripe,
}));

vi.mock('@stripe/react-stripe-js', () => ({
  CardElement: (props: { onChange?: () => void; options?: unknown }) => {
    stripeMocks.cardElementProps = props;
    return <div data-testid="stripe-card-element" />;
  },
  Elements: ({ children, stripe }: { children: ReactNode; stripe: unknown }) => {
    stripeMocks.elementsStripe = stripe;
    return <div data-testid="stripe-elements">{children}</div>;
  },
  useElements: () => ({
    getElement: stripeMocks.getElement,
  }),
  useStripe: () => ({
    createToken: stripeMocks.createToken,
  }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('StripeCardForm official Stripe integration', () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    stripeMocks.cardElementProps = null;
    stripeMocks.createToken.mockReset();
    stripeMocks.createToken.mockResolvedValue({ token: { id: 'tok_modern' } });
    stripeMocks.elementsStripe = null;
    stripeMocks.getElement.mockReset();
    stripeMocks.getElement.mockReturnValue(stripeMocks.cardElement);
    stripeMocks.loadStripe.mockReset();
    stripeMocks.loadStripe.mockReturnValue(Promise.resolve({ stripe: true }));
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    if (root) act(() => root?.unmount());
    container.remove();
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('loads Stripe through the official SDK and tokenizes on every CardElement change', async () => {
    const handleToken = vi.fn();

    act(() => {
      root!.render(<StripeCardForm publicKey="pk_test" onToken={handleToken} />);
    });

    expect(stripeMocks.loadStripe).toHaveBeenCalledWith('pk_test');
    expect(stripeMocks.elementsStripe).toBe(stripeMocks.loadStripe.mock.results[0]?.value);
    expect(stripeMocks.cardElementProps?.options).toEqual({
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

    await act(async () => {
      stripeMocks.cardElementProps?.onChange?.();
      await Promise.resolve();
    });

    expect(stripeMocks.getElement).toHaveBeenCalled();
    expect(stripeMocks.createToken).toHaveBeenCalledWith(stripeMocks.cardElement);
    expect(handleToken).toHaveBeenCalledWith({ id: 'tok_modern' });
  });

  it('reports Stripe tokenization errors without enabling checkout', async () => {
    const handleToken = vi.fn();
    const handleError = vi.fn();
    stripeMocks.createToken.mockResolvedValue({ error: { message: 'Card declined' } });

    act(() => {
      root!.render(
        <StripeCardForm publicKey="pk_error" onToken={handleToken} onError={handleError} />,
      );
    });

    await act(async () => {
      stripeMocks.cardElementProps?.onChange?.();
      await Promise.resolve();
    });

    expect(handleToken).toHaveBeenCalledWith(null);
    expect(handleError).toHaveBeenCalledWith('Card declined');
  });
});
