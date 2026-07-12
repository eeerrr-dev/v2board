import type { ReactNode } from 'react';
import { screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { StripePaymentForm, type StripePaymentFormHandle } from './stripe-payment-form';

const stripeMocks = vi.hoisted(() => ({
  confirmPayment: vi.fn(),
  elements: { kind: 'elements' },
  elementsOptions: null as unknown,
  elementsStripe: null as unknown,
  loadStripe: vi.fn(),
  paymentElementProps: null as null | {
    onChange?: (event: { complete: boolean }) => void;
    options?: unknown;
  },
}));

vi.mock('@stripe/stripe-js/pure', () => ({ loadStripe: stripeMocks.loadStripe }));

vi.mock('@stripe/react-stripe-js', () => ({
  Elements: ({
    children,
    stripe,
    options,
  }: {
    children: ReactNode;
    stripe: unknown;
    options: unknown;
  }) => {
    stripeMocks.elementsStripe = stripe;
    stripeMocks.elementsOptions = options;
    return <div data-testid="stripe-elements">{children}</div>;
  },
  PaymentElement: (props: {
    onChange?: (event: { complete: boolean }) => void;
    options?: unknown;
  }) => {
    stripeMocks.paymentElementProps = props;
    return <div data-testid="stripe-payment-element" />;
  },
  useElements: () => stripeMocks.elements,
  useStripe: () => ({ confirmPayment: stripeMocks.confirmPayment }),
}));

describe('StripePaymentForm', () => {
  beforeEach(() => {
    stripeMocks.confirmPayment.mockReset();
    stripeMocks.elementsOptions = null;
    stripeMocks.elementsStripe = null;
    stripeMocks.loadStripe.mockReset();
    stripeMocks.loadStripe.mockReturnValue(Promise.resolve({ stripe: true }));
    stripeMocks.paymentElementProps = null;
  });

  it('mounts Payment Element with the server-owned client secret', () => {
    const onComplete = vi.fn();
    renderWithProviders(
      <StripePaymentForm
        publicKey="pk_test"
        clientSecret="pi_test_secret_abc"
        returnUrl="https://example.test/#/order/T1"
        onCompleteChange={onComplete}
      />,
    );

    expect(screen.getByTestId('stripe-payment-element')).toBeInTheDocument();
    expect(stripeMocks.loadStripe).toHaveBeenCalledWith('pk_test');
    expect(stripeMocks.elementsStripe).toBe(stripeMocks.loadStripe.mock.results[0]?.value);
    expect(stripeMocks.elementsOptions).toMatchObject({
      clientSecret: 'pi_test_secret_abc',
      loader: 'auto',
    });

    stripeMocks.paymentElementProps?.onChange?.({ complete: true });
    expect(onComplete).toHaveBeenCalledWith(true);
  });

  it('initializes Stripe.js only once per publishable key across rerenders', () => {
    const view = renderWithProviders(
      <StripePaymentForm
        publicKey="pk_cached"
        clientSecret="pi_test_secret_first"
        returnUrl="https://example.test/#/order/T1"
      />,
    );

    view.rerender(
      <StripePaymentForm
        publicKey="pk_cached"
        clientSecret="pi_test_secret_second"
        returnUrl="https://example.test/#/order/T1"
      />,
    );
    expect(stripeMocks.loadStripe).toHaveBeenCalledTimes(1);

    view.rerender(
      <StripePaymentForm
        publicKey="pk_second_account"
        clientSecret="pi_test_secret_third"
        returnUrl="https://example.test/#/order/T1"
      />,
    );
    expect(stripeMocks.loadStripe).toHaveBeenCalledTimes(2);
    expect(stripeMocks.loadStripe).toHaveBeenLastCalledWith('pk_second_account');
  });

  it('confirms the PaymentIntent without creating a legacy card token', async () => {
    stripeMocks.confirmPayment.mockResolvedValue({
      paymentIntent: { status: 'succeeded' },
    });
    const ref: { current: StripePaymentFormHandle | null } = { current: null };
    renderWithProviders(
      <StripePaymentForm
        publicKey="pk_test"
        clientSecret="pi_test_secret_abc"
        returnUrl="https://example.test/#/order/T1"
        ref={ref}
      />,
    );

    await expect(ref.current!.confirm()).resolves.toEqual({ status: 'succeeded' });
    expect(stripeMocks.confirmPayment).toHaveBeenCalledWith({
      elements: stripeMocks.elements,
      confirmParams: { return_url: 'https://example.test/#/order/T1' },
      redirect: 'if_required',
    });
  });

  it('returns Stripe validation errors to the checkout controller', async () => {
    stripeMocks.confirmPayment.mockResolvedValue({ error: { message: 'Card declined' } });
    const ref: { current: StripePaymentFormHandle | null } = { current: null };
    renderWithProviders(
      <StripePaymentForm
        publicKey="pk_test"
        clientSecret="pi_test_secret_abc"
        returnUrl="https://example.test/#/order/T1"
        ref={ref}
      />,
    );

    await expect(ref.current!.confirm()).resolves.toEqual({ error: 'Card declined' });
  });

  it('does not treat an uncaptured intent as settled', async () => {
    stripeMocks.confirmPayment.mockResolvedValue({
      paymentIntent: { status: 'requires_capture' },
    });
    const ref: { current: StripePaymentFormHandle | null } = { current: null };
    renderWithProviders(
      <StripePaymentForm
        publicKey="pk_test"
        clientSecret="pi_test_secret_abc"
        returnUrl="https://example.test/#/order/T1"
        ref={ref}
      />,
    );

    await expect(ref.current!.confirm()).resolves.toEqual({
      error: 'Unexpected Stripe payment status: requires_capture',
    });
  });
});
