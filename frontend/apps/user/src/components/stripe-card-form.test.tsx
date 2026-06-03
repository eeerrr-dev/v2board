import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { StripeCardForm } from './stripe-card-form';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('StripeCardForm legacy behavior', () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    if (root) act(() => root?.unmount());
    container.remove();
    document.body.innerHTML = '';
    document
      .querySelectorAll('script[src^="https://js.stripe.com/v3"]')
      .forEach((script) => script.remove());
    delete window.Stripe;
    vi.restoreAllMocks();
  });

  it('passes the bundled theme CardElement style and tokenizes on every change', async () => {
    let onChange: ((event: { complete: boolean }) => void) | undefined;
    const card = {
      mount: vi.fn(),
      unmount: vi.fn(),
      destroy: vi.fn(),
      on: vi.fn((event: 'change', handler: (event: { complete: boolean }) => void) => {
        if (event === 'change') onChange = handler;
      }),
    };
    const create = vi.fn(() => card);
    const createToken = vi.fn().mockResolvedValue({ token: { id: 'tok_legacy' } });
    const registerWrapper = vi.fn();
    vi.spyOn(Date, 'now').mockReturnValue(1234);
    const stripe = vi.fn(() => ({
      _registerWrapper: registerWrapper,
      elements: () => ({ create }),
      createToken,
    })) as unknown as NonNullable<typeof window.Stripe>;
    window.Stripe = stripe;
    const handleToken = vi.fn();

    act(() => {
      root!.render(<StripeCardForm publicKey="pk_test" onToken={handleToken} />);
    });

    await vi.waitFor(() => expect(create).toHaveBeenCalled());
    expect(stripe).toHaveBeenCalledWith('pk_test');
    expect(registerWrapper).toHaveBeenCalledWith({
      name: 'stripe-js',
      version: '1.38.1',
      startTime: 1234,
    });
    expect(create).toHaveBeenCalledWith('card', {
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
      onChange?.({ complete: false });
      await Promise.resolve();
    });

    expect(createToken).toHaveBeenCalledWith(card);
    expect(handleToken).toHaveBeenCalledWith({ id: 'tok_legacy' });

    act(() => root?.unmount());
    root = null;

    expect(card.destroy).toHaveBeenCalledTimes(1);
    expect(card.unmount).not.toHaveBeenCalled();
  });

  it('uses the loadStripe call time as the legacy wrapper startTime', async () => {
    let onChange: ((event: { complete: boolean }) => void) | undefined;
    const card = {
      mount: vi.fn(),
      destroy: vi.fn(),
      on: vi.fn((event: 'change', handler: (event: { complete: boolean }) => void) => {
        if (event === 'change') onChange = handler;
      }),
    };
    const create = vi.fn(() => card);
    const createToken = vi.fn().mockResolvedValue({ token: { id: 'tok_delayed' } });
    const registerWrapper = vi.fn();
    let loaded = false;
    vi.spyOn(Date, 'now').mockImplementation(() => (loaded ? 5678 : 1234));
    let script: HTMLScriptElement | null = null;
    vi.spyOn(document.head, 'appendChild').mockImplementation((node: Node) => {
      script = node as HTMLScriptElement;
      return node;
    });
    const stripe = vi.fn(() => ({
      _registerWrapper: registerWrapper,
      elements: () => ({ create }),
      createToken,
    })) as unknown as NonNullable<typeof window.Stripe>;
    const handleToken = vi.fn();

    act(() => {
      root!.render(<StripeCardForm publicKey="pk_delayed" onToken={handleToken} />);
    });

    expect(script).not.toBeNull();
    expect(script!.src).toBe('https://js.stripe.com/v3');
    window.Stripe = stripe;
    loaded = true;

    await act(async () => {
      script!.dispatchEvent(new Event('load'));
      await Promise.resolve();
    });

    await vi.waitFor(() => expect(create).toHaveBeenCalled());
    expect(registerWrapper).toHaveBeenCalledWith({
      name: 'stripe-js',
      version: '1.38.1',
      startTime: 1234,
    });

    await act(async () => {
      onChange?.({ complete: false });
      await Promise.resolve();
    });

    expect(createToken).toHaveBeenCalledWith(card);
    expect(handleToken).toHaveBeenCalledWith({ id: 'tok_delayed' });
  });
});
