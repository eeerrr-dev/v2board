import type { ReactNode } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { StripeCardForm, type StripeCardFormHandle } from './stripe-card-form';

const stripeMocks = vi.hoisted(() => ({
  cardElement: { kind: 'card-element' },
  cardElementProps: null as null | {
    onChange?: (event: { complete: boolean; error?: { message: string } }) => void;
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
  CardElement: (props: {
    onChange?: (event: { complete: boolean; error?: { message: string } }) => void;
    options?: unknown;
  }) => {
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

  it('loads Stripe through the official SDK and reports completion without tokenizing on change', async () => {
    const handleComplete = vi.fn();

    act(() => {
      root!.render(<StripeCardForm publicKey="pk_test" onCompleteChange={handleComplete} />);
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

    // A CardElement change now only reports completion — it must NOT hit Stripe.
    act(() => {
      stripeMocks.cardElementProps?.onChange?.({ complete: true });
    });

    expect(handleComplete).toHaveBeenCalledWith(true);
    expect(stripeMocks.createToken).not.toHaveBeenCalled();
  });

  it('tokenizes once, on demand, when the parent calls the submit-time ref', async () => {
    const ref: { current: StripeCardFormHandle | null } = { current: null };

    act(() => {
      root!.render(<StripeCardForm publicKey="pk_test" ref={ref} />);
    });

    let token: { id: string } | null = null;
    await act(async () => {
      token = (await ref.current?.tokenize()) ?? null;
    });

    expect(stripeMocks.getElement).toHaveBeenCalled();
    expect(stripeMocks.createToken).toHaveBeenCalledTimes(1);
    expect(stripeMocks.createToken).toHaveBeenCalledWith(stripeMocks.cardElement);
    expect(token).toEqual({ id: 'tok_modern' });
  });

  it('resolves the submit-time tokenize to null when Stripe rejects the card', async () => {
    stripeMocks.createToken.mockResolvedValue({ error: { message: 'Card declined' } });
    const ref: { current: StripeCardFormHandle | null } = { current: null };

    act(() => {
      root!.render(<StripeCardForm publicKey="pk_error" ref={ref} />);
    });

    let token: { id: string } | null = { id: 'unset' };
    await act(async () => {
      token = (await ref.current?.tokenize()) ?? null;
    });

    expect(token).toBeNull();
  });
});
