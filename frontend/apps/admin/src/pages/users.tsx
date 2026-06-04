import { useEffect, useMemo, useState, type AnchorHTMLAttributes } from 'react';
import {
  App,
  Badge,
  Button,
  DatePicker,
  Dropdown,
  Input,
  Modal,
  Select,
  Table,
  Tag,
  Tooltip,
} from 'antd';
import type { ButtonProps, TablePaginationConfig, TableProps } from 'antd';
import {
  AccountBookOutlined,
  CaretDownOutlined,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  FileExcelOutlined,
  FilterOutlined,
  LoadingOutlined,
  MailOutlined,
  PlusOutlined,
  ReloadOutlined,
  SelectOutlined,
  SolutionOutlined,
  StopOutlined,
  UserAddOutlined,
  UsergroupAddOutlined,
} from '@ant-design/icons';
import dayjs, { type Dayjs } from 'dayjs';
import { useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import type { AdminFilter } from '@v2board/api-client';
import type { AdminUserRow, PlanPeriod } from '@v2board/types';
import { formatDateMinuteSlash, formatDateTime } from '@v2board/config/format';
import {
  useAdminPlans,
  useAdminUsers,
  useAssignOrderMutation,
  useBanUsersMutation,
  useDeleteAllUsersMutation,
  useDeleteUserMutation,
  useDumpUsersCsvMutation,
  useGenerateUserMutation,
  useResetUserSecretMutation,
  useSendMailToUsersMutation,
  useServerGroups,
} from '@/lib/queries';
import { i18nGet } from '@/lib/errors';
import { legacyCopyText } from '@/lib/legacy-copy';
import { UserManageDrawer } from '@/components/user-manage-drawer';
import { UserTrafficModal } from '@/components/user-traffic-modal';
import { LegacyFilterDrawer, type LegacyFilterKey } from '@/components/legacy-filter-drawer';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';

type QueryState = TablePaginationConfig & {
  current: number;
  pageSize: number;
  filter: AdminFilter[];
  sort?: string;
  sort_type?: 'ASC' | 'DESC';
};

interface PlanOption {
  label: string;
  value: number;
}

interface GenerateUserSubmit {
  email_prefix?: string;
  email_suffix?: string;
  password?: string;
  plan_id?: number | null;
  expired_at?: string | null;
  generate_count?: string;
}

interface SendMailSubmit {
  subject?: string;
  content?: string;
}

interface AssignOrderSubmit {
  email?: string;
  plan_id?: number;
  period?: PlanPeriod;
  total_amount?: string;
}

function assignOrderSubmit(email?: string): AssignOrderSubmit {
  return {
    email: email || undefined,
    plan_id: undefined,
    period: undefined,
    total_amount: undefined,
  };
}

const LEGACY_HABIT_KEY = 'habit';
const LEGACY_USER_PAGE_SIZE_KEY = 'user_manage_page_size';

const PERIOD_TEXT: Record<string, string> = {
  month_price: '月付',
  quarter_price: '季付',
  half_year_price: '半年付',
  year_price: '年付',
  two_year_price: '两年付',
  three_year_price: '三年付',
  onetime_price: '一次性',
  reset_price: '流量重置包',
};

function readLegacyHabit(key: string): unknown {
  if (typeof window === 'undefined') return undefined;
  try {
    const stored = window.localStorage.getItem(LEGACY_HABIT_KEY);
    if (!stored) return undefined;
    const parsed = JSON.parse(stored) as Record<string, unknown>;
    return parsed?.[key];
  } catch {
    return undefined;
  }
}

function writeLegacyHabit(key: string, value: unknown) {
  if (typeof window === 'undefined') return;
  try {
    const stored = window.localStorage.getItem(LEGACY_HABIT_KEY);
    if (stored) {
      const legacyHabit = stored as unknown as Record<string, unknown>;
      legacyHabit[key] = value;
      window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify(legacyHabit));
    } else {
      window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify({ [key]: value }));
    }
  } catch {
    window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify({ [key]: value }));
  }
}

function legacyDisabledAnchorProps(disabled: boolean): AnchorHTMLAttributes<HTMLAnchorElement> {
  return { disabled } as unknown as AnchorHTMLAttributes<HTMLAnchorElement>;
}

function readLegacyUserPageSize() {
  const pageSize = Number(readLegacyHabit(LEGACY_USER_PAGE_SIZE_KEY));
  return Number.isFinite(pageSize) && pageSize > 0 ? pageSize : 10;
}

function readStoredUserFilter(): AdminFilter[] {
  if (typeof window === 'undefined') return [];
  const stored = window.sessionStorage.getItem('v2board-admin-user-filter');
  if (!stored) return [];
  window.sessionStorage.removeItem('v2board-admin-user-filter');
  try {
    return JSON.parse(stored) as AdminFilter[];
  } catch {
    return [];
  }
}

function showError(message: ReturnType<typeof App.useApp>['message'], error: unknown) {
  if (error instanceof Error) message.error(i18nGet(error.message));
}

function downloadText(name: string, buffer: unknown) {
  const blob = new Blob([buffer as BlobPart], { type: 'text/plain,charset=UTF-8' });
  const url = window.URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.style.display = 'none';
  anchor.download = name;
  anchor.click();
  window.URL.revokeObjectURL(url);
}

function downloadGeneratedUserCsv(buffer: unknown) {
  const blob = new Blob([buffer as BlobPart], { type: 'text/plain,charset=UTF-8' });
  const url = window.URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.style.display = 'none';
  anchor.download = `USER ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv`;
  anchor.click();
  window.URL.revokeObjectURL(url);
}

export default function UsersPage() {
  const { message, modal } = App.useApp();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [query, setQuery] = useState<QueryState>(() => ({
    current: 1,
    pageSize: readLegacyUserPageSize(),
    filter: readStoredUserFilter(),
  }));
  const users = useAdminUsers(query);
  const plans = useAdminPlans();
  const groups = useServerGroups();
  const remove = useDeleteUserMutation();
  const resetSecret = useResetUserSecretMutation();
  const generate = useGenerateUserMutation();
  const dumpCsv = useDumpUsersCsvMutation();
  const sendMail = useSendMailToUsersMutation();
  const banUsers = useBanUsersMutation();
  const deleteAll = useDeleteAllUsersMutation();

  const [editing, setEditing] = useState<AdminUserRow | null>(null);
  const [creating, setCreating] = useState(false);
  const [mailOpen, setMailOpen] = useState(false);
  const [assigning, setAssigning] = useState<AdminUserRow | null>(null);
  const [trafficUser, setTrafficUser] = useState<AdminUserRow | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    user: AdminUserRow;
    top: number;
    left: number;
  } | null>(null);

  useEffect(
    () => () => {
      queryClient.removeQueries({ queryKey: ['admin', 'users'] });
      queryClient.removeQueries({ queryKey: ['admin', 'user'] });
    },
    [queryClient],
  );

  const planOptions = useMemo(
    () => plans.data?.map((plan) => ({ label: plan.name, value: plan.id })) ?? [],
    [plans.data],
  );
  const filterPlanOptions = useMemo(
    () => plans.data?.map((plan) => ({ key: plan.name, value: plan.id })) ?? [],
    [plans.data],
  );

  const groupMap = useMemo(() => {
    const map = new Map<number, string>();
    for (const group of groups.data ?? []) map.set(group.id, group.name);
    return map;
  }, [groups.data]);

  const filterKeys = useMemo<LegacyFilterKey[]>(
    () => [
      { key: 'email', title: '邮箱', condition: ['模糊'] },
      { key: 'id', title: '用户ID', condition: ['=', '>=', '>', '<', '<='] },
      {
        key: 'plan_id',
        title: '订阅',
        condition: ['='],
        type: 'select',
        options: [{ key: '无订阅', value: 'null' }, ...filterPlanOptions],
      },
      { key: 'transfer_enable', title: '流量', condition: ['>=', '>', '<', '<='] },
      { key: 'd', title: '下行', condition: ['>=', '>', '<', '<='] },
      { key: 'expired_at', title: '到期时间', condition: ['>=', '>', '<', '<='], type: 'date' },
      { key: 'uuid', title: 'UUID', condition: ['='] },
      { key: 'token', title: 'TOKEN', condition: ['='] },
      {
        key: 'banned',
        title: '账号状态',
        condition: ['='],
        type: 'select',
        options: [
          { key: '正常', value: 0 },
          { key: '封禁', value: 1 },
        ],
      },
      { key: 'invite_by_email', title: '邀请人邮箱', condition: ['模糊'] },
      { key: 'invite_user_id', title: '邀请人ID', condition: ['='] },
      { key: 'remarks', title: '备注', condition: ['模糊'] },
      {
        key: 'is_admin',
        title: '管理员',
        condition: ['='],
        type: 'select',
        options: [
          { key: '是', value: 1 },
          { key: '否', value: 0 },
        ],
      },
    ],
    [filterPlanOptions],
  );

  const setFilter = (filter: AdminFilter[]) =>
    setQuery((state) => ({ ...state, current: 1, filter }));

  const jumpOrderFilter = (key: string, condition: string, value: string | number) => {
    window.sessionStorage.setItem(
      'v2board-admin-order-filter',
      JSON.stringify([{ key, condition, value }]),
    );
    navigate('/order');
  };

  const resetUserSecret = (row: AdminUserRow) =>
    modal.confirm({
      title: '重置安全信息',
      content: `确定要重置${row.email}的安全信息吗？`,
      okText: '确定',
      cancelText: '取消',
      onOk: () => {
        void resetSecret
          .mutateAsync(row.id)
          .then(() => {
            message.success('重置成功');
            void users.refetch();
          })
          .catch((error) => showError(message, error));
      },
    });

  const deleteUser = (row: AdminUserRow) =>
    modal.confirm({
      title: '删除用户',
      content: `确定要删除${row.email}的用户信息吗？`,
      okText: '确定',
      cancelText: '取消',
      onOk: () => {
        void remove
          .mutateAsync(row.id)
          .then(() => {
            message.success('删除成功');
            void users.refetch();
          })
          .catch((error) => showError(message, error));
      },
    });

  const runUserAction = (key: string, row: AdminUserRow) => {
    setContextMenu(null);
    if (key === 'edit') setEditing(row);
    if (key === 'assign') setAssigning(row);
    if (key === 'copy') {
      legacyCopyText(row.subscribe_url);
      message.success('复制成功');
    }
    if (key === 'reset') resetUserSecret(row);
    if (key === 'orders') jumpOrderFilter('user_id', '=', row.id);
    if (key === 'invite') setFilter([{ key: 'invite_user_id', condition: '=', value: row.id }]);
    if (key === 'traffic') setTrafficUser(row);
    if (key === 'delete') deleteUser(row);
  };

  const columns = useMemo<TableProps<AdminUserRow>['columns']>(
    () => [
      { title: 'ID', dataIndex: 'id', key: 'id', sorter: true },
      {
        title: '邮箱',
        dataIndex: 'email',
        key: 'email',
        render: (email: string, row) => {
          const legacyOnlineAt = (row as AdminUserRow & { t?: number | null }).t;
          const online = !(Date.now() / 1000 - 600 > Number(legacyOnlineAt));
          return (
            <Tooltip
              placement="top"
              title={
                legacyOnlineAt ? `最后在线${formatDateTime(Number(legacyOnlineAt))}` : '从未在线'
              }
            >
              <Badge status={online ? 'success' : 'default'} />{email}
            </Tooltip>
          );
        },
      },
      {
        title: '状态',
        dataIndex: 'banned',
        key: 'banned',
        sorter: true,
        render: (banned: 0 | 1) => (
          <Tag color={banned ? 'red' : 'green'}>{banned ? '封禁' : '正常'}</Tag>
        ),
      },
      {
        title: '订阅',
        dataIndex: 'plan_name',
        key: 'plan_id',
        sorter: true,
        render: (value: string | null) => value || '-',
      },
      {
        title: '权限组',
        dataIndex: 'group_id',
        key: 'group_id',
        sorter: true,
        render: (value: number | null) => (value != null ? groupMap.get(value) ?? '-' : '-'),
      },
      {
        title: '已用(G)',
        dataIndex: 'total_used',
        key: 'total_used',
        sorter: true,
        render: (value: number | string, row) => {
          const usedOverLimit =
            parseFloat(String(value)) > parseFloat(String(row.transfer_enable));
          return (
            <Tag color={usedOverLimit ? 'red' : 'green'}>
              {value}
            </Tag>
          );
        },
      },
      {
        title: '流量(G)',
        dataIndex: 'transfer_enable',
        key: 'transfer_enable',
        sorter: true,
      },
      {
        title: '设备数',
        dataIndex: 'device_limit',
        key: 'updated_at',
        sorter: (a, b) => (a.alive_ip as number) - (b.alive_ip as number),
        render: (value: number | null, row) => {
          const deviceCount = row.alive_ip !== null ? row.alive_ip : 0;
          const deviceLimit = row.device_limit !== null ? row.device_limit : '∞';
          const text = `${deviceCount} / ${deviceLimit}`;
          return row.ips ? (
            <Tooltip placement="top" title={row.ips}>
              {text}
            </Tooltip>
          ) : (
            text
          );
        },
      },
      {
        title: '到期时间',
        dataIndex: 'expired_at',
        key: 'expired_at',
        sorter: true,
        render: (value: number | null) => (
          <Tag color={value !== null && value < Date.now() / 1000 ? 'red' : 'green'}>
            {value ? formatDateMinuteSlash(value) : value === null ? '长期有效' : '-'}
          </Tag>
        ),
      },
      { title: '余额', dataIndex: 'balance', key: 'balance', sorter: true },
      {
        title: '佣金',
        dataIndex: 'commission_balance',
        key: 'commission_balance',
        sorter: true,
      },
      {
        title: '加入时间',
        dataIndex: 'created_at',
        key: 'created_at',
        sorter: true,
        render: (value: number) => formatDateMinuteSlash(value),
      },
      {
        title: '操作',
        dataIndex: 'action',
        key: 'action',
        align: 'right',
        fixed: 'right',
        render: (_: unknown, row) => (
          <Dropdown
            trigger={['click']}
            menu={{
              items: [
                { key: 'edit', label: <a><EditOutlined /> 编辑</a> },
                { key: 'assign', label: <a><PlusOutlined /> 分配订单</a> },
                { key: 'copy', label: <a><CopyOutlined /> 复制订阅URL</a> },
                { key: 'reset', label: <a><ReloadOutlined /> 重置UUID及订阅URL</a> },
                { key: 'orders', label: <a><AccountBookOutlined /> TA的订单</a> },
                { key: 'invite', label: <a><UsergroupAddOutlined /> TA的邀请</a> },
                { key: 'traffic', label: <a><SolutionOutlined /> TA的流量记录</a> },
                { key: 'delete', label: <a><DeleteOutlined /> 删除用户</a> },
              ],
              onClick: ({ key }) => {
                runUserAction(String(key), row);
              },
            }}
          >
            <a ref={legacyHref()}>
              操作 <CaretDownOutlined />
            </a>
          </Dropdown>
        ),
      },
    ],
    [groupMap, runUserAction],
  );

  return (
    <>
      <LegacySpin loading={users.isFetching}>
        <div className="block border-bottom">
          <div className="bg-white">
            <div className="v2board-table-action" style={{ padding: 15 }}>
              <Tooltip title="Tips：可以使用过滤器过滤后再使用操作对过滤的用户进行操作。" placement="right">
                <Button.Group>
                  <LegacyFilterDrawer
                    key={query.filter.length}
                    value={query.filter}
                    keys={filterKeys}
                    onChange={setFilter}
                  >
                    <Button type={query.filter.length > 0 ? 'primary' : ('' as ButtonProps['type'])}>
                      <FilterOutlined /> 过滤器
                    </Button>
                  </LegacyFilterDrawer>
                  <Dropdown
                    menu={{
                      items: [
                        { key: 'csv', label: <a><FileExcelOutlined /> 导出CSV</a> },
                        { key: 'mail', label: <a><MailOutlined /> 发送邮件</a> },
                        {
                          key: 'ban',
                          label: <a {...legacyDisabledAnchorProps(!query.filter.length)}><StopOutlined /> 批量封禁</a>,
                          disabled: !query.filter.length,
                        },
                        {
                          key: 'delete',
                          label: <a {...legacyDisabledAnchorProps(!query.filter.length)}><DeleteOutlined /> 批量删除</a>,
                          disabled: !query.filter.length,
                        },
                      ],
                      onClick: ({ key }) => {
                        if (key === 'csv') {
                          message.loading('导出中');
                          void dumpCsv
                            .mutateAsync(query.filter)
                            .then((response) => {
                              message.destroy();
                              downloadText(`${formatDateTime(Date.now() / 1000)}.csv`, response.buffer);
                            })
                            .catch((error) => {
                              message.destroy();
                              showError(message, error);
                            });
                        }
                        if (key === 'mail') setMailOpen(true);
                        if (key === 'ban') {
                          modal.confirm({
                            title: '提醒',
                            content: '确定要进行封禁吗？',
                            onOk: () => {
                              void banUsers
                                .mutateAsync(query.filter)
                                .then(() => {
                                  void users.refetch();
                                })
                                .catch((error) => showError(message, error));
                            },
                          });
                        }
                        if (key === 'delete') {
                          modal.confirm({
                            title: '提醒',
                            content: '确定要进行删除吗？',
                            onOk: () => {
                              void deleteAll
                                .mutateAsync(query.filter)
                                .then(() => {
                                  void users.refetch();
                                })
                                .catch((error) => showError(message, error));
                            },
                          });
                        }
                      },
                    }}
                  >
                    <Button>
                      <SelectOutlined />
                      操作
                    </Button>
                  </Dropdown>
                </Button.Group>
              </Tooltip>
              <span className="float-right">
                <Button className="ml-2" onClick={() => setCreating(true)}>
                  <UserAddOutlined />
                </Button>
              </span>
            </div>
            <Table<AdminUserRow>
              className="v2board-table"
              tableLayout="auto"
              dataSource={users.data?.data ?? []}
              pagination={{
                current: query.current,
                pageSize: query.pageSize,
                total: users.data?.total,
                size: 'small',
                showSizeChanger: true,
                pageSizeOptions: [10, 50, 100, 150],
              }}
              columns={columns}
              scroll={{ x: 1500 }}
              onRow={(record) => ({
                onClick: () => setContextMenu(null),
                onContextMenu: (event) => {
                  event.preventDefault();
                  setContextMenu({ user: record, top: event.clientY, left: event.clientX });
                },
              })}
              onChange={(pagination: TablePaginationConfig, _filters, sorter) => {
                const singleSorter = Array.isArray(sorter) ? sorter[0] : sorter;
                setQuery((state) => {
                  writeLegacyHabit(LEGACY_USER_PAGE_SIZE_KEY, pagination.pageSize);
                  return {
                    ...state,
                    ...pagination,
                    sort: typeof singleSorter?.columnKey === 'string' ? singleSorter.columnKey : undefined,
                    sort_type: singleSorter?.order === 'ascend' ? 'ASC' : 'DESC',
                  } as QueryState;
                });
              }}
            />
            <div
              id="v2board-table-dropdown"
              className="ant-dropdown ant-dropdown-placement-bottomLeft"
              style={{
                display: contextMenu ? 'unset' : 'none',
                position: 'fixed',
                top: contextMenu?.top ?? 0,
                left: contextMenu?.left ?? 0,
              }}
              onClick={() => setContextMenu(null)}
            >
              <ul className="ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical">
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('edit', contextMenu.user)}>
                    <EditOutlined /> 编辑
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('assign', contextMenu.user)}>
                    <PlusOutlined /> 分配订单
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('copy', contextMenu.user)}>
                    <CopyOutlined /> 复制订阅URL
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a
                    style={{ color: '#ff4d4f' }}
                    onClick={() => contextMenu && runUserAction('reset', contextMenu.user)}
                  >
                    <ReloadOutlined /> 重置UUID及订阅URL
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('orders', contextMenu.user)}>
                    <AccountBookOutlined /> TA的订单
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('invite', contextMenu.user)}>
                    <UsergroupAddOutlined /> TA的邀请
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('traffic', contextMenu.user)}>
                    <SolutionOutlined /> TA的流量记录
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('delete', contextMenu.user)}>
                    <DeleteOutlined /> 删除用户
                  </a>
                </li>
              </ul>
            </div>
          </div>
        </div>
      </LegacySpin>

      <UserManageDrawer
        userId={editing?.id}
        open={editing != null}
        onClose={() => setEditing(null)}
        onSaved={() => users.refetch()}
      />

      <GenerateUserModal
        open={creating}
        plans={planOptions}
        loading={generate.isPending}
        onClose={() => setCreating(false)}
        onSubmit={(values) =>
          generate
            .mutateAsync(values as Parameters<typeof generate.mutateAsync>[0])
            .then(async (response) => {
              if (values.generate_count) downloadGeneratedUserCsv(response.buffer);
              await users.refetch();
              setCreating(false);
            })
            .catch((error) => showError(message, error))
        }
      />

      <SendMailModal
        open={mailOpen}
        filter={query.filter}
        loading={sendMail.isPending}
        onClose={() => setMailOpen(false)}
        onSubmit={(values) =>
          sendMail
            .mutateAsync({ filter: query.filter, ...values })
            .then(() => {
              message.success('已加入队列执行');
              setMailOpen(false);
            })
            .catch((error) => showError(message, error))
        }
      />

      <AssignOrderModal
        user={assigning}
        plans={planOptions}
        onClose={() => setAssigning(null)}
      />

      <UserTrafficModal
        userId={trafficUser?.id}
        open={trafficUser != null}
        onClose={() => setTrafficUser(null)}
      />
    </>
  );
}

function GenerateUserModal({
  open,
  plans,
  loading,
  onClose,
  onSubmit,
}: {
  open: boolean;
  plans: PlanOption[];
  loading: boolean;
  onClose: () => void;
  onSubmit: (values: GenerateUserSubmit) => Promise<void>;
}) {
  const [submit, setSubmit] = useState<GenerateUserSubmit>({});
  useEffect(() => {
    if (!open) setSubmit({});
  }, [open]);

  const close = () => {
    setSubmit({});
    onClose();
  };

  const setSubmitField = <K extends keyof GenerateUserSubmit>(
    key: K,
    value: GenerateUserSubmit[K],
  ) => {
    setSubmit((state) => ({ ...state, [key]: value }));
  };

  const onDateChange = (date: Dayjs | null) => {
    setSubmitField('expired_at', date ? date.format('X') : null);
  };

  return (
    <Modal
      open={open}
      onCancel={close}
      title="创建用户"
      cancelText="取消"
      okText="生成"
      okButtonProps={{ loading }}
      onOk={() => onSubmit({ ...submit })}
    >
      <div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">邮箱</label>
          <Input.Group compact>
            {!submit.generate_count && (
              <Input
                placeholder="账号（批量生成请留空）"
                style={{ width: '45%' }}
                value={submit.email_prefix}
                onChange={(event) => setSubmitField('email_prefix', event.target.value)}
              />
            )}
            <Input
              placeholder="@"
              style={{ width: '10%', textAlign: 'center' }}
              disabled
            />
            <Input
              placeholder="域"
              style={{ width: '45%' }}
              value={submit.email_suffix}
              onChange={(event) => setSubmitField('email_suffix', event.target.value)}
            />
          </Input.Group>
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">密码</label>
          <Input
            value={submit.password}
            placeholder="留空则密码与邮箱相同"
            onChange={(event) => setSubmitField('password', event.target.value)}
          />
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">到期时间</label>
          <div>
            <DatePicker
              placeholder="请选择用户到期日期，为空则不限制到期时间"
              defaultValue={submit.expired_at ? dayjs(1000 * Number(submit.expired_at)) : undefined}
              style={{ width: '100%' }}
              onChange={onDateChange}
            />
          </div>
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">订阅计划</label>
          <Select
            placeholder="请选择用户订阅计划"
            style={{ width: '100%' }}
            value={submit.plan_id || null}
            onChange={(planId) => setSubmitField('plan_id', planId)}
          >
            <Select.Option value={null}>无</Select.Option>
            {plans.map((plan) => (
              <Select.Option key={Math.random()} value={plan.value}>
                {plan.label}
              </Select.Option>
            ))}
          </Select>
        </div>
        {!submit.email_prefix && (
          <div className="form-group">
            <label htmlFor="example-text-input-alt">生成数量</label>
            <Input
              value={submit.generate_count}
              placeholder="如果为批量生成请输入生成数量"
              onChange={(event) => setSubmitField('generate_count', event.target.value)}
            />
          </div>
        )}
      </div>
    </Modal>
  );
}

function SendMailModal({
  open,
  filter,
  loading,
  onClose,
  onSubmit,
}: {
  open: boolean;
  filter: AdminFilter[];
  loading: boolean;
  onClose: () => void;
  onSubmit: (values: SendMailSubmit) => Promise<void>;
}) {
  const [submit, setSubmit] = useState<SendMailSubmit>({});
  return (
    <Modal
      open={open}
      title="发送邮件"
      onOk={() => onSubmit(submit)}
      okButtonProps={{ loading }}
      onCancel={onClose}
    >
      <div className="form-group">
        <label htmlFor="example-text-input-alt">收件人</label>
        <Input
          disabled
          value={filter.length ? '过滤用户' : '全部用户'}
        />
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">主题</label>
        <Input
          placeholder="请输入邮件主题"
          value={submit.subject}
          onChange={(event) =>
            setSubmit((state) => ({ ...state, subject: event.target.value }))
          }
        />
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">发送内容</label>
        <Input.TextArea
          rows={12}
          value={submit.content}
          placeholder="请输入邮件内容"
          onChange={(event) =>
            setSubmit((state) => ({ ...state, content: event.target.value }))
          }
        />
      </div>
    </Modal>
  );
}

function AssignOrderModal({
  user,
  plans,
  onClose,
}: {
  user: AdminUserRow | null;
  plans: PlanOption[];
  onClose: () => void;
}) {
  const { message } = App.useApp();
  const assign = useAssignOrderMutation();
  const [submit, setSubmit] = useState<AssignOrderSubmit>(() => assignOrderSubmit());
  useEffect(() => {
    if (user) {
      setSubmit(assignOrderSubmit(user.email));
    }
  }, [user]);

  const close = () => {
    setSubmit(assignOrderSubmit(user?.email));
    onClose();
  };

  const setSubmitField = <K extends keyof typeof submit>(key: K, value: (typeof submit)[K]) => {
    setSubmit((state) => ({ ...state, [key]: value }));
  };

  return (
    <Modal
      title="订单分配"
      open={Boolean(user)}
      onCancel={close}
      okText={assign.isPending ? <LoadingOutlined /> : '确定'}
      cancelText="取消"
      onOk={() => {
        assign
          .mutateAsync(submit)
          .then(close)
          .catch((error) => showError(message, error));
      }}
    >
      <div className="form-group">
        <label htmlFor="example-text-input-alt">用户邮箱</label>
        <Input
          placeholder="请输入用户邮箱"
          value={submit.email}
          onChange={(event) => setSubmitField('email', event.target.value)}
        />
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">请选择订阅</label>
        <div>
          <Select
            value={submit.plan_id}
            style={{ width: '100%' }}
            placeholder="请选择订阅"
            onChange={(plan_id) => setSubmitField('plan_id', plan_id)}
          >
            {plans.map((plan) => (
              <Select.Option value={plan.value} key={Math.random()}>
                {plan.label}
              </Select.Option>
            ))}
          </Select>
        </div>
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">请选择周期</label>
        <div>
          <Select
            value={submit.period}
            style={{ width: '100%' }}
            placeholder="请选择周期"
            onChange={(period) => setSubmitField('period', period)}
          >
            {Object.keys(PERIOD_TEXT).map((period) => (
              <Select.Option value={period} key={Math.random()}>
                {PERIOD_TEXT[period]}
              </Select.Option>
            ))}
          </Select>
        </div>
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">支付金额</label>
        <Input
          placeholder="请输入需要支付的金额"
          addonAfter="¥"
          value={submit.total_amount}
          onChange={(event) => setSubmitField('total_amount', event.target.value)}
        />
      </div>
    </Modal>
  );
}
