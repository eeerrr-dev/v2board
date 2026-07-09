import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { admin } from '@v2board/api-client';
import PaymentsPage from './payments';

// The payment config surface is a redesigned shadcn island (PageHeader +
// DataTable + a Sheet editor) replacing the antd modal / drag-sort / ant-table
// replica. The DOM and source byte-pins are retired; the drag handle is swapped
// for accessible move buttons. What stays covered is the Tier-1 contract: the
// paymentMethods/paymentForm fetch, the save payload
// ({...submit, payment, config}) with the ×100 fixed-fee cents math, the enable
// and delete mutations, and the sort.mutate id-list reorder payload.

function makePayments() {
  return [
    {
      id: 1,
      uuid: 'u1',
      name: 'Alipay',
      payment: 'AlipayF2F',
      icon: null,
      handling_fee_fixed: 0,
      handling_fee_percent: 0,
      config: {},
      notify_domain: null,
      notify_url: 'https://example.com/notify/1',
      enable: 1,
      sort: 1,
      created_at: 1,
      updated_at: 1,
    },
    {
      id: 2,
      uuid: 'u2',
      name: 'Stripe',
      payment: 'StripeCheckout',
      icon: null,
      handling_fee_fixed: 0,
      handling_fee_percent: 0,
      config: {},
      notify_domain: null,
      notify_url: 'https://example.com/notify/2',
      enable: 0,
      sort: 2,
      created_at: 1,
      updated_at: 1,
    },
  ];
}

const mocks = vi.hoisted(() => ({
  data: [] as ReturnType<typeof makePayments>,
  refetch: vi.fn(),
  saveMutateAsync: vi.fn(),
  showMutate: vi.fn(),
  dropMutateAsync: vi.fn(),
  sortMutate: vi.fn(),
  confirm: vi.fn(),
}));

vi.mock('@/lib/queries', () => ({
  useAdminPayments: () => ({
    isFetching: false,
    isPending: false,
    error: undefined,
    refetch: mocks.refetch,
    data: mocks.data,
  }),
  useSavePaymentMutation: () => ({ mutateAsync: mocks.saveMutateAsync }),
  useShowPaymentMutation: () => ({ mutate: mocks.showMutate }),
  useDropPaymentMutation: () => ({ mutateAsync: mocks.dropMutateAsync }),
  useSortPaymentMutation: () => ({ mutate: mocks.sortMutate }),
}));

vi.mock('@/lib/api', () => ({ apiClient: {} }));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

vi.mock('@v2board/api-client', () => ({
  admin: {
    paymentMethods: vi.fn(),
    paymentForm: vi.fn(),
    savePayment: vi.fn(),
  },
}));

describe('PaymentsPage', () => {
  beforeEach(() => {
    mocks.data = makePayments();
    mocks.refetch.mockReset().mockResolvedValue(undefined);
    mocks.saveMutateAsync.mockReset().mockResolvedValue(undefined);
    mocks.showMutate.mockReset();
    mocks.dropMutateAsync.mockReset().mockResolvedValue(undefined);
    mocks.sortMutate.mockReset();
    mocks.confirm.mockReset().mockResolvedValue(true);
    vi.mocked(admin.paymentMethods).mockResolvedValue(['AlipayF2F', 'StripeCheckout']);
    vi.mocked(admin.paymentForm).mockResolvedValue({});
  });

  it('renders the payment rows', () => {
    render(<PaymentsPage />);
    expect(screen.getByText('Alipay')).toBeInTheDocument();
    expect(screen.getByText('AlipayF2F')).toBeInTheDocument();
    expect(screen.getByText('https://example.com/notify/1')).toBeInTheDocument();
  });

  it('toggles enable through show.mutate and refetches on success', async () => {
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getAllByRole('switch')[0]!);
    expect(mocks.showMutate).toHaveBeenCalledWith(1, expect.objectContaining({ onSuccess: expect.any(Function) }));
    mocks.showMutate.mock.calls[0]![1].onSuccess();
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('fetches the method list and dynamic form when the editor opens', async () => {
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    await waitFor(() => expect(admin.paymentMethods).toHaveBeenCalled());
    // No record → default to the first method, and its form is fetched.
    await waitFor(() =>
      expect(admin.paymentForm).toHaveBeenCalledWith({}, 'AlipayF2F', undefined),
    );
    expect(screen.getByTestId('payment-editor')).toBeInTheDocument();
  });

  it('saves with the ×100 fixed-fee cents math and the selected method + config', async () => {
    vi.mocked(admin.paymentForm).mockResolvedValue({
      float_amount: { label: '倍率', type: 'input', value: 'preset', description: '' },
    });
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    await waitFor(() => expect(screen.getByTestId('payment-editor')).toBeInTheDocument());
    // The dynamic config default falls back to the field value.
    await waitFor(() => expect(screen.getByLabelText('倍率')).toHaveValue('preset'));

    await user.type(screen.getByLabelText('固定手续费(选填)'), '2');
    await user.click(screen.getByTestId('payment-save'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith(
        expect.objectContaining({ payment: 'AlipayF2F', handling_fee_fixed: 200 }),
      ),
    );
  });

  it('reorders with sort.mutate over the new id order, then refetches', async () => {
    const user = userEvent.setup();
    render(<PaymentsPage />);

    // Move the first row (id 1) down → new order [2, 1].
    await user.click(within(screen.getByTestId('payments-table')).getAllByLabelText('下移')[0]!);
    expect(mocks.sortMutate).toHaveBeenCalledWith([2, 1], expect.objectContaining({ onSuccess: expect.any(Function) }));
    mocks.sortMutate.mock.calls[0]![1].onSuccess();
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('deletes only after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.dropMutateAsync).toHaveBeenCalledWith(1));
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('does not delete when the confirm dialog is dismissed', async () => {
    mocks.confirm.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    expect(mocks.dropMutateAsync).not.toHaveBeenCalled();
  });
});
