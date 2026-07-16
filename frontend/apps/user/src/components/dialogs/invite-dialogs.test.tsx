import type { ReactNode } from 'react';
import { screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { createTestTranslation } from '@/test/i18next-selector';
import { TransferDialog } from './transfer-dialog';
import { WithdrawDialog } from './withdraw-dialog';
import type * as RuntimeConfigModule from '@/lib/runtime-config';

const mocks = vi.hoisted(() => ({
  invalidateQueries: vi.fn(),
  navigate: vi.fn(),
  transferMutateAsync: vi.fn(),
  withdrawMutateAsync: vi.fn(),
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({
    invalidateQueries: mocks.invalidateQueries,
  }),
}));

vi.mock('react-router', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () =>
    createTestTranslation({
        'common.cancel': '取消',
        'dashboard.transfer_to_balance': '推广佣金划转至余额',
        'invite.current_commission_balance': '当前推广佣金余额',
        'invite.transfer': '划转',
        'invite.transfer_amount': '划转金额',
        'invite.transfer_notice': '划转后的余额仅用于{title}消费使用',
        'invite.transfer_placeholder': '请输入需要划转到余额的金额',
        'invite.transfer_invalid': '请输入有效的划转金额',
        'invite.transfer_decimals': '划转金额最多支持两位小数',
        'invite.transfer_exceeds': '划转金额不能超过当前推广佣金余额',
        'invite.withdraw': '申请提现',
        'invite.withdraw_account': '提现账号',
        'invite.withdraw_account_placeholder': '请输入提现账号',
        'invite.withdraw_button': '推广佣金提现',
        'invite.withdraw_method': '提现方式',
        'invite.withdraw_method_placeholder': '请选择提现方式',
        'invite.withdraw_submit': '确认',
        'profile.confirm': '确认',
    }),
}));

vi.mock('@/lib/runtime-config', async (importOriginal) => ({
  ...(await importOriginal<typeof RuntimeConfigModule>()),
  getSiteTitle: () => 'V2Board',
}));

vi.mock('@/lib/queries', () => ({
  userKeys: {
    info: ['user', 'info'],
  },
  useTransferMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: MutationCallbacks) =>
      runMockMutation(mocks.transferMutateAsync, payload, options),
  }),
  useWithdrawCommissionMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: MutationCallbacks) =>
      runMockMutation(mocks.withdrawMutateAsync, payload, options),
  }),
}));

interface MutationCallbacks {
  onError?: (error: unknown) => void;
  onSuccess?: (data: unknown) => void;
}

function runMockMutation(
  mutation: (...args: unknown[]) => unknown,
  payload: unknown,
  options?: MutationCallbacks,
) {
  void Promise.resolve(mutation(payload)).then(options?.onSuccess, options?.onError);
}

vi.mock('@/components/ui/select', () => ({
  Select: ({
    children,
    disabled,
    name,
    onValueChange,
    value,
  }: {
    children: ReactNode;
    disabled?: boolean;
    name?: string;
    onValueChange: (value: string) => void;
    value?: string;
  }) => {
    const trigger = findSelectTriggerProps(children);
    return (
      <select
        id={trigger.id}
        name={name}
        disabled={disabled}
        aria-invalid={trigger['aria-invalid']}
        aria-describedby={trigger['aria-describedby']}
        data-testid="invite-select-trigger"
        value={value ?? ''}
        onBlur={trigger.onBlur}
        onChange={(event) => onValueChange(event.target.value)}
      >
        <option value="">{findSelectPlaceholder(children)}</option>
        {collectSelectOptions(children).map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    );
  },
  SelectContent: ({ children }: { children: ReactNode }) => children,
  SelectItem: ({ children }: { children: ReactNode; value: string }) => children,
  SelectTrigger: ({ children }: { children: ReactNode }) => children,
  SelectValue: ({ placeholder }: { placeholder?: string }) => placeholder,
}));

function collectSelectOptions(children: ReactNode): Array<{ label: ReactNode; value: string }> {
  const options: Array<{ label: ReactNode; value: string }> = [];
  for (const child of Array.isArray(children) ? children : [children]) {
    if (!child || typeof child !== 'object' || !('props' in child)) continue;
    const props = child.props as { children?: ReactNode; value?: string };
    if (typeof props.value === 'string')
      options.push({ label: props.children, value: props.value });
    options.push(...collectSelectOptions(props.children));
  }
  return options;
}

function findSelectPlaceholder(children: ReactNode): ReactNode {
  for (const child of Array.isArray(children) ? children : [children]) {
    if (!child || typeof child !== 'object' || !('props' in child)) continue;
    const props = child.props as { children?: ReactNode; placeholder?: string };
    if (typeof props.placeholder === 'string') return props.placeholder;
    const nested = findSelectPlaceholder(props.children);
    if (nested) return nested;
  }
  return '';
}

function findSelectTriggerProps(children: ReactNode): {
  id?: string;
  onBlur?: () => void;
  'aria-invalid'?: boolean;
  'aria-describedby'?: string;
} {
  for (const child of Array.isArray(children) ? children : [children]) {
    if (!child || typeof child !== 'object' || !('props' in child)) continue;
    const props = child.props as {
      children?: ReactNode;
      id?: string;
      onBlur?: () => void;
      'aria-invalid'?: boolean;
      'aria-describedby'?: string;
      'data-testid'?: string;
    };
    if (props['data-testid'] === 'invite-select-trigger') return props;
    const nested = findSelectTriggerProps(props.children);
    if (nested.id) return nested;
  }
  return {};
}

function renderTransferDialog() {
  return renderWithProviders(
    <TransferDialog max={12345}>
      <button type="button">划转</button>
    </TransferDialog>,
  );
}

function renderWithdrawDialog() {
  return renderWithProviders(
    <WithdrawDialog methods={['Alipay', 'Bank']}>
      <button type="button">推广佣金提现</button>
    </WithdrawDialog>,
  );
}

function confirmButton() {
  return within(screen.getByTestId('invite-dialog-footer')).getByRole('button', { name: '确认' });
}

describe('invite commission dialogs shadcn behavior', () => {
  beforeEach(() => {
    mocks.invalidateQueries.mockReset();
    mocks.navigate.mockReset();
    mocks.transferMutateAsync.mockReset();
    mocks.transferMutateAsync.mockResolvedValue(true);
    mocks.withdrawMutateAsync.mockReset();
    mocks.withdrawMutateAsync.mockResolvedValue(true);
  });

  it('renders the shadcn transfer dialog and submits the raw amount to the mutation', async () => {
    const { user } = renderTransferDialog();

    await user.click(screen.getByRole('button', { name: '划转' }));

    const dialog = screen.getByTestId('invite-dialog');
    expect(within(dialog).getByText('划转后的余额仅用于V2Board消费使用')).toBeInTheDocument();
    const balance = within(dialog).getByLabelText('当前推广佣金余额');
    expect(balance).toBeDisabled();
    expect(balance).toHaveValue('123.45');

    await user.type(within(dialog).getByLabelText('划转金额'), '12.34');
    await user.click(confirmButton());

    // The dialog hands the raw yuan string to the mutation; the cents
    // conversion (100 * amount) lives in the API layer, not here.
    await waitFor(() => expect(mocks.transferMutateAsync).toHaveBeenCalledWith('12.34'));
    await waitFor(() => expect(screen.queryByTestId('invite-dialog')).not.toBeInTheDocument());
    // The user-record invalidation now lives in the transfer mutation's
    // onSuccess (covered in queries.test.ts), so the dialog no longer triggers
    // it directly.
    expect(mocks.invalidateQueries).not.toHaveBeenCalled();
  });

  it('resets the shadcn transfer amount when the dialog is closed and reopened', async () => {
    const { user } = renderTransferDialog();

    await user.click(screen.getByRole('button', { name: '划转' }));
    await user.type(screen.getByLabelText('划转金额'), '45.67');

    await user.click(
      within(screen.getByTestId('invite-dialog-footer')).getByRole('button', { name: '取消' }),
    );
    await waitFor(() => expect(screen.queryByTestId('invite-dialog')).not.toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: '划转' }));
    expect(screen.getByLabelText('划转金额')).toHaveValue('');

    await user.click(confirmButton());

    const error = await screen.findByText('请输入需要划转到余额的金额');
    const amount = screen.getByLabelText('划转金额');
    expect(error).toBeInTheDocument();
    expect(amount).toHaveAttribute('aria-invalid', 'true');
    expect(amount).toHaveAccessibleDescription('请输入需要划转到余额的金额');
    expect(mocks.transferMutateAsync).not.toHaveBeenCalled();
  });

  it('rejects a non-numeric transfer amount before calling the mutation', async () => {
    const { user } = renderTransferDialog();

    await user.click(screen.getByRole('button', { name: '划转' }));
    await user.type(screen.getByLabelText('划转金额'), 'abc');
    await user.click(confirmButton());

    expect(await screen.findByText('请输入有效的划转金额')).toBeInTheDocument();
    expect(mocks.transferMutateAsync).not.toHaveBeenCalled();
  });

  it('rejects a transfer amount with more than two decimal places', async () => {
    const { user } = renderTransferDialog();

    await user.click(screen.getByRole('button', { name: '划转' }));
    // 10.999 is finite, positive, and within balance, but cents cannot hold a
    // third decimal — the old path silently rounded it to 1100 cents.
    await user.type(screen.getByLabelText('划转金额'), '10.999');
    await user.click(confirmButton());

    expect(await screen.findByText('划转金额最多支持两位小数')).toBeInTheDocument();
    expect(mocks.transferMutateAsync).not.toHaveBeenCalled();
  });

  it('rejects a transfer amount above the available commission balance', async () => {
    const { user } = renderTransferDialog();

    await user.click(screen.getByRole('button', { name: '划转' }));
    // max={12345} cents = 123.45 yuan; 200 yuan exceeds it.
    await user.type(screen.getByLabelText('划转金额'), '200');
    await user.click(confirmButton());

    expect(await screen.findByText('划转金额不能超过当前推广佣金余额')).toBeInTheDocument();
    expect(mocks.transferMutateAsync).not.toHaveBeenCalled();
  });

  it('renders the shadcn withdraw dialog and submits the ticket-withdraw payload', async () => {
    const { user } = renderWithdrawDialog();

    await user.click(screen.getByRole('button', { name: '推广佣金提现' }));

    const dialog = screen.getByTestId('invite-dialog');
    expect(within(dialog).getByRole('heading', { name: '申请提现' })).toBeInTheDocument();

    await user.selectOptions(within(dialog).getByTestId('invite-select-trigger'), 'Bank');
    await user.type(within(dialog).getByLabelText('提现账号'), 'bank-account-123');
    await user.click(confirmButton());

    await waitFor(() =>
      expect(mocks.withdrawMutateAsync).toHaveBeenCalledWith({
        withdraw_account: 'bank-account-123',
        withdraw_method: 'Bank',
      }),
    );
    expect(mocks.navigate).toHaveBeenCalledWith('/ticket');
  });

  it('resets the shadcn withdraw dialog fields when closed and reopened', async () => {
    const { user } = renderWithdrawDialog();

    await user.click(screen.getByRole('button', { name: '推广佣金提现' }));
    await user.selectOptions(screen.getByTestId('invite-select-trigger'), 'Bank');
    await user.type(screen.getByLabelText('提现账号'), 'bank-account-123');

    await user.click(
      within(screen.getByTestId('invite-dialog-footer')).getByRole('button', { name: '取消' }),
    );
    await waitFor(() => expect(screen.queryByTestId('invite-dialog')).not.toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: '推广佣金提现' }));
    const methodSelect = screen.getByTestId('invite-select-trigger');
    expect(methodSelect).toHaveValue('');
    expect(
      within(methodSelect).getByRole('option', { name: '请选择提现方式' }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText('提现账号')).toHaveValue('');

    await user.click(confirmButton());

    const accountError = await screen.findByText('请输入提现账号');
    const method = screen.getByLabelText('提现方式');
    const account = screen.getByLabelText('提现账号');
    expect(accountError).toBeInTheDocument();
    expect(
      screen.getByText('请选择提现方式', { selector: '[data-slot="field-error"]' }),
    ).toBeInTheDocument();
    expect(method).toHaveAttribute('aria-invalid', 'true');
    expect(method).toHaveAccessibleDescription('请选择提现方式');
    expect(account).toHaveAttribute('aria-invalid', 'true');
    expect(account).toHaveAccessibleDescription('请输入提现账号');
    expect(mocks.withdrawMutateAsync).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });
});
