import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { act } from 'react';
import type { CSSProperties } from 'react';
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

vi.mock('react-router-dom', () => ({
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
    mutateAsync: mocks.transferMutateAsync,
  }),
  useWithdrawCommissionMutation: () => ({
    mutateAsync: mocks.withdrawMutateAsync,
  }),
}));

vi.mock('@/components/legacy-select', () => ({
  LegacySelect: ({
    onChange,
    options,
    placeholder,
    style,
    value,
  }: {
    onChange: (value: string) => void;
    options: Array<{ label: string; value: string }>;
    placeholder?: string;
    style?: CSSProperties;
    value?: string;
  }) => (
    <select
      aria-label={placeholder}
      className="ant-select"
      style={style}
      value={value ?? ''}
      onChange={(event) => onChange(event.target.value)}
    >
      <option value="">{placeholder}</option>
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

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

describe('invite commission dialogs bundled-theme behavior', () => {
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

  it('renders the old transfer modal and submits the raw amount to the mutation', async () => {
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

    expect(document.body.innerHTML).toContain('推广佣金划转至余额');
    expect(document.body.innerHTML).toContain('划转后的余额仅用于V2Board消费使用');
    expect(document.body.innerHTML).toContain('当前推广佣金余额');
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
        .querySelector<HTMLButtonElement>('.ant-modal-footer .ant-btn-primary')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.transferMutateAsync).toHaveBeenCalledWith('12.34');
    expect(mocks.invalidateQueries).toHaveBeenCalledWith({ queryKey: ['user', 'info'] });
  });

  it('keeps transfer amount conversion out of the dialog like the old component', () => {
    const source = readFileSync(join(testDir, 'transfer-dialog.tsx'), 'utf8');

    expect(source).toContain('await mutateAsync(yuan);');
    expect(source).not.toContain('Number(yuan) * 100');
  });

  it('preserves the old transfer close/reopen quirk: input DOM remains, state is cleared', async () => {
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
      Array.from(document.body.querySelectorAll<HTMLButtonElement>('.ant-modal-footer .ant-btn'))[0]!
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
    expect(reopenedAmount.value).toBe('45.67');

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('.ant-modal-footer .ant-btn-primary')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.transferMutateAsync).toHaveBeenCalledWith(undefined);
  });

  it('renders the old withdraw modal and submits the ticket-withdraw payload', async () => {
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
        .querySelector<HTMLButtonElement>('.ant-modal-footer .ant-btn-primary')!
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

  it('preserves the old withdraw close/reopen quirk: uncontrolled account DOM remains, state is cleared', async () => {
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
      Array.from(document.body.querySelectorAll<HTMLButtonElement>('.ant-modal-footer .ant-btn'))[0]!
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
    ).toBe('bank-account-123');

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('.ant-modal-footer .ant-btn-primary')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.withdrawMutateAsync).toHaveBeenCalledWith({
      withdraw_account: undefined,
      withdraw_method: undefined,
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/ticket');
  });
});
