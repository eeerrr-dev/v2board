import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PaymentFormDefinition } from '@v2board/types';
import PaymentsPage from './payments';

// The payment config surface is a redesigned shadcn island (PageHeader +
// DataTable + a Sheet editor) replacing the antd modal / drag-sort / ant-table
// replica. The DOM and source byte-pins are retired; the drag handle is swapped
// for accessible move buttons. What stays covered is the Tier-1 contract: the
// paymentMethods/paymentForm keyed query contract, the save payload
// ({...submit, payment, config}); fixed-fee cents conversion is covered at the
// api-client boundary rather than duplicated in this form. The enable
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
      enable: true,
      sort: 1,
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
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
      enable: false,
      sort: 2,
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
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
  methodsData: [] as string[] | undefined,
  methodsError: null as Error | null,
  methodsFetching: false,
  methodsPending: false,
  methodsRefetch: vi.fn(),
  paymentMethodsHook: vi.fn(),
  paymentFormHook: vi.fn(),
  definitions: {} as Record<
    string,
    {
      data: PaymentFormDefinition | undefined;
      error: Error | null;
      isError: boolean;
      isFetching: boolean;
      isPending: boolean;
      refetch: ReturnType<typeof vi.fn>;
    }
  >,
}));

vi.mock('@/lib/queries', () => ({
  useAdminPayments: () => ({
    isFetching: false,
    isPending: false,
    error: undefined,
    refetch: mocks.refetch,
    data: mocks.data,
    isError: false,
  }),
  usePaymentMethods: (enabled: boolean) => {
    mocks.paymentMethodsHook(enabled);
    return {
      data: mocks.methodsData,
      error: mocks.methodsError,
      isError: mocks.methodsError !== null,
      isFetching: mocks.methodsFetching,
      isPending: mocks.methodsPending,
      refetch: mocks.methodsRefetch,
    };
  },
  usePaymentForm: (payment: string | undefined, id: number | undefined, enabled: boolean) => {
    mocks.paymentFormHook(payment, id, enabled);
    return (
      mocks.definitions[payment ?? ''] ?? {
        data: undefined,
        error: null,
        isError: false,
        isFetching: false,
        isPending: enabled,
        refetch: vi.fn(),
      }
    );
  },
  useSavePaymentMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.saveMutateAsync(payload)).then(
        options?.onSuccess,
        () => undefined,
      );
    },
    isPending: false,
  }),
  useShowPaymentMutation: () => ({ mutate: mocks.showMutate }),
  useDropPaymentMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.dropMutateAsync(payload)).then(options?.onSuccess);
    },
  }),
  useSortPaymentMutation: () => ({ mutate: mocks.sortMutate }),
}));

vi.mock('@v2board/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

function setDefinition(
  payment: string,
  data: PaymentFormDefinition | undefined,
  state: Partial<{
    error: Error | null;
    isFetching: boolean;
    isPending: boolean;
  }> = {},
) {
  const error = state.error ?? null;
  const refetch = vi.fn();
  mocks.definitions[payment] = {
    data,
    error,
    isError: error !== null,
    isFetching: state.isFetching ?? false,
    isPending: state.isPending ?? false,
    refetch,
  };
  return mocks.definitions[payment]!;
}

describe('PaymentsPage', () => {
  beforeEach(() => {
    mocks.data = makePayments();
    mocks.refetch.mockReset().mockResolvedValue(undefined);
    mocks.saveMutateAsync.mockReset().mockResolvedValue(undefined);
    mocks.showMutate.mockReset();
    mocks.dropMutateAsync.mockReset().mockResolvedValue(undefined);
    mocks.sortMutate.mockReset();
    mocks.confirm.mockReset().mockResolvedValue(true);
    mocks.methodsData = ['AlipayF2F', 'MGate', 'StripeCheckout'];
    mocks.methodsError = null;
    mocks.methodsFetching = false;
    mocks.methodsPending = false;
    mocks.methodsRefetch.mockReset().mockResolvedValue(undefined);
    mocks.paymentMethodsHook.mockReset();
    mocks.paymentFormHook.mockReset();
    mocks.definitions = {};
    setDefinition('AlipayF2F', {
      key: { label: '支付宝密钥', type: 'input', value: 'alipay-default' },
    });
    setDefinition('MGate', {
      token: { label: 'MGate Token', type: 'input', value: 'mgate-default' },
    });
    setDefinition('StripeCheckout', {
      secret_key: { label: 'Stripe Secret Key', type: 'input', value: 'sk-default' },
    });
  });

  it('renders the payment rows', () => {
    render(<PaymentsPage />);
    expect(screen.getByText('Alipay')).toBeInTheDocument();
    expect(screen.getByText('AlipayF2F')).toBeInTheDocument();
    expect(screen.getByText('https://example.com/notify/1')).toBeInTheDocument();
  });

  it('toggles enable through the query-layer invalidating mutation', async () => {
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getAllByRole('switch')[0]!);
    expect(mocks.showMutate).toHaveBeenCalledWith({ id: 1, enable: false });
  });

  it('opens immediately and enables the keyed method and form queries', async () => {
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    expect(screen.getByTestId('payment-editor')).toBeInTheDocument();
    await waitFor(() => expect(mocks.paymentMethodsHook).toHaveBeenCalledWith(true));
    // No record defaults to the first method; the query key includes driver + id.
    await waitFor(() =>
      expect(mocks.paymentFormHook).toHaveBeenCalledWith('AlipayF2F', undefined, true),
    );
  });

  it('keeps method loading, failure retry, and empty states inside the open editor', async () => {
    mocks.methodsData = undefined;
    mocks.methodsPending = true;
    const user = userEvent.setup();
    const { rerender } = render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    expect(screen.getByTestId('payment-editor')).toBeInTheDocument();
    expect(screen.getByTestId('payment-methods-loading')).toBeInTheDocument();
    expect(screen.getByTestId('payment-save')).toBeDisabled();

    mocks.methodsPending = false;
    mocks.methodsError = new Error('methods failed');
    rerender(<PaymentsPage />);
    const error = screen.getByTestId('payment-methods-error');
    await user.click(within(error).getByTestId('error-state-retry'));
    expect(mocks.methodsRefetch).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId('payment-save')).toBeDisabled();

    mocks.methodsError = null;
    mocks.methodsData = [];
    rerender(<PaymentsPage />);
    expect(screen.getByTestId('payment-methods-empty')).toHaveTextContent('暂无可用支付接口');
    expect(screen.getByTestId('payment-save')).toBeDisabled();
  });

  it('shows a retryable definition failure and blocks empty definitions', async () => {
    const failed = setDefinition('AlipayF2F', undefined, {
      error: new Error('definition failed'),
    });
    const user = userEvent.setup();
    const { rerender } = render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    const error = await screen.findByTestId('payment-definition-error');
    expect(screen.getByTestId('payment-save')).toBeDisabled();
    await user.click(within(error).getByTestId('error-state-retry'));
    expect(failed.refetch).toHaveBeenCalledTimes(1);

    setDefinition('AlipayF2F', {});
    rerender(<PaymentsPage />);
    expect(screen.getByTestId('payment-definition-empty')).toHaveTextContent(
      '该支付接口未提供配置字段',
    );
    expect(screen.getByTestId('payment-save')).toBeDisabled();
  });

  it('saves the decimal fixed fee and selected method + config for api-client serialization', async () => {
    setDefinition('AlipayF2F', {
      float_amount: { label: '倍率', type: 'input', value: 'preset', description: '' },
    });
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    await waitFor(() => expect(screen.getByTestId('payment-editor')).toBeInTheDocument());
    // The dynamic config default falls back to the field value.
    await waitFor(() => expect(screen.getByLabelText('倍率')).toHaveValue('preset'));

    await user.type(screen.getByLabelText('显示名称'), '支付宝');
    await user.type(screen.getByLabelText('固定手续费(选填)'), '2');
    await user.click(screen.getByTestId('payment-save'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith(
        expect.objectContaining({ payment: 'AlipayF2F', handling_fee_fixed: '2' }),
      ),
    );
  });

  it('submits only the selected driver config after switching Alipay through MGate to Stripe', async () => {
    setDefinition('AlipayF2F', {
      key: {
        label: '支付宝密钥',
        type: 'input',
        value: 'alipay-default',
      },
      mch_id: {
        label: '支付宝商户 ID',
        type: 'input',
        value: 'merchant-default',
      },
    });
    setDefinition('MGate', {
      token: {
        label: 'MGate Token',
        type: 'input',
        value: 'mgate-default',
      },
    });
    setDefinition('StripeCheckout', {
      publishable_key: {
        label: 'Stripe Publishable Key',
        type: 'input',
        value: 'pk-default',
      },
      secret_key: {
        label: 'Stripe Secret Key',
        type: 'input',
        value: 'sk-default',
      },
    });
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    await waitFor(() => expect(screen.getByLabelText('支付宝密钥')).toHaveValue('alipay-default'));
    await user.click(screen.getByLabelText('接口文件'));
    await user.click(await screen.findByRole('option', { name: 'MGate' }));
    await waitFor(() => expect(screen.getByLabelText('MGate Token')).toHaveValue('mgate-default'));
    expect(screen.queryByLabelText('支付宝密钥')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('支付宝商户 ID')).not.toBeInTheDocument();
    await user.clear(screen.getByLabelText('MGate Token'));
    await user.type(screen.getByLabelText('MGate Token'), 'mgate-user');

    await user.click(screen.getByLabelText('接口文件'));
    await user.click(await screen.findByRole('option', { name: 'StripeCheckout' }));
    await waitFor(() =>
      expect(screen.getByLabelText('Stripe Secret Key')).toHaveValue('sk-default'),
    );
    expect(screen.queryByLabelText('支付宝密钥')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('支付宝商户 ID')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('MGate Token')).not.toBeInTheDocument();

    await user.type(screen.getByLabelText('显示名称'), 'Stripe');
    await user.clear(screen.getByLabelText('Stripe Publishable Key'));
    await user.type(screen.getByLabelText('Stripe Publishable Key'), 'pk-user');
    await user.clear(screen.getByLabelText('Stripe Secret Key'));
    await user.type(screen.getByLabelText('Stripe Secret Key'), 'sk-user');
    await user.click(screen.getByTestId('payment-save'));

    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalled());
    const payload = mocks.saveMutateAsync.mock.calls[0]?.[0];
    expect(payload).toEqual(
      expect.objectContaining({
        payment: 'StripeCheckout',
        config: { publishable_key: 'pk-user', secret_key: 'sk-user' },
      }),
    );
    expect(payload.config).not.toHaveProperty('token');
    expect(payload.config).not.toHaveProperty('key');
    expect(payload.config).not.toHaveProperty('mch_id');
  });

  it('ignores a late definition after rapidly switching to another keyed driver', async () => {
    setDefinition('MGate', undefined, { isFetching: true, isPending: true });
    setDefinition('StripeCheckout', {
      secret_key: { label: 'Stripe Secret Key', type: 'input', value: 'stripe-current' },
    });
    const user = userEvent.setup();
    const { rerender } = render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    await screen.findByLabelText('支付宝密钥');

    await user.click(screen.getByLabelText('接口文件'));
    await user.click(await screen.findByRole('option', { name: 'MGate' }));
    expect(await screen.findByTestId('payment-definition-loading')).toBeInTheDocument();

    await user.click(screen.getByLabelText('接口文件'));
    await user.click(await screen.findByRole('option', { name: 'StripeCheckout' }));
    expect(await screen.findByLabelText('Stripe Secret Key')).toHaveValue('stripe-current');

    const lateMGate = mocks.definitions.MGate!;
    lateMGate.data = {
      token: { label: 'Late MGate Token', type: 'input', value: 'must-not-leak' },
    };
    lateMGate.isFetching = false;
    lateMGate.isPending = false;
    rerender(<PaymentsPage />);

    expect(screen.queryByLabelText('Late MGate Token')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Stripe Secret Key')).toHaveValue('stripe-current');

    await user.type(screen.getByLabelText('显示名称'), 'Stripe keyed');
    await user.click(screen.getByTestId('payment-save'));
    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalled());
    expect(mocks.saveMutateAsync.mock.calls[0]?.[0]).toEqual(
      expect.objectContaining({
        payment: 'StripeCheckout',
        config: { secret_key: 'stripe-current' },
      }),
    );
  });

  it('preserves same-driver edits but restores the record config after switching away and back', async () => {
    mocks.data[0] = {
      ...mocks.data[0]!,
      config: { key: 'record-key' },
    };
    const user = userEvent.setup();
    const { rerender } = render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-edit-1'));
    const keyInput = await screen.findByLabelText('支付宝密钥');
    expect(keyInput).toHaveValue('record-key');
    await user.clear(keyInput);
    await user.type(keyInput, 'same-driver-edit');

    // A same-key query refresh re-applies the definition without replacing the
    // user's current config value with its default.
    setDefinition('AlipayF2F', {
      key: { label: '支付宝密钥', type: 'input', value: 'new-default' },
    });
    rerender(<PaymentsPage />);
    expect(screen.getByLabelText('支付宝密钥')).toHaveValue('same-driver-edit');

    await user.click(screen.getByLabelText('接口文件'));
    await user.click(await screen.findByRole('option', { name: 'MGate' }));
    await screen.findByLabelText('MGate Token');
    await user.click(screen.getByLabelText('接口文件'));
    await user.click(await screen.findByRole('option', { name: 'AlipayF2F' }));
    expect(await screen.findByLabelText('支付宝密钥')).toHaveValue('record-key');
  });

  it('keeps the configured editor open when the save request fails', async () => {
    setDefinition('AlipayF2F', {
      secret: { label: '密钥', type: 'input', value: 'configured' },
    });
    mocks.saveMutateAsync.mockRejectedValueOnce(new Error('save failed'));
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    await waitFor(() => expect(screen.getByLabelText('密钥')).toHaveValue('configured'));
    await user.type(screen.getByLabelText('显示名称'), '失败后保留');
    await user.click(screen.getByTestId('payment-save'));

    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledOnce());
    expect(screen.getByTestId('payment-editor')).toBeInTheDocument();
    expect(screen.getByLabelText('显示名称')).toHaveValue('失败后保留');
    expect(screen.getByLabelText('密钥')).toHaveValue('configured');
  });

  it('blocks fields that the payment backend would reject', async () => {
    setDefinition('AlipayF2F', {
      secret: { label: '密钥', type: 'input', value: 'configured' },
    });
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-create'));
    await waitFor(() => expect(screen.getByLabelText('密钥')).toHaveValue('configured'));
    await user.type(screen.getByLabelText('自定义通知域名(选填)'), 'not-a-url');
    await user.type(screen.getByLabelText('百分比手续费(选填)'), '0');
    await user.click(screen.getByTestId('payment-save'));

    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    expect(await screen.findByText('显示名称不能为空')).toBeInTheDocument();
    expect(screen.getByText('请输入有效的 HTTP(S) 通知域名')).toBeInTheDocument();
    expect(screen.getByText('百分比手续费范围须在 0.1 到 100 之间')).toBeInTheDocument();
  });

  it('renders a legacy zero percentage fee as blank for API-boundary normalization', async () => {
    setDefinition('AlipayF2F', {
      key: { label: '密钥', type: 'input', value: 'configured' },
    });
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-edit-1'));
    await waitFor(() => expect(screen.getByLabelText('密钥')).toHaveValue('configured'));
    expect(screen.getByLabelText('百分比手续费(选填)')).toHaveDisplayValue('');
    await user.click(screen.getByTestId('payment-save'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith(
        expect.objectContaining({ id: 1, handling_fee_percent: '' }),
      ),
    );
  });

  it('reorders with sort.mutate over the new id order', async () => {
    const user = userEvent.setup();
    render(<PaymentsPage />);

    // Move the first row (id 1) down → new order [2, 1].
    await user.click(within(screen.getByTestId('payments-table')).getAllByLabelText('下移')[0]!);
    expect(mocks.sortMutate).toHaveBeenCalledWith(
      [2, 1],
      expect.objectContaining({ onSettled: expect.any(Function) }),
    );
  });

  it('deletes only after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<PaymentsPage />);

    await user.click(screen.getByTestId('payment-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.dropMutateAsync).toHaveBeenCalledWith(1));
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
