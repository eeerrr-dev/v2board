import type { ReactNode } from 'react';
import { screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import { getLocaleAntdMessages } from '@v2board/i18n';
import { VIRTUALIZE_MIN_ROWS } from '@/components/ui/table';
import { renderWithProviders } from '@/test/render';
import TicketsPage from './index';

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
    error: false,
    fetching: true,
    makeTickets,
    openWindow: vi.fn(),
    refetch: vi.fn(),
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
    isError: mocks.error,
    isFetching: mocks.fetching,
    refetch: mocks.refetch,
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

// Radix Select does not open under happy-dom (pointer capture / scrollIntoView),
// so the file keeps its native-select stand-in. The real trigger/content hooks
// (`ticket-select-trigger` / `ticket-select-content`) stay covered by the
// interaction-parity harness in a real browser.
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
      aria-label="工单等级"
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
  mocks.error = false;
  mocks.fetching = true;
  mocks.savePending = false;
  mocks.saveMutateAsync.mockReset();
  mocks.closeMutateAsync.mockReset();
  mocks.openWindow.mockClear();
  mocks.refetch.mockClear();
  confirmDialog.mockReset();
}

describe('TicketsPage shadcn surface', () => {
  beforeEach(() => {
    resetMocks();
    mocks.fetching = false;
  });

  it('renders the ticket card, table rows, level labels, statuses, actions, and dates', () => {
    const { container } = renderWithProviders(<TicketsPage />);

    expect(screen.getByTestId('ticket-surface')).toBeInTheDocument();
    expect(screen.getByTestId('ticket-table')).toBeInTheDocument();
    expect(screen.getByText('工单历史')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '新的工单' })).toBe(
      screen.getByTestId('ticket-new-trigger'),
    );

    expect(screen.getByText('Need help')).toBeInTheDocument();
    expect(screen.getByText('Waiting reply')).toBeInTheDocument();
    expect(screen.getByText('Closed ticket')).toBeInTheDocument();

    // Level labels come straight from the numeric `level` field (0/1/2 -> 低/中/高).
    expect(screen.getByText('中')).toBeInTheDocument();
    expect(screen.getByText('低')).toBeInTheDocument();
    expect(screen.getByText('高')).toBeInTheDocument();

    expect(screen.getByText('已答复')).toBeInTheDocument();
    expect(screen.getByText('待处理')).toBeInTheDocument();
    expect(screen.getByText('已关闭')).toBeInTheDocument();

    expect(screen.getByText(formatLegacyDateMinuteSlash(1_700_000_000))).toBeInTheDocument();
    expect(screen.getByText(formatLegacyDateMinuteSlash(60))).toBeInTheDocument();

    expect(screen.getAllByTestId('ticket-view')).toHaveLength(3);
    expect(screen.getAllByTestId('ticket-close')).toHaveLength(3);

    // Rows are keyed by index (not ticket id) — the row-key contract the
    // parity harness reads via [data-row-key].
    const rowKeys = Array.from(container.querySelectorAll('[data-row-key]')).map((row) =>
      row.getAttribute('data-row-key'),
    );
    expect(rowKeys).toEqual(['0', '1', '2']);
    // Small lists are not virtualized: no measurement wiring on the rows.
    expect(container.querySelector('[data-index]')).toBeNull();
  });

  it('windows rows through the shared virtualizer once the list outgrows VIRTUALIZE_MIN_ROWS', () => {
    const total = VIRTUALIZE_MIN_ROWS + 1;
    mocks.tickets = Array.from({ length: total }, (_, index) => ({
      id: index + 1,
      subject: `Ticket ${index + 1}`,
      level: 0,
      status: 0,
      reply_status: 0,
      created_at: 1_700_000_000 + index * 60,
      updated_at: 1_700_000_000 + index * 60,
    }));
    // happy-dom has no layout, so give the virtualizer's scroll container a
    // real viewport size (virtual-core reads offsetWidth/offsetHeight);
    // otherwise the visible window is empty.
    const heightSpy = vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(600);
    const widthSpy = vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(1024);

    try {
      const { container } = renderWithProviders(<TicketsPage />);

      const dataRows = Array.from(container.querySelectorAll('tr[data-row-key]'));
      // The virtualizer only mounts a window of rows plus spacer padding rather
      // than all 151 of them, and wires each mounted row for measurement.
      expect(dataRows.length).toBeGreaterThan(0);
      expect(dataRows.length).toBeLessThan(total);
      expect(dataRows[0]).toHaveAttribute('data-index');
    } finally {
      heightSpy.mockRestore();
      widthSpy.mockRestore();
    }
  });

  it('keeps the new-ticket trigger usable while a save is pending and marks the confirm busy', async () => {
    mocks.savePending = true;

    const { user } = renderWithProviders(<TicketsPage />);

    const trigger = screen.getByTestId('ticket-new-trigger');
    expect(trigger).toHaveTextContent('新的工单');
    expect(trigger).toBeEnabled();

    await user.click(trigger);

    // The pending state lives on the dialog's submit button, not the trigger.
    const confirm = screen.getByRole('button', { name: '确认' });
    expect(confirm).toBeDisabled();
    expect(confirm).toHaveAttribute('aria-busy', 'true');
  });

  it('marks the ticket card as loading while the ticket fetch is pending', () => {
    mocks.fetching = true;

    renderWithProviders(<TicketsPage />);

    expect(screen.getByTestId('ticket-surface')).toHaveClass('opacity-80');
  });

  it('renders the shared empty row with the locale antd empty copy', () => {
    mocks.tickets = [];

    renderWithProviders(<TicketsPage />);

    expect(screen.getByTestId('ticket-empty')).toHaveTextContent(
      getLocaleAntdMessages('zh-CN').emptyDescription,
    );
  });

  it('surfaces a failed fetch as an error state with retry instead of the empty table', async () => {
    mocks.tickets = [];
    mocks.error = true;

    const { user } = renderWithProviders(<TicketsPage />);

    // A failed fetch must not be misrepresented as "no tickets".
    expect(screen.getByTestId('ticket-error')).toBeInTheDocument();
    expect(screen.queryByTestId('ticket-empty')).not.toBeInTheDocument();
    expect(screen.queryByTestId('ticket-table')).not.toBeInTheDocument();
    // The new-ticket entry point stays available alongside the error.
    expect(screen.getByTestId('ticket-new-trigger')).toBeEnabled();

    await user.click(screen.getByTestId('error-state-retry'));

    expect(mocks.refetch).toHaveBeenCalledTimes(1);
  });
});

describe('TicketsPage shadcn interactions', () => {
  let originalOpen: typeof window.open;

  beforeEach(() => {
    resetMocks();
    mocks.fetching = false;
    mocks.saveMutateAsync.mockResolvedValue(undefined);
    mocks.closeMutateAsync.mockResolvedValue(undefined);
    originalOpen = window.open;
    window.open = mocks.openWindow as unknown as typeof window.open;
  });

  afterEach(() => {
    window.open = originalOpen;
  });

  it('opens ticket detail in the legacy popup window on desktop', async () => {
    const { user } = renderWithProviders(<TicketsPage />);

    const viewButtons = screen.getAllByTestId('ticket-view');
    expect(viewButtons).toHaveLength(3);

    await user.click(viewButtons[0]!);

    expect(mocks.openWindow).toHaveBeenCalledWith(
      `${window.location.origin}${window.location.pathname}#/ticket/7`,
      'newwindow',
      'height=600,width=800,top=0,left=0,toolbar=no,menubar=no,scrollbars=no,resizable=no,location=no,status=no',
    );
  });

  it('saves a new ticket through the shadcn dialog form', async () => {
    const { user } = renderWithProviders(<TicketsPage />);

    await user.click(screen.getByTestId('ticket-new-trigger'));

    expect(screen.getByTestId('ticket-dialog')).toBeInTheDocument();
    expect(screen.getByTestId('ticket-dialog-title')).toHaveTextContent('新的工单');

    const subject = screen.getByLabelText('主题');
    expect(subject).toHaveAttribute('placeholder', '请输入工单主题');
    const message = screen.getByLabelText('消息');
    expect(message).toHaveAttribute('placeholder', '请描述您遇到的问题');

    await user.type(subject, 'Billing question');
    await user.selectOptions(screen.getByTestId('ticket-select-native'), '2');
    await user.type(message, 'Please check my invoice');

    // The parity harness submits via the footer's last button, so keep the
    // confirm button in that slot.
    const footerButtons = within(screen.getByTestId('ticket-dialog-footer')).getAllByRole(
      'button',
    );
    const confirm = footerButtons[footerButtons.length - 1]!;
    expect(confirm).toHaveTextContent('确认');

    await user.click(confirm);

    // The save payload carries the numeric level exactly as selected.
    expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
      level: 2,
      message: 'Please check my invoice',
      subject: 'Billing question',
    });
    // The list refresh is owned by useSaveTicketMutation's onSuccess (see
    // queries.test.ts), so the page does not invalidate at the call site.
  });

  it('blocks an empty submit and surfaces the required-field errors', async () => {
    const { user } = renderWithProviders(<TicketsPage />);

    await user.click(screen.getByTestId('ticket-new-trigger'));
    await user.click(screen.getByRole('button', { name: '确认' }));

    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();

    // The canonical Form primitive renders each error through FormMessage (the
    // parity harness never read the old per-field error testids). The three
    // messages carry the placeholder copy as required text, in field order.
    await screen.findByText('请输入工单主题');
    const dialog = screen.getByTestId('ticket-dialog');
    const messages = Array.from(
      dialog.querySelectorAll<HTMLElement>('[data-slot="form-message"]'),
    );
    expect(messages.map((node) => node.textContent)).toEqual([
      '请输入工单主题',
      '请选择工单等级',
      '请描述您遇到的问题',
    ]);

    // Each control is now wired to its message via aria-invalid +
    // aria-describedby — the accessibility the hand-rolled fields lacked.
    const subject = screen.getByLabelText('主题');
    expect(subject).toHaveAttribute('aria-invalid', 'true');
    expect(subject.getAttribute('aria-describedby')).toContain(messages[0]!.id);
    const message = screen.getByLabelText('消息');
    expect(message).toHaveAttribute('aria-invalid', 'true');
    expect(message.getAttribute('aria-describedby')).toContain(messages[2]!.id);
  });

  it('keeps new-ticket form data after canceling because only a successful save clears state', async () => {
    const { user } = renderWithProviders(<TicketsPage />);

    await user.click(screen.getByTestId('ticket-new-trigger'));
    await user.type(screen.getByLabelText('主题'), 'Still here');
    await user.selectOptions(screen.getByTestId('ticket-select-native'), '1');
    await user.type(screen.getByLabelText('消息'), 'Keep this draft');

    await user.click(screen.getByRole('button', { name: '取消' }));
    await waitFor(() => {
      expect(screen.queryByTestId('ticket-dialog')).not.toBeInTheDocument();
    });

    await user.click(screen.getByTestId('ticket-new-trigger'));

    expect(screen.getByLabelText('主题')).toHaveValue('Still here');
    expect(screen.getByTestId('ticket-select-native')).toHaveValue('1');
    expect(screen.getByLabelText('消息')).toHaveValue('Keep this draft');
  });

  it('clears new-ticket state after a successful save', async () => {
    const { user } = renderWithProviders(<TicketsPage />);

    await user.click(screen.getByTestId('ticket-new-trigger'));
    await user.type(screen.getByLabelText('主题'), 'Saved subject');
    await user.selectOptions(screen.getByTestId('ticket-select-native'), '2');
    await user.type(screen.getByLabelText('消息'), 'Saved body');
    await user.click(screen.getByRole('button', { name: '确认' }));

    expect(mocks.saveMutateAsync).toHaveBeenLastCalledWith({
      level: 2,
      message: 'Saved body',
      subject: 'Saved subject',
    });
    await waitFor(() => {
      expect(screen.queryByTestId('ticket-dialog')).not.toBeInTheDocument();
    });

    await user.click(screen.getByTestId('ticket-new-trigger'));

    expect(screen.getByLabelText('主题')).toHaveValue('');
    expect(screen.getByTestId('ticket-select-native')).toHaveValue('');
    expect(screen.getByLabelText('消息')).toHaveValue('');

    // Re-submitting the now-empty form is blocked by the required-field
    // validation, so the mutation is not called a second time.
    await user.click(screen.getByRole('button', { name: '确认' }));

    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);
    expect(await screen.findByText('请输入工单主题')).toBeInTheDocument();
  });

  it('confirms before closing a ticket and only fires the close mutation on accept', async () => {
    const { user } = renderWithProviders(<TicketsPage />);

    const closeButtons = screen.getAllByTestId('ticket-close');
    expect(closeButtons).toHaveLength(3);

    await user.click(closeButtons[0]!);

    // Closing confirms through the shared AlertDialog first; the mutation only
    // fires once the user accepts.
    expect(confirmDialog).toHaveBeenCalledTimes(1);
    const options = confirmDialog.mock.calls[0]![0] as {
      description?: unknown;
      title?: unknown;
      onConfirm?: () => Promise<unknown>;
    };
    expect(options.title).toBe('注意');
    expect(options.description).toBe('确定关闭该工单吗？');
    expect(mocks.closeMutateAsync).not.toHaveBeenCalled();

    await options.onConfirm?.();

    expect(mocks.closeMutateAsync).toHaveBeenCalledWith(7);
  });

  it('keeps closed-ticket close action clickable for legacy API parity', async () => {
    const { user } = renderWithProviders(<TicketsPage />);

    const closed = screen.getAllByTestId('ticket-close')[2]!;
    expect(closed).toBeEnabled();

    await user.click(closed);

    const options = confirmDialog.mock.calls[0]![0] as { onConfirm?: () => Promise<unknown> };
    await options.onConfirm?.();

    expect(mocks.closeMutateAsync).toHaveBeenCalledWith(9);
  });
});
