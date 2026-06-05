import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import TicketsPage from './index';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'index.tsx'), 'utf8');

const mocks = vi.hoisted(() => ({
  invalidateQueries: vi.fn(),
  removeQueries: vi.fn(),
  saveMutateAsync: vi.fn(),
  savePending: false,
  closeMutateAsync: vi.fn(),
  openWindow: vi.fn(),
  tickets: [
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
  ],
  fetching: true,
}));

const labels: Record<string, string> = {
  'common.cancel': '取消',
  'ticket.action': '操作',
  'ticket.close_ticket': '关闭',
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
    t: (key: string) => labels[key] ?? key,
    i18n: { language: 'zh-CN' },
  }),
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({
    invalidateQueries: mocks.invalidateQueries,
    removeQueries: mocks.removeQueries,
  }),
}));

vi.mock('@/lib/queries', () => ({
  userKeys: {
    tickets: ['user', 'tickets'],
  },
  useTickets: () => ({
    data: mocks.tickets,
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

vi.mock('@/components/legacy-select', () => ({
  LegacySelect: ({
    value,
    placeholder,
    options,
    onChange,
  }: {
    value?: number;
    placeholder?: string;
    options: Array<{ value: number; label: string }>;
    onChange: (value: number) => void;
  }) => (
    <select
      className="ant-select legacy-select-probe"
      data-placeholder={placeholder}
      value={value ?? ''}
      onChange={(event) => onChange(Number(event.currentTarget.value))}
    >
      <option value="" />
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

describe('TicketsPage bundled-theme table', () => {
  beforeEach(() => {
    mocks.fetching = true;
    mocks.savePending = false;
  });

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the legacy table shell, fixed action column, statuses, dates, and dividers', () => {
    const html = renderToStaticMarkup(<TicketsPage />);

    expect(html).toContain('block block-rounded js-appear-enabled ');
    expect(html).not.toContain('block-mode-loading');
    expect(html).toContain('工单历史');
    expect(html).toContain('btn btn-primary btn-sm btn-primary btn-rounded px-3');
    expect(html).toContain('新的工单');
    expect(html).toContain('ant-table-wrapper');
    expect(html).toContain('class="ant-table-fixed" style="width:900px;table-layout:auto"');
    expect(html).toContain('class="ant-table-fixed" style="table-layout:auto"');
    expect(html).toContain('ant-table-fixed-right');
    expect(html).toContain('<span class="ant-table-column-title">#</span>');
    expect(html).toContain('主题');
    expect(html).toContain('工单级别');
    expect(html).toContain('工单状态');
    expect(html).toContain('创建时间');
    expect(html).toContain('最后回复');
    expect(html).toContain('操作');
    expect(html).toContain('Need help');
    expect(html).toContain('Waiting reply');
    expect(html).toContain('Closed ticket');
    expect(html).toContain('<td>中</td>');
    expect(html).toContain('<td>低</td>');
    expect(html).toContain('<td>高</td>');
    expect(html).toContain('ant-badge-status-processing');
    expect(html).toContain('已答复');
    expect(html).toContain('ant-badge-status-error');
    expect(html).toContain('待处理');
    expect(html).toContain('ant-badge-status-success');
    expect(html).toContain('已关闭');
    expect(html).toContain(formatLegacyDateMinuteSlash(1_700_000_000));
    expect(html).toContain(formatLegacyDateMinuteSlash(60));
    expect(html.match(/ant-divider ant-divider-vertical/g)).toHaveLength(6);
    expect(html).not.toContain('role="separator"');
    expect(html).not.toContain('data-row-key');
  });

  it('keeps bundled antd table row keys internal-only', () => {
    expect(source).not.toContain('data-row-key');
  });

  it('keeps the original new-ticket button text even while ticket save is pending', () => {
    mocks.savePending = true;

    const html = renderToStaticMarkup(<TicketsPage />);

    expect(html).toContain('btn btn-primary btn-sm btn-primary btn-rounded px-3');
    expect(html).toContain('新的工单</button>');
    expect(html).not.toContain('anticon-loading');
  });

  it('applies the original fetch loading class to the block, not the inner Table spin', async () => {
    mocks.fetching = true;
    const container = document.createElement('div');
    document.body.appendChild(container);
    const root = createRoot(container);

    try {
      await act(async () => {
        root.render(<TicketsPage />);
        await Promise.resolve();
      });

      expect(container.querySelector('.block')?.className).toContain('block-mode-loading');
      expect(container.innerHTML).not.toContain('ant-spin-spinning');
      expect(container.innerHTML).not.toContain('ant-spin-blur');
    } finally {
      act(() => root.unmount());
      container.remove();
    }
  });
});

describe('TicketsPage legacy interactions', () => {
  let container: HTMLDivElement;
  let root: Root | null;
  let originalOpen: typeof window.open;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    originalOpen = window.open;
    window.open = mocks.openWindow as unknown as typeof window.open;
    mocks.fetching = false;
    mocks.invalidateQueries.mockClear();
    mocks.removeQueries.mockClear();
    mocks.saveMutateAsync.mockReset();
    mocks.saveMutateAsync.mockResolvedValue(undefined);
    mocks.savePending = false;
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

  it('opens ticket detail in the legacy popup window on desktop', async () => {
    await act(async () => {
      root!.render(<TicketsPage />);
      await Promise.resolve();
    });

    const viewLinks = Array.from(container.querySelectorAll('a')).filter(
      (link) => link.textContent === '查看',
    );
    expect(viewLinks).toHaveLength(6);

    act(() => {
      viewLinks[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(mocks.openWindow).toHaveBeenCalledWith(
      `${window.location.origin}${window.location.pathname}#/ticket/7`,
      'newwindow',
      'height=600,width=800,top=0,left=0,toolbar=no,menubar=no,scrollbars=no,resizable=no,location=no,status=no',
    );
  });

  it('saves a new ticket through the legacy modal form and keeps the loading text behavior', async () => {
    const originalConsoleError = console.error;
    const consoleError = vi.spyOn(console, 'error').mockImplementation((...args) => {
      const messageText = String(args[0] ?? '');
      if (
        messageText.includes('changing an uncontrolled input to be controlled') ||
        messageText.includes('changing a controlled input to be uncontrolled')
      ) {
        return;
      }
      originalConsoleError(...args);
    });

    try {
      await act(async () => {
        root!.render(<TicketsPage />);
        await Promise.resolve();
      });

      const newButton = Array.from(container.querySelectorAll('button')).find(
        (button) => button.textContent === '新的工单',
      );
      expect(newButton).toBeDefined();

      await act(async () => {
        newButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await Promise.resolve();
      });

      expect(document.body.innerHTML).toContain('ant-modal-title');
      expect(document.body.innerHTML).toContain('新的工单');
      expect(document.body.innerHTML).toContain('请输入工单主题');
      expect(document.body.innerHTML).toContain('请选择工单等级');
      expect(document.body.innerHTML).toContain('请描述您遇到的问题');

      const subject = document.body.querySelector(
        'input[placeholder="请输入工单主题"]',
      ) as HTMLInputElement;
      const level = document.body.querySelector('.legacy-select-probe') as HTMLSelectElement;
      const message = document.body.querySelector(
        'textarea[placeholder="请描述您遇到的问题"]',
      ) as HTMLTextAreaElement;

      await act(async () => {
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
          subject,
          'Billing question',
        );
        subject.dispatchEvent(new Event('input', { bubbles: true }));
        level.value = '2';
        level.dispatchEvent(new Event('change', { bubbles: true }));
        Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(
          message,
          'Please check my invoice',
        );
        message.dispatchEvent(new Event('input', { bubbles: true }));
        await Promise.resolve();
      });

      const okButton = document.body.querySelector(
        '.ant-modal-footer .ant-btn-primary',
      ) as HTMLButtonElement;
      expect(okButton.textContent).toBe('确 认');

      await act(async () => {
        okButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await Promise.resolve();
      });

      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        subject: 'Billing question',
        level: 2,
        message: 'Please check my invoice',
      });
      expect(mocks.invalidateQueries).toHaveBeenCalledWith({ queryKey: ['user', 'tickets'] });
    } finally {
      consoleError.mockRestore();
    }
  });

  it('keeps new-ticket form data after canceling because only a successful save clears saveData', async () => {
    await act(async () => {
      root!.render(<TicketsPage />);
      await Promise.resolve();
    });

    const newButton = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '新的工单',
    );
    expect(newButton).toBeDefined();

    await act(async () => {
      newButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const subject = document.body.querySelector(
      'input[placeholder="请输入工单主题"]',
    ) as HTMLInputElement;
    const level = document.body.querySelector('.legacy-select-probe') as HTMLSelectElement;
    const message = document.body.querySelector(
      'textarea[placeholder="请描述您遇到的问题"]',
    ) as HTMLTextAreaElement;

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        subject,
        'Still here',
      );
      subject.dispatchEvent(new Event('input', { bubbles: true }));
      level.value = '1';
      level.dispatchEvent(new Event('change', { bubbles: true }));
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(
        message,
        'Keep this draft',
      );
      message.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('.ant-modal-footer .ant-btn')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      newButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(
      document.body.querySelector<HTMLInputElement>('input[placeholder="请输入工单主题"]')!.value,
    ).toBe('Still here');
    expect(document.body.querySelector<HTMLSelectElement>('.legacy-select-probe')!.value).toBe('1');
    expect(
      document.body.querySelector<HTMLTextAreaElement>(
        'textarea[placeholder="请描述您遇到的问题"]',
      )!.value,
    ).toBe('Keep this draft');

    await act(async () => {
      document.body
        .querySelector<HTMLButtonElement>('.ant-modal-footer .ant-btn-primary')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
      subject: 'Still here',
      level: 1,
      message: 'Keep this draft',
    });
  });

  it('clears saveData after a successful new-ticket save while the old modal DOM stays mounted', async () => {
    const originalConsoleError = console.error;
    const consoleError = vi.spyOn(console, 'error').mockImplementation((...args) => {
      const messageText = String(args[0] ?? '');
      if (
        messageText.includes('changing an uncontrolled input to be controlled') ||
        messageText.includes('changing a controlled input to be uncontrolled')
      ) {
        return;
      }
      originalConsoleError(...args);
    });

    try {
      await act(async () => {
        root!.render(<TicketsPage />);
        await Promise.resolve();
      });

      const newButton = Array.from(container.querySelectorAll('button')).find(
        (button) => button.textContent === '新的工单',
      );
      expect(newButton).toBeDefined();

      await act(async () => {
        newButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await Promise.resolve();
      });

      const subject = document.body.querySelector(
        'input[placeholder="请输入工单主题"]',
      ) as HTMLInputElement;
      const level = document.body.querySelector('.legacy-select-probe') as HTMLSelectElement;
      const message = document.body.querySelector(
        'textarea[placeholder="请描述您遇到的问题"]',
      ) as HTMLTextAreaElement;

      await act(async () => {
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
          subject,
          'Saved subject',
        );
        subject.dispatchEvent(new Event('input', { bubbles: true }));
        level.value = '2';
        level.dispatchEvent(new Event('change', { bubbles: true }));
        Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(
          message,
          'Saved body',
        );
        message.dispatchEvent(new Event('input', { bubbles: true }));
        await Promise.resolve();
      });

      await act(async () => {
        document.body
          .querySelector<HTMLButtonElement>('.ant-modal-footer .ant-btn-primary')!
          .dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await Promise.resolve();
      });
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(mocks.saveMutateAsync).toHaveBeenLastCalledWith({
        subject: 'Saved subject',
        level: 2,
        message: 'Saved body',
      });

      await act(async () => {
        newButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await Promise.resolve();
      });

      expect(
        document.body.querySelector<HTMLInputElement>('input[placeholder="请输入工单主题"]')!.value,
      ).toBe('Saved subject');
      expect(document.body.querySelector<HTMLSelectElement>('.legacy-select-probe')!.value).toBe(
        '',
      );
      expect(
        document.body.querySelector<HTMLTextAreaElement>(
          'textarea[placeholder="请描述您遇到的问题"]',
        )!.value,
      ).toBe('Saved body');

      await act(async () => {
        document.body
          .querySelector<HTMLButtonElement>('.ant-modal-footer .ant-btn-primary')!
          .dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await Promise.resolve();
      });
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(mocks.saveMutateAsync).toHaveBeenLastCalledWith({
        subject: undefined,
        level: undefined,
        message: undefined,
      });
    } finally {
      consoleError.mockRestore();
    }
  });

  it('keeps the bundled ticket level select value as the direct payload value', () => {
    expect(source).toContain('const levelLabel = LEVELS[ticket.level]?.labelKey;');
    expect(source).toContain('onChange={(nextLevel) => setLevel(nextLevel as TicketLevel)}');
    expect(source).not.toContain('LEVELS[Number(ticket.level)]');
    expect(source).not.toContain('setLevel(Number(nextLevel) as TicketLevel)');
  });

  it('keeps the bundled new-ticket modal mask closable and save gate props', () => {
    const modalSource = source.slice(
      source.indexOf('<DialogContent'),
      source.indexOf('</DialogContent>', source.indexOf('<DialogContent')),
    );

    expect(modalSource).toContain('title={t(\'ticket.new\')}');
    expect(modalSource).toContain('okText={t(\'ticket.confirm\')}');
    expect(modalSource).toContain('cancelText={t(\'common.cancel\')}');
    expect(modalSource).toContain('maskClosable');
    expect(modalSource).toContain('onOk={() => void saveTicket()}');
  });

  it('closes an open ticket and empties ticket state on unmount like the old model', async () => {
    await act(async () => {
      root!.render(<TicketsPage />);
      await Promise.resolve();
    });

    const closeLinks = Array.from(container.querySelectorAll('a')).filter(
      (link) => link.textContent === '关闭',
    );
    expect(closeLinks).toHaveLength(6);

    await act(async () => {
      closeLinks[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.closeMutateAsync).toHaveBeenCalledWith(7);
    expect(mocks.invalidateQueries).toHaveBeenCalledWith({ queryKey: ['user', 'tickets'] });

    act(() => root?.unmount());
    root = null;

    expect(mocks.removeQueries).toHaveBeenCalledWith({ queryKey: ['user', 'tickets'] });
    expect(mocks.removeQueries).toHaveBeenCalledWith({ queryKey: ['user', 'ticket'] });
  });

  it('keeps the closed-ticket close anchor disabled attribute without suppressing the legacy click', async () => {
    expect(source).toContain("import type { AnchorHTMLAttributes } from 'react';");
    expect(source).toContain('function legacyDisabledAnchorProps(disabled: unknown): AnchorHTMLAttributes<HTMLAnchorElement>');
    expect(source).toContain('return { disabled } as unknown as AnchorHTMLAttributes<HTMLAnchorElement>;');
    expect(source).toContain('{...legacyDisabledAnchorProps(ticket.status)}');
    expect(source).not.toContain("...(ticket.status ? { disabled: true } : {})");

    await act(async () => {
      root!.render(<TicketsPage />);
      await Promise.resolve();
    });

    const closeLinks = Array.from(container.querySelectorAll<HTMLAnchorElement>('a')).filter(
      (link) => link.textContent === '关闭',
    );
    const closedMain = closeLinks[2]!;
    const closedFixed = closeLinks[5]!;

    expect(closedMain.getAttribute('disabled')).toBe('');
    expect(closedFixed.getAttribute('disabled')).toBe('');

    await act(async () => {
      closedMain.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.closeMutateAsync).toHaveBeenCalledWith(9);
  });
});
