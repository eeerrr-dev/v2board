import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { act } from 'react';
import type { ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TransferDialog } from './transfer-dialog';
import { WithdrawDialog } from './withdraw-dialog';

const testDir = dirname(fileURLToPath(import.meta.url));

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
  useTranslation: () => ({
    i18n: { language: 'zh-CN' },
    t: (key: string, values?: Record<string, unknown>) => {
      const labels: Record<string, string> = {
        'common.cancel': '取消',
        'dashboard.transfer_to_balance': '推广佣金划转至余额',
        'invite.current_commission_balance': '当前推广佣金余额',
        'invite.transfer': '划转',
        'invite.transfer_amount': '划转金额',
        'invite.transfer_notice': '划转后的余额仅用于{title}消费使用',
        'invite.transfer_placeholder': '请输入需要划转到余额的金额',
        'invite.withdraw': '申请提现',
        'invite.withdraw_account': '提现账号',
        'invite.withdraw_account_placeholder': '请输入提现账号',
        'invite.withdraw_button': '推广佣金提现',
        'invite.withdraw_method': '提现方式',
        'invite.withdraw_method_placeholder': '请选择提现方式',
        'invite.withdraw_submit': '确认',
        'profile.confirm': '确认',
      };
      return (labels[key] ?? key)
        .replace('{{title}}', String(values?.title ?? ''))
        .replace('{title}', String(values?.title ?? ''));
    },
  }),
}));

vi.mock('@/lib/legacy-settings', () => ({
  getLegacySettings: () => ({
    title: 'V2Board',
  }),
}));

vi.mock('@/lib/queries', () => ({
  userKeys: {
    info: ['user', 'info'],
  },
  useTransferMutation: () => ({
    isPending: false,
    mutateAsync: mocks.transferMutateAsync,
  }),
  useWithdrawCommissionMutation: () => ({
    isPending: false,
    mutateAsync: mocks.withdrawMutateAsync,
  }),
}));

vi.mock('@/components/ui/select', () => ({
  Select: ({
    children,
    onValueChange,
    value,
  }: {
    children: ReactNode;
    onValueChange: (value: string) => void;
    value?: string;
  }) => (
    <select
      data-testid="invite-select-trigger"
      value={value ?? ''}
      onChange={(event) => onValueChange(event.target.value)}
    >
      <option value="">{findSelectPlaceholder(children)}</option>
      {collectSelectOptions(children).map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
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
    if (typeof props.value === 'string') options.push({ label: props.children, value: props.value });
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

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function resetMocks() {
  mocks.invalidateQueries.mockReset();
  mocks.navigate.mockReset();
  mocks.transferMutateAsync.mockReset();
  mocks.transferMutateAsync.mockResolvedValue(true);
  mocks.withdrawMutateAsync.mockReset();
  mocks.withdrawMutateAsync.mockResolvedValue(true);
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

function setNativeInputValue(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

describe('invite commission dialogs shadcn behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    resetMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
    document.body.className = '';
    document.body.removeAttribute('style');
  });

  it('renders the shadcn transfer dialog and submits the raw amount to the mutation', async () => {
    await act(async () => {
      root.render(
        <TransferDialog max={12345}>
          <button type="button">划转</button>
        </TransferDialog>,
      );
    });

    await act(async () => {
      container.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(document.body.innerHTML).toContain('data-testid="invite-dialog"');
    expect(document.body.innerHTML).toContain('划转后的余额仅用于V2Board消费使用');
    expect(document.body.innerHTML).toContain('当前推广佣金余额');
    expect(
      document.body.querySelector<HTMLButtonElement>(
        '[data-testid="invite-dialog-footer"] button:last-child',
      )
        ?.textContent?.replace(/\s/g, ''),
    ).toBe('确认');
    expect(document.body.innerHTML).not.toContain('提交提现');
    expect(document.body.querySelector<HTMLInputElement>('input[disabled]')!.value).toBe('123.45');

    const amount = Array.from(document.body.querySelectorAll<HTMLInputElement>('input')).find(
      (input) => input.placeholder === '请输入需要划转到余额的金额',
    )!;
    await act(async () => {
      setNativeInputValue(amount, '12.34');
      await Promise.resolve();
    });

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('[data-testid="invite-dialog-footer"] button:last-child')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.transferMutateAsync).toHaveBeenCalledWith('12.34');
    // The user-record invalidation now lives in the transfer mutation's
    // onSuccess (covered in queries.test.ts), so the dialog no longer triggers
    // it directly.
    expect(mocks.invalidateQueries).not.toHaveBeenCalled();
  });

  it('validates transfer with react-hook-form and keeps amount conversion out of the dialog', () => {
    const source = readFileSync(join(testDir, 'transfer-dialog.tsx'), 'utf8');

    expect(source).toContain("from 'react-hook-form'");
    expect(source).toContain('zodResolver(transferSchema)');
    expect(source).toContain('form.handleSubmit');
    expect(source).toContain('await transfer.mutateAsync(yuan);');
    expect(source).not.toContain('Number(yuan) * 100');
  });

  it('resets the shadcn transfer amount when the dialog is closed and reopened', async () => {
    await act(async () => {
      root.render(
        <TransferDialog max={12345}>
          <button type="button">划转</button>
        </TransferDialog>,
      );
    });

    await act(async () => {
      container.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const amount = Array.from(document.body.querySelectorAll<HTMLInputElement>('input')).find(
      (input) => input.placeholder === '请输入需要划转到余额的金额',
    )!;
    await act(async () => {
      setNativeInputValue(amount, '45.67');
      await Promise.resolve();
    });

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('[data-testid="invite-dialog-footer"] button:first-child')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      container.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const reopenedAmount = Array.from(document.body.querySelectorAll<HTMLInputElement>('input')).find(
      (input) => input.placeholder === '请输入需要划转到余额的金额',
    )!;
    expect(reopenedAmount.value).toBe('');

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('[data-testid="invite-dialog-footer"] button:last-child')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.transferMutateAsync).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain('请输入需要划转到余额的金额');
  });

  it('renders the shadcn withdraw dialog and submits the ticket-withdraw payload', async () => {
    await act(async () => {
      root.render(
        <WithdrawDialog methods={['Alipay', 'Bank']}>
          <button type="button">推广佣金提现</button>
        </WithdrawDialog>,
      );
    });

    await act(async () => {
      container.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(document.body.innerHTML).toContain('申请提现');
    expect(document.body.innerHTML).toContain('提现方式');
    expect(document.body.innerHTML).toContain('提现账号');
    expect(
      document.body.querySelector<HTMLButtonElement>(
        '[data-testid="invite-dialog-footer"] button:last-child',
      )
        ?.textContent?.replace(/\s/g, ''),
    ).toBe('确认');
    expect(document.body.innerHTML).not.toContain('提交提现');

    await act(async () => {
      const method = document.body.querySelector<HTMLSelectElement>('select')!;
      method.value = 'Bank';
      method.dispatchEvent(new Event('change', { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      const account = document.body.querySelector<HTMLInputElement>(
        'input[placeholder="请输入提现账号"]',
      )!;
      setNativeInputValue(account, 'bank-account-123');
      await Promise.resolve();
    });

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('[data-testid="invite-dialog-footer"] button:last-child')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.withdrawMutateAsync).toHaveBeenCalledWith({
      withdraw_account: 'bank-account-123',
      withdraw_method: 'Bank',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/ticket');
  });

  it('resets the shadcn withdraw dialog fields when closed and reopened', async () => {
    await act(async () => {
      root.render(
        <WithdrawDialog methods={['Alipay', 'Bank']}>
          <button type="button">推广佣金提现</button>
        </WithdrawDialog>,
      );
    });

    await act(async () => {
      container.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      const method = document.body.querySelector<HTMLSelectElement>('select')!;
      method.value = 'Bank';
      method.dispatchEvent(new Event('change', { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      const account = document.body.querySelector<HTMLInputElement>(
        'input[placeholder="请输入提现账号"]',
      )!;
      setNativeInputValue(account, 'bank-account-123');
      await Promise.resolve();
    });

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('[data-testid="invite-dialog-footer"] button:first-child')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      container.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(document.body.querySelector<HTMLSelectElement>('select')!.value).toBe('');
    expect(
      document.body.querySelector<HTMLInputElement>('input[placeholder="请输入提现账号"]')!.value,
    ).toBe('');

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('[data-testid="invite-dialog-footer"] button:last-child')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.withdrawMutateAsync).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain('请选择提现方式');
    expect(document.body.textContent).toContain('请输入提现账号');
  });
});
