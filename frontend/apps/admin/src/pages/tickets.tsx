import { useEffect, useRef, useState, type AnchorHTMLAttributes } from 'react';
import { createPortal } from 'react-dom';
import { App } from 'antd';
import type { FilterValue } from 'antd/es/table/interface';
import dayjs from 'dayjs';
import type { Ticket } from '@v2board/types';
import { useParams } from 'react-router-dom';
import {
  useAdminTicket,
  useAdminTickets,
  useAdminUserInfo,
  useCloseTicketMutation,
  useReplyTicketMutation,
} from '@/lib/queries';
import type { AdminPageQuery } from '@v2board/api-client';
import { UserManageDrawer } from '@/components/user-manage-drawer';
import { UserTrafficModal } from '@/components/user-traffic-modal';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';
import { LegacyFilterIcon, LegacySolutionIcon, LegacyUserIcon } from '@/components/legacy-ant-icon';
import {
  LegacyStandaloneTable,
  LegacyTablePagination,
  legacyTableRowKey,
  type LegacyStandaloneTableHeader,
  type LegacyTablePaginationChange,
} from '@/components/legacy-standalone-table';
import { LegacyTooltip } from '@/components/legacy-tooltip';
import { LegacyRadio } from '@/components/legacy-radio';
import { LegacyCheckbox } from '@/components/legacy-checkbox';
import { LegacyBadge } from '@/components/legacy-badge';
import { LegacyDivider } from '@/components/legacy-divider';

type TicketQuery = AdminPageQuery & {
  total?: number;
  status?: number;
  email?: string;
  reply_status?: FilterValue | null;
};

function legacyDisabledAnchorProps(disabled: unknown): AnchorHTMLAttributes<HTMLAnchorElement> {
  return { disabled } as unknown as AnchorHTMLAttributes<HTMLAnchorElement>;
}

function formatMinute(value: number) {
  return dayjs(1000 * value).format('YYYY/MM/DD HH:mm');
}

function ticketLevelName(value: number) {
  return ['低', '中', '高'][value];
}

function filterValueIncludes(value: FilterValue | null | undefined, option: number) {
  return Array.isArray(value) && value.includes(option);
}

function LegacyTicketReplyStatusFilterDropdown({
  open,
  value,
  onClear,
  onConfirm,
  onToggle,
}: {
  open: boolean;
  value: FilterValue | null | undefined;
  onClear: () => void;
  onConfirm: () => void;
  onToggle: (value: number) => void;
}) {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted || typeof document === 'undefined') return null;

  return createPortal(
    <div
      className={`ant-dropdown  ant-dropdown-placement-bottomRight${open ? '' : ' ant-dropdown-hidden'}`}
    >
      <div className="ant-table-filter-dropdown">
        <ul
          className="ant-dropdown-menu ant-dropdown-menu-without-submenu ant-dropdown-menu-root ant-dropdown-menu-vertical"
          role="menu"
          tabIndex={0}
        >
          {[
            { label: '已回复', value: 1 },
            { label: '待回复', value: 0 },
          ].map((item) => {
            const checked = filterValueIncludes(value, item.value);
            return (
              <li key={item.value} className="ant-dropdown-menu-item" role="menuitem">
                <LegacyCheckbox
                  checked={checked}
                  value=""
                  onChange={() => onToggle(item.value)}
                />
                <span>{item.label}</span>
              </li>
            );
          })}
        </ul>
        <div className="ant-table-filter-dropdown-btns">
          <a className="ant-table-filter-dropdown-link confirm" onClick={onConfirm}>
            确定
          </a>
          <a className="ant-table-filter-dropdown-link clear" onClick={onClear}>
            重置
          </a>
        </div>
      </div>
    </div>,
    document.body,
  );
}

export function startLegacyTicketPolling(refetch: () => unknown) {
  let timer: number | undefined;
  const check = () => {
    timer = window.setTimeout(() => {
      void refetch();
      check();
    }, 5000);
  };
  check();
  return () => {
    if (timer !== undefined) window.clearTimeout(timer);
  };
}

export default function TicketsPage() {
  const { ticket_id: ticketId } = useParams();
  if (ticketId) return <TicketChatPage ticketId={ticketId} />;
  return <TicketListPage />;
}

function TicketListPage() {
  const [query, setQuery] = useState<TicketQuery>({ current: 1, pageSize: 10, status: 0 });
  const tickets = useAdminTickets(query);
  const closeTicket = useCloseTicketMutation();
  const searchTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const data = tickets.data?.data ?? [];
  const [replyStatusFilterOpen, setReplyStatusFilterOpen] = useState(false);
  const [replyStatusFilterValue, setReplyStatusFilterValue] = useState<FilterValue | null>(
    query.reply_status ?? null,
  );

  const filter = (key: keyof TicketQuery, value: TicketQuery[keyof TicketQuery]) => {
    setQuery((current) => ({
      ...current,
      current: 1,
      pageSize: 10,
      [key]: value,
    }));
  };

  const onSearch = (key: keyof TicketQuery, value: string) => {
    clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => filter(key, value), 300);
  };

  const toggleReplyStatusFilterValue = (value: number) => {
    setReplyStatusFilterValue((current) => {
      const values = Array.isArray(current) ? current : [];
      const next = filterValueIncludes(values, value)
        ? values.filter((item) => item !== value)
        : [...values, value];
      return next.length ? next : null;
    });
  };

  const confirmReplyStatusFilter = () => {
    setReplyStatusFilterOpen(false);
    filter('reply_status', replyStatusFilterValue);
  };

  const clearReplyStatusFilter = () => {
    setReplyStatusFilterOpen(false);
    setReplyStatusFilterValue(null);
    filter('reply_status', null);
  };

  const updateTablePagination = (pagination: LegacyTablePaginationChange) =>
    setQuery((current) => ({
      ...current,
      ...pagination,
    }));

  const toChat = (id: number) => {
    const url = `${window.location.origin}${window.location.pathname}#/ticket/${id}`;
    const userAgent = window.navigator.userAgent.toLowerCase();
    if (!userAgent.includes('mobile') && !userAgent.includes('ipad')) {
      window.open(
        url,
        '_blank',
        'height=600,width=800,top=0,left=0,toolbar=no,menubar=no,scrollbars=no,resizable=no,location=no,status=no',
      );
      return;
    }

    window.location.href = url;
  };

  const headers: LegacyStandaloneTableHeader[] = [
    { title: '#' },
    { title: '主题' },
    { title: '工单级别' },
    {
      title: '工单状态',
      className:
        query.status !== 1
          ? 'ant-table-column-has-actions ant-table-column-has-filters'
          : undefined,
      suffix:
        query.status !== 1 ? (
          <LegacyFilterIcon
            filled
            title="筛选"
            tabIndex={-1}
            className="ant-dropdown-trigger"
            onClick={() => setReplyStatusFilterOpen((current) => !current)}
          />
        ) : undefined,
    },
    { title: '创建时间' },
    { title: '最后回复' },
    { title: '操作', alignRight: true, fixedRight: true },
  ];

  const renderTicketStatus = (value: 0 | 1, row: Ticket) =>
    row.status === 1 ? (
      <LegacyBadge status="success" text="已关闭" />
    ) : (
      <LegacyBadge status={value ? 'processing' : 'error'} text={value ? '已回复' : '待回复'} />
    );

  const renderTicketActions = (row: Ticket) => (
    <div>
      <a ref={legacyHref()} onClick={() => toChat(row.id)}>
        查看
      </a>
      <LegacyDivider type="vertical" />
      <a
        {...legacyDisabledAnchorProps(row.status)}
        ref={legacyHref()}
        onClick={() =>
          closeTicket.mutate(row.id, {
            onSuccess: () => {
              void tickets.refetch();
            },
          })
        }
      >
        关闭
      </a>
    </div>
  );

  return (
    <LegacySpin loading={tickets.isFetching}>
      <div className="block border-bottom">
        <div className="bg-white">
          <div className="p-3">
            <LegacyRadio.Group
              value={query.status}
              onChange={(event) => filter('status', Number(event.target.value))}
            >
              <LegacyRadio.Button value={0}>已开启</LegacyRadio.Button>
              <LegacyRadio.Button value={1}>已关闭</LegacyRadio.Button>
            </LegacyRadio.Group>
            <div style={{ float: 'right' }}>
              <input
                placeholder="输入邮箱搜索"
                type="text"
                className="ant-input"
                defaultValue=""
                onChange={(event) => onSearch('email', event.target.value)}
              />
            </div>
          </div>
          <LegacyStandaloneTable
            headers={headers}
            isEmpty={data.length === 0}
            scrollX={900}
            scrollPositionRight="desktop"
            pagination={
              <LegacyTablePagination
                current={query.current ?? 1}
                pageSize={query.pageSize ?? 10}
                total={tickets.data?.total}
                onChange={updateTablePagination}
              />
            }
            fixedRightChildren={data.map((row, index) => (
              <tr
                key={index}
                className="ant-table-row ant-table-row-level-0"
                {...legacyTableRowKey(index)}
              >
                <td
                  className="ant-table-align-right ant-table-row-cell-last"
                  style={{ textAlign: 'right' }}
                >
                  {renderTicketActions(row)}
                </td>
              </tr>
            ))}
          >
            {data.map((row, index) => (
              <tr
                key={index}
                className="ant-table-row ant-table-row-level-0"
                {...legacyTableRowKey(index)}
              >
                <td className="">{row.id}</td>
                <td className="">{row.subject}</td>
                <td className="">{ticketLevelName(row.level)}</td>
                <td className="">{renderTicketStatus(row.reply_status, row)}</td>
                <td className="">{formatMinute(row.created_at)}</td>
                <td className="">{formatMinute(row.updated_at)}</td>
                <td
                  className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"
                  style={{ textAlign: 'right' }}
                >
                  {renderTicketActions(row)}
                </td>
              </tr>
            ))}
          </LegacyStandaloneTable>
          {query.status !== 1 ? (
            <LegacyTicketReplyStatusFilterDropdown
              open={replyStatusFilterOpen}
              value={replyStatusFilterValue}
              onToggle={toggleReplyStatusFilterValue}
              onConfirm={confirmReplyStatusFilter}
              onClear={clearReplyStatusFilter}
            />
          ) : null}
        </div>
      </div>
    </LegacySpin>
  );
}

function TicketChatPage({ ticketId }: { ticketId: string }) {
  const { message: messageApi } = App.useApp();
  const ticket = useAdminTicket(ticketId);
  const reply = useReplyTicketMutation();
  const [message, setMessage] = useState<string | undefined>(undefined);
  const [userOpen, setUserOpen] = useState(false);
  const [trafficOpen, setTrafficOpen] = useState(false);
  const chatRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const messageCount = ticket.data?.message!.length;

  useEffect(() => {
    const chat = chatRef.current;
    if (!chat) return;
    chat.scrollTo(0, chat.scrollHeight);
  }, [messageCount]);

  useEffect(() => {
    return startLegacyTicketPolling(ticket.refetch);
  }, [ticket.refetch]);

  const sendReply = async () => {
    if (reply.isPending) return;
    const replyMessageKey = 'v2board-admin-ticket-reply';
    const closeReplyMessage = messageApi.open({
      content: '发送中',
      duration: 0,
      key: replyMessageKey,
      type: 'loading',
    });
    try {
      await reply.mutateAsync({ id: ticketId, message });
    } finally {
      closeReplyMessage();
    }
    await ticket.refetch();
    if (inputRef.current) inputRef.current.value = '';
  };

  const current = ticket.data;
  const emptyNotice = current ? undefined : ticket.isError ? '工单不存在' : '加载中...';
  useAdminUserInfo(current?.user_id);

  return (
    <div>
      <div className="block-content-full bg-gray-lighter p-3">
        <span className="tag___12_9H">{current?.subject ?? emptyNotice}</span>
        <div className="ctrl___UqDJ7">
          <LegacyTooltip title="用户管理" placement="left">
            <LegacyUserIcon onClick={() => current?.user_id && setUserOpen(true)} />
          </LegacyTooltip>
          <LegacyDivider type="vertical" />
          <LegacyTooltip title="TA的流量记录" placement="left">
            <LegacySolutionIcon onClick={() => current?.user_id && setTrafficOpen(true)} />
          </LegacyTooltip>
        </div>
      </div>
      <div
        className="bg-white js-chat-messages block-content block-content-full text-wrap-break-word overflow-y-auto content___DW5w1"
        ref={chatRef}
      >
        {current?.message!.map((item, index) =>
          item.is_me ? (
            <div key={index}>
              <div className="font-size-sm text-muted my-2 text-right">
                {formatMinute(item.created_at)}
              </div>
              <div className="text-right ml-4">
                <div className="d-inline-block bg-gray-lighter px-3 py-2 mb-2 mw-100 rounded text-left">
                  {item.message}
                </div>
              </div>
            </div>
          ) : (
            <div key={index}>
              <div className="font-size-sm text-muted my-2">{formatMinute(item.created_at)}</div>
              <div className="mr-4">
                <div className="d-inline-block bg-success-lighter px-3 py-2 mb-2 mw-100 rounded text-left">
                  {item.message}
                </div>
              </div>
            </div>
          ),
        )}
        {emptyNotice ? (
          <div className="font-size-sm text-muted my-2 text-center">{emptyNotice}</div>
        ) : null}
      </div>
      <div className="js-chat-form block-content p-2 bg-body-dark input___1j_ND">
        <input
          ref={inputRef}
          type="text"
          className="js-chat-input bg-body-dark border-0 form-control form-control-alt"
          placeholder="输入内容回复工单..."
          onChange={(event) => setMessage(event.target.value)}
          onKeyDown={(event) => {
            if (event.keyCode === 13) void sendReply();
          }}
        />
      </div>
      <UserTrafficModal
        key={current?.user_id}
        userId={current?.user_id}
        open={trafficOpen}
        onClose={() => setTrafficOpen(false)}
      />
      <UserManageDrawer
        userId={current?.user_id}
        open={userOpen}
        onClose={() => setUserOpen(false)}
      />
    </div>
  );
}
