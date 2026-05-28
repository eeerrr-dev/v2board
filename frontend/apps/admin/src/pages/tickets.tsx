import { useState } from 'react';
import { App, Button, Card, Drawer, Input, Space, Table, Tag, Typography } from 'antd';
import type { TableProps } from 'antd';
import { useTranslation } from 'react-i18next';
import { useAdminTickets, useCloseTicketMutation, useReplyTicketMutation } from '@/lib/queries';
import type { Ticket } from '@v2board/types';
import { formatDateTime } from '@v2board/config/format';
import { i18nGet } from '@/lib/errors';
import { apiClient } from '@/lib/api';
import { useQuery } from '@tanstack/react-query';

const LEVEL_COLORS: Record<number, string> = { 0: 'default', 1: 'orange', 2: 'red' };
const LEVEL_KEY = ['ticket.level_low', 'ticket.level_medium', 'ticket.level_high'];

export default function TicketsPage() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [query, setQuery] = useState({ current: 1, pageSize: 20 });
  const tickets = useAdminTickets(query);
  const reply = useReplyTicketMutation();
  const close = useCloseTicketMutation();
  const [activeId, setActiveId] = useState<number | null>(null);
  const [replyText, setReplyText] = useState('');

  const detail = useQuery({
    queryKey: ['admin', 'ticket', activeId],
    queryFn: () =>
      apiClient.request<Ticket>({
        url: apiClient.resolveAdminPath('/ticket/fetch'),
        params: { id: activeId },
      }),
    enabled: activeId != null,
  });

  const columns: TableProps<Ticket>['columns'] = [
    { title: t('ticket.col_id'), dataIndex: 'id', width: 80 },
    { title: t('ticket.subject'), dataIndex: 'subject' },
    {
      title: t('ticket.level'),
      dataIndex: 'level',
      render: (v: number) => <Tag color={LEVEL_COLORS[v] ?? 'default'}>{t(LEVEL_KEY[v] ?? LEVEL_KEY[0]!)}</Tag>,
    },
    {
      title: t('ticket.status'),
      dataIndex: 'status',
      render: (v: number) => (
        <Tag color={v === 0 ? 'green' : 'default'}>
          {t(v === 0 ? 'ticket.open' : 'ticket.closed')}
        </Tag>
      ),
    },
    {
      title: t('ticket.created_at_col'),
      dataIndex: 'created_at',
      render: (v: number) => formatDateTime(v),
    },
    {
      title: t('ticket.last_reply'),
      dataIndex: 'updated_at',
      render: (v: number) => formatDateTime(v),
    },
    {
      title: t('common.operation'),
      render: (_: unknown, row) => (
        <Button size="small" onClick={() => setActiveId(row.id)}>
          {t('common.detail')}
        </Button>
      ),
    },
  ];

  const onReply = async () => {
    if (!activeId || !replyText.trim()) return;
    try {
      await reply.mutateAsync({ id: activeId, message: replyText });
      setReplyText('');
      message.success(t('common.success'));
      detail.refetch();
    } catch (e) {
      if (e instanceof Error) message.error(i18nGet(e.message));
    }
  };

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.tickets')}</Typography.Title>
      <Card>
        <Table<Ticket>
          loading={tickets.isLoading}
          rowKey="id"
          dataSource={tickets.data?.data ?? []}
          columns={columns}
          pagination={{
            current: query.current,
            pageSize: query.pageSize,
            total: tickets.data?.total ?? 0,
            onChange: (current, pageSize) => setQuery({ current, pageSize }),
          }}
        />
      </Card>
      <Drawer
        open={activeId != null}
        onClose={() => setActiveId(null)}
        title={detail.data?.subject ?? t('ticket.detail')}
        width={520}
        extra={
          detail.data?.status === 0 && (
            <Button
              danger
              onClick={async () => {
                try {
                  await close.mutateAsync(activeId as number);
                  message.success(t('common.success'));
                  setActiveId(null);
                } catch (e) {
                  if (e instanceof Error) message.error(i18nGet(e.message));
                }
              }}
            >
              {t('ticket.close_ticket')}
            </Button>
          )
        }
      >
        {detail.isLoading ? (
          'Loading...'
        ) : (
          <Space direction="vertical" style={{ width: '100%' }}>
            {detail.data?.message?.map((m) => (
              <Card
                key={m.id}
                size="small"
                style={{
                  marginInlineStart: m.is_me ? 'auto' : undefined,
                  marginInlineEnd: m.is_me ? undefined : 'auto',
                  maxWidth: '85%',
                }}
              >
                <div style={{ whiteSpace: 'pre-wrap' }}>{m.message}</div>
                <div style={{ fontSize: 11, color: '#999', marginTop: 4 }}>
                  {formatDateTime(m.created_at)}
                </div>
              </Card>
            ))}
            {detail.data?.status === 0 && (
              <Space.Compact style={{ width: '100%' }}>
                <Input.TextArea
                  rows={3}
                  value={replyText}
                  onChange={(e) => setReplyText(e.target.value)}
                  placeholder={t('ticket.reply_placeholder')}
                />
                <Button type="primary" onClick={onReply} disabled={!replyText.trim()}>
                  {t('ticket.send')}
                </Button>
              </Space.Compact>
            )}
          </Space>
        )}
      </Drawer>
    </div>
  );
}
