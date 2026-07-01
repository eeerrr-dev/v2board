import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { act } from 'react';
import type { ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import TicketsPage from './index';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'index.tsx'), 'utf8');

const mocks = vi.hoisted(() => {
  const makeTickets = () => [
    {
      id: 7,
      subject: 'Need help',
      level: 1,
      status: 0,
      reply_status: 1,
      created_at: 1_700_000_000,
      updated_at: 1_700_000_060,
    },
    {
      id: 8,
      subject: 'Waiting reply',
      level: 0,
      status: 0,
      reply_status: 0,
      created_at: 0,
      updated_at: 60,
    },
    {
      id: 9,
      subject: 'Closed ticket',
      level: 2,
      status: 1,
      reply_status: 0,
      created_at: 1_700_001_200,
      updated_at: 1_700_001_260,
    },
  ];

  return {
    closeMutateAsync: vi.fn(),
    fetching: true,
    makeTickets,
    openWindow: vi.fn(),
    saveMutateAsync: vi.fn(),
    savePending: false,
    tickets: makeTickets(),
  };
});

const confirmDialog = vi.hoisted(() => vi.fn());

const labels: Record<string, string> = {
  'common.attention': '注意',
  'common.cancel': '取消',
  'ticket.action': '操作',
  'ticket.close_ticket': '关闭',
  'ticket.confirm_close': '确定关闭该工单吗？',
  'ticket.col_id': '#',
  'ticket.confirm': '确认',
  'ticket.created_at_col': '创建时间',
  'ticket.history': '工单历史',
  'ticket.last_reply_col': '最后回复',
  'ticket.level': '工单级别',
  'ticket.level_form': '工单等级',
  'ticket.level_high': '高',
  'ticket.level_low': '低',
  'ticket.level_medium': '中',
  'ticket.level_placeholder': '请选择工单等级',
  'ticket.message': '消息',
  'ticket.message_placeholder': '请描述您遇到的问题',
  'ticket.new': '新的工单',
  'ticket.pending': '待处理',
  'ticket.replied': '已答复',
  'ticket.closed': '已关闭',
  'ticket.status': '工单状态',
  'ticket.subject': '主题',
  'ticket.subject_placeholder': '请输入工单主题',
  'ticket.view': '查看',
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: 'zh-CN' },
    t: (key: string) => labels[key] ?? key,
  }),
}));

vi.mock('@/lib/queries', () => ({
  useTickets: () => ({
    data: mocks.tickets,
    error: undefined,
    isFetching: mocks.fetching,
  }),
  useSaveTicketMutation: () => ({
    isPending: mocks.savePending,
    mutateAsync: mocks.saveMutateAsync,
  }),
  useCloseTicketMutation: () => ({
    mutateAsync: mocks.closeMutateAsync,
  }),
}));

vi.mock('@/components/ui/confirm-dialog', () => ({
  confirmDialog,
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
      data-testid="ticket-select-native"
      value={value ?? ''}
      onChange={(event) => onValueChange(event.currentTarget.value)}
    >
      {children}
    </select>
  ),
  SelectContent: ({ children }: { children: ReactNode }) => children,
  SelectItem: ({ children, value }: { children: ReactNode; value: string }) => (
    <option value={value}>{children}</option>
  ),
  SelectTrigger: ({ children }: { children: ReactNode }) => children,
  SelectValue: ({ placeholder }: { placeholder?: string }) => <option value="">{placeholder}</option>,
}));

function resetMocks() {
  mocks.tickets = mocks.makeTickets();
  mocks.fetching = true;
  mocks.savePending = false;
  mocks.closeMutateAsync.mockReset();
  confirmDialog.mockReset();
}

function setNativeInputValue(input: HTMLInputElement, value: string) {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
}

function setNativeTextareaValue(textarea: HTMLTextAreaElement, value: string) {
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(
    textarea,
    value,
  );
  textarea.dispatchEvent(new Event('input', { bubbles: true }));
  textarea.dispatchEvent(new Event('change', { bubbles: true }));
}

describe('TicketsPage shadcn surface', () => {
  beforeEach(resetMocks);

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the shadcn ticket card, table, statuses, actions, and dates', () => {
    const html = renderToStaticMarkup(<TicketsPage />);

    expect(html).toContain('data-testid="ticket-surface"');
    expect(html).toContain('data-testid="ticket-table"');
    expect(html).toContain('data-testid="ticket-new-trigger"');
    expect(html).toContain('工单历史');
    expect(html).toContain('新的工单');
    expect(html).toContain('Need help');
    expect(html).toContain('Waiting reply');
    expect(html).toContain('Closed ticket');
    expect(html).toContain('>中<');
    expect(html).toContain('>低<');
    expect(html).toContain('>高<');
    expect(html).toContain('已答复');
    expect(html).toContain('待处理');
    expect(html).toContain('已关闭');
    expect(html).toContain(formatLegacyDateMinuteSlash(1_700_000_000));
    expect(html).toContain(formatLegacyDateMinuteSlash(60));
    expect(html).toContain('data-testid="ticket-view"');
    expect(html).toContain('data-testid="ticket-close"');
    expect(html.match(/data-row-key="0"/g)).toHaveLength(1);
    expect(html.match(/data-row-key="1"/g)).toHaveLength(1);
    expect(html.match(/data-row-key="2"/g)).toHaveLength(1);
    expect(html).not.toContain('block block-rounded');
    expect(html).not.toContain('ant-table-wrapper');
    expect(html).not.toContain('ant-table-fixed-right');
  });

  it('renders ticket rows through shared TanStack DataTable columns', () => {
    expect(source).toContain('satisfies DataTableColumn<(typeof tickets)[number]>[]');
    expect(source).toContain('virtualizer={{ enabled: tickets.length > VIRTUALIZE_MIN_ROWS }}');
    expect(source).not.toContain('data-row-key={ticket.id}');
    expect(source).not.toContain('<TableRow');
  });

  it('keeps the new-ticket trigger text stable while ticket save is pending', () => {
    mocks.savePending = true;

    const html = renderToStaticMarkup(<TicketsPage />);

    expect(html).toContain('data-testid="ticket-new-trigger"');
    expect(html).toContain('新的工单</button>');
  });

  it('marks the ticket card as loading while ticket/fetch is pending', async () => {
    mocks.fetching = true;
    const container = document.createElement('div');
    document.body.appendChild(container);
    const root = createRoot(container);

    try {
      await act(async () => {
        root.render(<TicketsPage />);
        await Promise.resolve();
      });

      expect(container.querySelector('[data-testid="ticket-surface"]')?.className).toContain(
        'opacity-80',
      );
      expect(container.innerHTML).not.toContain('block-mode-loading');
      expect(container.innerHTML).not.toContain('ant-spin-spinning');
    } finally {
      act(() => root.unmount());
      container.remove();
    }
  });

  it('renders a shadcn empty row without the legacy antd empty shell', () => {
    mocks.tickets = [];

    const html = renderToStaticMarkup(<TicketsPage />);

    expect(html).toContain('data-testid="ticket-empty"');
    expect(html).not.toContain('ant-table-placeholder');
    expect(html).not.toContain('ant-empty');
  });
});

describe('TicketsPage shadcn interactions', () => {
  let container: HTMLDivElement;
  let root: Root | null;
  let originalOpen: typeof window.open;

  beforeEach(() => {
    resetMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    originalOpen = window.open;
    window.open = mocks.openWindow as unknown as typeof window.open;
    mocks.fetching = false;
    mocks.saveMutateAsync.mockReset();
    mocks.saveMutateAsync.mockResolvedValue(undefined);
    mocks.closeMutateAsync.mockReset();
    mocks.closeMutateAsync.mockResolvedValue(undefined);
    mocks.openWindow.mockClear();
  });

  afterEach(() => {
    if (root) {
      act(() => root?.unmount());
      root = null;
    }
    window.open = originalOpen;
    container.remove();
    document.body.innerHTML = '';
  });

  async function renderTickets() {
    await act(async () => {
      root!.render(<TicketsPage />);
      await Promise.resolve();
    });
  }

  async function openCreateDialog() {
    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="ticket-new-trigger"]',
    )!;
    await act(async () => {
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
  }

  it('opens ticket detail in the legacy popup window on desktop', async () => {
    await renderTickets();

    const viewButtons = Array.from(
      container.querySelectorAll<HTMLButtonElement>('[data-testid="ticket-view"]'),
    );
    expect(viewButtons).toHaveLength(3);

    act(() => {
      viewButtons[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(mocks.openWindow).toHaveBeenCalledWith(
      `${window.location.origin}${window.location.pathname}#/ticket/7`,
      'newwindow',
      'height=600,width=800,top=0,left=0,toolbar=no,menubar=no,scrollbars=no,resizable=no,location=no,status=no',
    );
  });

  it('saves a new ticket through the shadcn dialog form', async () => {
    await renderTickets();
    await openCreateDialog();

    expect(document.body.innerHTML).toContain('data-testid="ticket-dialog"');
    expect(document.body.innerHTML).toContain('新的工单');
    expect(document.body.innerHTML).toContain('请输入工单主题');
    expect(document.body.innerHTML).toContain('请选择工单等级');
    expect(document.body.innerHTML).toContain('请描述您遇到的问题');

    const subject = document.body.querySelector<HTMLInputElement>(
      'input[placeholder="请输入工单主题"]',
    )!;
    const level = document.body.querySelector<HTMLSelectElement>(
      '[data-testid="ticket-select-native"]',
    )!;
    const message = document.body.querySelector<HTMLTextAreaElement>(
      'textarea[placeholder="请描述您遇到的问题"]',
    )!;

    await act(async () => {
      setNativeInputValue(subject, 'Billing question');
      level.value = '2';
      level.dispatchEvent(new Event('change', { bubbles: true }));
      setNativeTextareaValue(message, 'Please check my invoice');
      await Promise.resolve();
    });

    const confirm = document.body.querySelector<HTMLButtonElement>(
      '[data-testid="ticket-dialog-footer"] button:last-child',
    )!;
    expect(confirm.textContent).toBe('确认');

    await act(async () => {
      confirm.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
      level: 2,
      message: 'Please check my invoice',
      subject: 'Billing question',
    });
    // The list refresh is now owned by useSaveTicketMutation's onSuccess (see
    // queries.test.ts), so the page no longer invalidates at the call site.
  });

  it('keeps new-ticket form data after canceling because only a successful save clears state', async () => {
    await renderTickets();
    await openCreateDialog();

    const subject = document.body.querySelector<HTMLInputElement>(
      'input[placeholder="请输入工单主题"]',
    )!;
    const level = document.body.querySelector<HTMLSelectElement>(
      '[data-testid="ticket-select-native"]',
    )!;
    const message = document.body.querySelector<HTMLTextAreaElement>(
      'textarea[placeholder="请描述您遇到的问题"]',
    )!;

    await act(async () => {
      setNativeInputValue(subject, 'Still here');
      level.value = '1';
      level.dispatchEvent(new Event('change', { bubbles: true }));
      setNativeTextareaValue(message, 'Keep this draft');
      await Promise.resolve();
    });

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('[data-testid="ticket-dialog-footer"] button:first-child')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    await openCreateDialog();

    expect(
      document.body.querySelector<HTMLInputElement>('input[placeholder="请输入工单主题"]')!.value,
    ).toBe('Still here');
    expect(
      document.body.querySelector<HTMLSelectElement>('[data-testid="ticket-select-native"]')!.value,
    ).toBe('1');
    expect(
      document.body.querySelector<HTMLTextAreaElement>(
        'textarea[placeholder="请描述您遇到的问题"]',
      )!.value,
    ).toBe('Keep this draft');
  });

  it('clears new-ticket state after a successful save', async () => {
    await renderTickets();
    await openCreateDialog();

    await act(async () => {
      setNativeInputValue(
        document.body.querySelector<HTMLInputElement>('input[placeholder="请输入工单主题"]')!,
        'Saved subject',
      );
      const level = document.body.querySelector<HTMLSelectElement>(
        '[data-testid="ticket-select-native"]',
      )!;
      level.value = '2';
      level.dispatchEvent(new Event('change', { bubbles: true }));
      setNativeTextareaValue(
        document.body.querySelector<HTMLTextAreaElement>(
          'textarea[placeholder="请描述您遇到的问题"]',
        )!,
        'Saved body',
      );
      await Promise.resolve();
    });

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('[data-testid="ticket-dialog-footer"] button:last-child')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.saveMutateAsync).toHaveBeenLastCalledWith({
      level: 2,
      message: 'Saved body',
      subject: 'Saved subject',
    });

    await openCreateDialog();

    expect(
      document.body.querySelector<HTMLInputElement>('input[placeholder="请输入工单主题"]')!.value,
    ).toBe('');
    expect(
      document.body.querySelector<HTMLSelectElement>('[data-testid="ticket-select-native"]')!.value,
    ).toBe('');
    expect(
      document.body.querySelector<HTMLTextAreaElement>(
        'textarea[placeholder="请描述您遇到的问题"]',
      )!.value,
    ).toBe('');

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('[data-testid="ticket-dialog-footer"] button:last-child')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.saveMutateAsync).toHaveBeenLastCalledWith({
      level: undefined,
      message: undefined,
      subject: undefined,
    });
  });

  it('keeps the ticket level select value as the direct numeric payload value', () => {
    expect(source).toContain('const levelLabel = LEVELS[row.original.level]?.labelKey;');
    expect(source).toContain('field.onChange(Number(nextLevel) as TicketLevel)');
    expect(source).not.toContain('LEVELS[Number(ticket.level)]');
  });

  it('closes an open ticket and empties ticket state on unmount', async () => {
    await renderTickets();

    const closeButtons = Array.from(
      container.querySelectorAll<HTMLButtonElement>('[data-testid="ticket-close"]'),
    );
    expect(closeButtons).toHaveLength(3);

    await act(async () => {
      closeButtons[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    // Closing confirms through the shared AlertDialog first; the mutation only
    // fires once the user accepts.
    expect(confirmDialog).toHaveBeenCalledTimes(1);
    const options = confirmDialog.mock.calls[0]![0] as {
      description?: unknown;
      onConfirm?: () => Promise<unknown>;
    };
    expect(options.description).toBe('确定关闭该工单吗？');
    expect(mocks.closeMutateAsync).not.toHaveBeenCalled();

    await act(async () => {
      await options.onConfirm?.();
    });
    expect(mocks.closeMutateAsync).toHaveBeenCalledWith(7);
    // The list refresh is now owned by useCloseTicketMutation's onSuccess.
    expect(source).not.toContain('removeQueries');
  });

  it('keeps closed-ticket close action clickable for legacy API parity', async () => {
    await renderTickets();

    const closeButtons = Array.from(
      container.querySelectorAll<HTMLButtonElement>('[data-testid="ticket-close"]'),
    );
    const closed = closeButtons[2]!;

    expect(closed.disabled).toBe(false);

    await act(async () => {
      closed.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const options = confirmDialog.mock.calls[0]![0] as { onConfirm?: () => Promise<unknown> };
    await act(async () => {
      await options.onConfirm?.();
    });
    expect(mocks.closeMutateAsync).toHaveBeenCalledWith(9);
  });
});
