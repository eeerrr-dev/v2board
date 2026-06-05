import { useEffect, useRef, useState, type AnchorHTMLAttributes } from 'react';
import { App, Badge, Empty, Input, Radio, Table, Tooltip } from 'antd';
import type { TablePaginationConfig, TableProps } from 'antd';
import type { ColumnType, FilterValue } from 'antd/es/table/interface';
import dayjs from 'dayjs';
import type { Ticket } from '@v2board/types';
import { SolutionOutlined, UserOutlined } from '@ant-design/icons';
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
  const levels = ['低', '中', '高'];

  const filter = (key: keyof TicketQuery, value: TicketQuery[keyof TicketQuery]) => {
    setQuery((current) => ({
      ...current,
      [key]: value,
      current: 1,
      pageSize: 10,
    }));
  };

  const onSearch = (key: keyof TicketQuery, value: string) => {
    clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => filter(key, value), 300);
  };

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

  const columns: TableProps<Ticket>['columns'] = [
    {
      title: '#',
      dataIndex: 'id',
      key: 'id',
    },
    {
      title: '主题',
      dataIndex: 'subject',
      key: 'subject',
    },
    {
      title: '工单级别',
      dataIndex: 'level',
      key: 'level',
      render: (value: number) => levels[value],
    },
    {
      title: '工单状态',
      dataIndex: 'reply_status',
      key: 'reply_status',
      filters: (query.status !== 1 && [
        { text: '已回复', value: 1 },
        { text: '待回复', value: 0 },
      ]) as ColumnType<Ticket>['filters'],
      render: (value: 0 | 1, row) =>
        row.status === 1 ? (
          <span>
            <Badge status="success" /> 已关闭
          </span>
        ) : (
          <span>
            <Badge status={value ? 'processing' : 'error'} /> {value ? '已回复' : '待回复'}
          </span>
        ),
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      render: (value: number) => formatMinute(value),
    },
    {
      title: '最后回复',
      dataIndex: 'updated_at',
      key: 'updated_at',
      render: (value: number) => formatMinute(value),
    },
    {
      title: '操作',
      dataIndex: 'action',
      key: 'action',
      align: 'right',
      fixed: 'right',
      render: (_value, row) => (
        <div>
          <a ref={legacyHref()} onClick={() => toChat(row.id)}>
            查看
          </a>
          <div className="ant-divider ant-divider-vertical" />
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
      ),
    },
  ];

  return (
    <LegacySpin loading={tickets.isFetching}>
      <div className="block border-bottom">
        <div className="bg-white">
          <div className="p-3">
            <Radio.Group value={query.status} onChange={(event) => filter('status', event.target.value)}>
              <Radio.Button value={0}>已开启</Radio.Button>
              <Radio.Button value={1}>已关闭</Radio.Button>
            </Radio.Group>
            <div style={{ float: 'right' }}>
              <Input
                placeholder="输入邮箱搜索"
                onChange={(event) => onSearch('email', event.target.value)}
              />
            </div>
          </div>
          <Table<Ticket>
            tableLayout="auto"
            dataSource={tickets.data?.data ?? []}
            pagination={{
              current: query.current,
              pageSize: query.pageSize,
              total: tickets.data?.total,
              size: 'small',
            }}
            columns={columns}
            scroll={{ x: 900 }}
            onChange={(
              pagination: TablePaginationConfig,
              filters: Record<string, FilterValue | null>,
            ) => {
              setQuery((current) => ({
                ...current,
                ...pagination,
                ...filters,
              }));
            }}
          />
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
    messageApi.loading('发送中');
    try {
      await reply.mutateAsync({ id: ticketId, message });
    } finally {
      messageApi.destroy();
    }
    void ticket.refetch();
    if (inputRef.current) inputRef.current.value = '';
  };

  const current = ticket.data;
  useAdminUserInfo(current?.user_id);

  if (ticket.isError && !current) {
    return (
      <div className="bg-white js-chat-messages block-content block-content-full text-wrap-break-word overflow-y-auto content___DW5w1">
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无数据" />
      </div>
    );
  }

  return (
    <div>
      <div className="block-content-full bg-gray-lighter p-3">
        <span className="tag___12_9H">{current?.subject}</span>
        <div className="ctrl___UqDJ7">
          <Tooltip title="用户管理" placement="left">
            <UserOutlined onClick={() => current?.user_id && setUserOpen(true)} />
          </Tooltip>
          <div className="ant-divider ant-divider-vertical" />
          <Tooltip title="TA的流量记录" placement="left">
            <SolutionOutlined onClick={() => current?.user_id && setTrafficOpen(true)} />
          </Tooltip>
        </div>
      </div>
      <div
        className="bg-white js-chat-messages block-content block-content-full text-wrap-break-word overflow-y-auto content___DW5w1"
        ref={chatRef}
      >
        {current?.message!.map((item) =>
          item.is_me ? (
            <div>
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
            <div>
              <div className="font-size-sm text-muted my-2">{formatMinute(item.created_at)}</div>
              <div className="mr-4">
                <div className="d-inline-block bg-success-lighter px-3 py-2 mb-2 mw-100 rounded text-left">
                  {item.message}
                </div>
              </div>
            </div>
          ),
        )}
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
