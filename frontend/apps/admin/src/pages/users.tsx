import { useEffect, useMemo, useState, type AnchorHTMLAttributes } from 'react';
import { App } from 'antd';
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
import { legacyFetchLoading } from '@/lib/legacy-fetch-loading';
import { LegacyButton } from '@/components/legacy-button';
import { LegacyDatePicker } from '@/components/legacy-date-picker';
import {
  LegacyAccountBookIcon,
  LegacyCaretDownIcon,
  LegacyCopyIcon,
  LegacyDeleteIcon,
  LegacyEditIcon,
  LegacyFileExcelIcon,
  LegacyFilterIcon,
  LegacyLoadingIcon,
  LegacyMailIcon,
  LegacyPlusIcon,
  LegacyReloadIcon,
  LegacySelectIcon,
  LegacySolutionIcon,
  LegacyStopIcon,
  LegacyUserAddIcon,
  LegacyUsergroupAddIcon,
} from '@/components/legacy-ant-icon';
import {
  LegacyStandaloneTable,
  LegacyTablePagination,
  legacyTableRowKey,
  type LegacyStandaloneTableHeader,
  type LegacyTablePaginationChange,
} from '@/components/legacy-standalone-table';
import { LegacyModal } from '@/components/legacy-modal';
import { legacyConfirm } from '@/components/legacy-confirm';
import {
  LegacySelect,
  type LegacySelectOption,
  type LegacySelectValue,
} from '@/components/legacy-select';
import {
  LegacyDropdown,
  LegacyDropdownMenu,
  LegacyDropdownMenuItem,
  LEGACY_DROPDOWN_CLICK_TRIGGER,
} from '@/components/legacy-dropdown';
import { LegacyTooltip } from '@/components/legacy-tooltip';
import {
  LegacyInput,
  LegacyInputCompactGroup,
  LegacyInputGroup,
  LegacyTextArea,
} from '@/components/legacy-input';
import { LegacyBadge } from '@/components/legacy-badge';
import { LegacyTag } from '@/components/legacy-tag';

type QueryState = {
  current: number;
  pageSize: number;
  pageSizeOptions?: number[];
  showSizeChanger?: boolean;
  size?: 'small';
  total?: number;
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

const GENERATE_USER_EMPTY_PLAN_OPTION: LegacySelectOption = { value: null, label: '无' };

const PERIOD_OPTIONS: LegacySelectOption[] = Object.keys(PERIOD_TEXT).map((period) => ({
  value: period,
  label: PERIOD_TEXT[period] ?? period,
}));

function planSelectOptions(plans: PlanOption[], includeEmpty = false): LegacySelectOption[] {
  return [
    ...(includeEmpty ? [GENERATE_USER_EMPTY_PLAN_OPTION] : []),
    ...plans.map((plan) => ({ value: plan.value, label: plan.label })),
  ];
}

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

const LEGACY_SORTABLE_CELL_CLASS = 'ant-table-column-has-actions ant-table-column-has-sorters';
const LEGACY_USER_PAGE_SIZE_OPTIONS = [10, 50, 100, 150];

function userRowKey(index: number) {
  return legacyTableRowKey(index);
}

function legacyUserTableRows<T>(rows: T[], current: number, pageSize: number) {
  if (rows.length <= pageSize) return rows;
  const page = Math.max(current || 1, 1);
  return rows.slice((page - 1) * pageSize, page * pageSize);
}

export default function UsersPage() {
  const { message } = App.useApp();
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
  const [toolbarDropdownVisible, setToolbarDropdownVisible] = useState(false);
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
      JSON.stringify({ filter: [{ key, condition, value }], total: users.data?.total }),
    );
    navigate('/order');
  };

  const resetUserSecret = (row: AdminUserRow) =>
    void legacyConfirm({
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
    void legacyConfirm({
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
          .catch(() => undefined);
      },
    });

  const runUserAction = (key: string, row: AdminUserRow) => {
    setContextMenu(null);
    if (key === 'edit') setEditing(row);
    if (key === 'assign') setAssigning(row);
    if (key === 'copy') {
      legacyCopyText(row.subscribe_url);
    }
    if (key === 'reset') resetUserSecret(row);
    if (key === 'orders') jumpOrderFilter('user_id', '=', row.id);
    if (key === 'invite') setFilter([{ key: 'invite_user_id', condition: '=', value: row.id }]);
    if (key === 'traffic') setTrafficUser(row);
    if (key === 'delete') deleteUser(row);
  };

  const data = users.data?.data ?? [];
  const sortUserTable = (sort: string) =>
    setQuery((state) => ({
      ...state,
      current: 1,
      pageSizeOptions: LEGACY_USER_PAGE_SIZE_OPTIONS,
      showSizeChanger: true,
      size: 'small',
      sort,
      sort_type: state.sort === sort && state.sort_type === 'ASC' ? 'DESC' : 'ASC',
    }));
  const sortableHeader = (title: string, sort: string): LegacyStandaloneTableHeader => ({
    title,
    className: LEGACY_SORTABLE_CELL_CLASS,
    onClick: () => sortUserTable(sort),
    sortOrder: query.sort === sort ? query.sort_type : undefined,
    sortable: true,
  });
  const headers: LegacyStandaloneTableHeader[] = [
    sortableHeader('ID', 'id'),
    { title: '邮箱' },
    sortableHeader('状态', 'banned'),
    sortableHeader('订阅', 'plan_id'),
    sortableHeader('权限组', 'group_id'),
    sortableHeader('已用(G)', 'total_used'),
    sortableHeader('流量(G)', 'transfer_enable'),
    sortableHeader('设备数', 'updated_at'),
    sortableHeader('到期时间', 'expired_at'),
    sortableHeader('余额', 'balance'),
    sortableHeader('佣金', 'commission_balance'),
    sortableHeader('加入时间', 'created_at'),
    { title: '操作', alignRight: true, fixedRight: true },
  ];

  const updateTablePagination = (pagination: LegacyTablePaginationChange) =>
    setQuery((state) => {
      writeLegacyHabit(LEGACY_USER_PAGE_SIZE_KEY, pagination.pageSize);
      return { ...state, sort_type: state.sort_type ?? 'DESC', ...pagination };
    });

  const renderUserEmail = (row: AdminUserRow) => {
    const legacyOnlineAt = (row as AdminUserRow & { t?: number | null }).t;
    const online = !(Date.now() / 1000 - 600 > Number(legacyOnlineAt));
    return (
      <LegacyTooltip
        placement="top"
        title={legacyOnlineAt ? `最后在线${formatDateTime(Number(legacyOnlineAt))}` : '从未在线'}
      >
        <span>
          <LegacyBadge status={online ? 'success' : 'default'} />
          {row.email}
        </span>
      </LegacyTooltip>
    );
  };

  const renderUserDeviceLimit = (row: AdminUserRow) => {
    const deviceCount = row.alive_ip !== null ? row.alive_ip : 0;
    const deviceLimit = row.device_limit !== null ? row.device_limit : '∞';
    const text = `${deviceCount} / ${deviceLimit}`;
    return row.ips ? (
      <LegacyTooltip placement="top" title={row.ips}>
        {text}
      </LegacyTooltip>
    ) : (
      text
    );
  };

  const renderUserExpiredAt = (value: number | null) => (
    <LegacyTag color={value !== null && value < Date.now() / 1000 ? 'red' : 'green'}>
      {value ? formatDateMinuteSlash(value) : value === null ? '长期有效' : '-'}
    </LegacyTag>
  );

  const renderUserActions = (row: AdminUserRow) => (
    <LegacyDropdown
      trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}
      overlay={
        <LegacyDropdownMenu>
          <LegacyDropdownMenuItem key="edit" onContextMenu={(event) => event.stopPropagation()}>
            <a onClick={() => runUserAction('edit', row)}>
              <LegacyEditIcon /> 编辑
            </a>
          </LegacyDropdownMenuItem>
          <LegacyDropdownMenuItem key="assign" onContextMenu={(event) => event.stopPropagation()}>
            <a onClick={() => runUserAction('assign', row)}>
              <LegacyPlusIcon /> 分配订单
            </a>
          </LegacyDropdownMenuItem>
          <LegacyDropdownMenuItem key="copy" onClick={(event) => event.stopPropagation()}>
            <a onClick={() => runUserAction('copy', row)}>
              <LegacyCopyIcon /> 复制订阅URL
            </a>
          </LegacyDropdownMenuItem>
          <LegacyDropdownMenuItem key="reset">
            <a onClick={() => runUserAction('reset', row)}>
              <LegacyReloadIcon /> 重置UUID及订阅URL
            </a>
          </LegacyDropdownMenuItem>
          <LegacyDropdownMenuItem key="orders" onClick={() => runUserAction('orders', row)}>
            <a>
              <LegacyAccountBookIcon /> TA的订单
            </a>
          </LegacyDropdownMenuItem>
          <LegacyDropdownMenuItem key="invite" onClick={() => runUserAction('invite', row)}>
            <a>
              <LegacyUsergroupAddIcon /> TA的邀请
            </a>
          </LegacyDropdownMenuItem>
          <LegacyDropdownMenuItem key="traffic" onContextMenu={(event) => event.stopPropagation()}>
            <a onClick={() => runUserAction('traffic', row)}>
              <LegacySolutionIcon /> TA的流量记录
            </a>
          </LegacyDropdownMenuItem>
          <LegacyDropdownMenuItem key="delete">
            <a onClick={() => runUserAction('delete', row)}>
              <LegacyDeleteIcon /> 删除用户
            </a>
          </LegacyDropdownMenuItem>
        </LegacyDropdownMenu>
      }
    >
      <a ref={legacyHref()}>
        操作 <LegacyCaretDownIcon />
      </a>
    </LegacyDropdown>
  );

  const visibleRows = legacyUserTableRows(data, query.current, query.pageSize);

  return (
    <>
      <LegacySpin loading={legacyFetchLoading(users.isFetching, users.error)}>
        <div className="block border-bottom">
          <div className="bg-white">
            <div className="v2board-table-action" style={{ padding: 15 }}>
              <LegacyTooltip
                title="Tips：可以使用过滤器过滤后再使用操作对过滤的用户进行操作。"
                placement="right"
              >
                <div className="ant-btn-group">
                  <LegacyFilterDrawer
                    key={query.filter.length}
                    value={query.filter}
                    keys={filterKeys}
                    onChange={setFilter}
                  >
                    <LegacyButton
                      className={`ant-btn${query.filter.length > 0 ? ' ant-btn-primary' : ''}`}
                    >
                      <LegacyFilterIcon />
                      <span> 过滤器</span>
                    </LegacyButton>
                  </LegacyFilterDrawer>
                  <LegacyDropdown
                    visible={toolbarDropdownVisible}
                    onVisibleChange={setToolbarDropdownVisible}
                    overlay={
                      <LegacyDropdownMenu>
                        <LegacyDropdownMenuItem key="csv">
                          <a
                            onClick={() => {
                              message.loading('导出中');
                              void dumpCsv
                                .mutateAsync(query.filter)
                                .then((response) => {
                                  message.destroy();
                                  downloadText(
                                    `${formatDateTime(Date.now() / 1000)}.csv`,
                                    response.buffer,
                                  );
                                })
                                .catch((error) => {
                                  message.destroy();
                                  showError(message, error);
                                });
                            }}
                          >
                            <LegacyFileExcelIcon /> 导出CSV
                          </a>
                        </LegacyDropdownMenuItem>
                        <LegacyDropdownMenuItem key="mail">
                          <a
                            onClick={() => {
                              setToolbarDropdownVisible(false);
                              setMailOpen(true);
                            }}
                          >
                            <LegacyMailIcon /> 发送邮件
                          </a>
                        </LegacyDropdownMenuItem>
                        <LegacyDropdownMenuItem key="ban" disabled={!query.filter.length}>
                          <a
                            {...legacyDisabledAnchorProps(!query.filter.length)}
                            onClick={() => {
                              void legacyConfirm({
                                title: '提醒',
                                content: '确定要进行封禁吗？',
                                okText: 'OK',
                                cancelText: 'Cancel',
                                onOk: () => {
                                  void banUsers
                                    .mutateAsync(query.filter)
                                    .then(() => {
                                      void users.refetch();
                                    })
                                    .catch(() => undefined);
                                },
                              });
                            }}
                          >
                            <LegacyStopIcon /> 批量封禁
                          </a>
                        </LegacyDropdownMenuItem>
                        <LegacyDropdownMenuItem key="delete" disabled={!query.filter.length}>
                          <a
                            {...legacyDisabledAnchorProps(!query.filter.length)}
                            onClick={() => {
                              void legacyConfirm({
                                title: '提醒',
                                content: '确定要进行删除吗？',
                                okText: 'OK',
                                cancelText: 'Cancel',
                                onOk: () => {
                                  void deleteAll
                                    .mutateAsync(query.filter)
                                    .then(() => {
                                      void users.refetch();
                                    })
                                    .catch(() => undefined);
                                },
                              });
                            }}
                          >
                            <LegacyDeleteIcon /> 批量删除
                          </a>
                        </LegacyDropdownMenuItem>
                      </LegacyDropdownMenu>
                    }
                  >
                    <LegacyButton className="ant-btn">
                      <LegacySelectIcon />
                      操作
                    </LegacyButton>
                  </LegacyDropdown>
                </div>
              </LegacyTooltip>
              <LegacyButton className="ant-btn ml-2" onClick={() => setCreating(true)}>
                <LegacyUserAddIcon />
              </LegacyButton>
            </div>
            <LegacyStandaloneTable
              className="v2board-table"
              headers={headers}
              isEmpty={visibleRows.length === 0}
              scrollX={1500}
              scrollPositionRight={false}
              fixedRightRowHeight={54}
              pagination={
                <LegacyTablePagination
                  current={query.current}
                  pageSize={query.pageSize}
                  total={users.data?.total}
                  pageSizeOptions={LEGACY_USER_PAGE_SIZE_OPTIONS}
                  onChange={updateTablePagination}
                />
              }
              fixedRightChildren={visibleRows.map((row, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  style={{ height: 54 }}
                  {...userRowKey(index)}
                >
                  <td
                    className="ant-table-align-right ant-table-row-cell-last"
                    style={{ textAlign: 'right' }}
                  >
                    {renderUserActions(row)}
                  </td>
                </tr>
              ))}
            >
              {visibleRows.map((row, index) => {
                const usedOverLimit =
                  parseFloat(String(row.total_used)) > parseFloat(String(row.transfer_enable));
                return (
                  <tr
                    key={index}
                    className="ant-table-row ant-table-row-level-0"
                    onClick={() => setContextMenu(null)}
                    onContextMenu={(event) => {
                      event.preventDefault();
                      setContextMenu({ user: row, top: event.clientY, left: event.clientX });
                    }}
                    {...userRowKey(index)}
                  >
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>{row.id}</td>
                    <td className="">{renderUserEmail(row)}</td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>
                      <LegacyTag color={row.banned ? 'red' : 'green'}>
                        {row.banned ? '封禁' : '正常'}
                      </LegacyTag>
                    </td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>{row.plan_name || '-'}</td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>
                      {row.group_id != null ? (groupMap.get(row.group_id) ?? '-') : '-'}
                    </td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>
                      <LegacyTag color={usedOverLimit ? 'red' : 'green'}>
                        {row.total_used}
                      </LegacyTag>
                    </td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>{row.transfer_enable}</td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>{renderUserDeviceLimit(row)}</td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>
                      {renderUserExpiredAt(row.expired_at)}
                    </td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>{row.balance}</td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>{row.commission_balance}</td>
                    <td className={LEGACY_SORTABLE_CELL_CLASS}>
                      {formatDateMinuteSlash(row.created_at)}
                    </td>
                    <td
                      className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"
                      style={{ textAlign: 'right' }}
                    >
                      {renderUserActions(row)}
                    </td>
                  </tr>
                );
              })}
            </LegacyStandaloneTable>
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
                    <LegacyEditIcon /> 编辑
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('assign', contextMenu.user)}>
                    <LegacyPlusIcon /> 分配订单
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('copy', contextMenu.user)}>
                    <LegacyCopyIcon /> 复制订阅URL
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a
                    style={{ color: '#ff4d4f' }}
                    onClick={() => contextMenu && runUserAction('reset', contextMenu.user)}
                  >
                    <LegacyReloadIcon /> 重置UUID及订阅URL
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('orders', contextMenu.user)}>
                    <LegacyAccountBookIcon /> TA的订单
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('invite', contextMenu.user)}>
                    <LegacyUsergroupAddIcon /> TA的邀请
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('traffic', contextMenu.user)}>
                    <LegacySolutionIcon /> TA的流量记录
                  </a>
                </li>
                <li className="ant-dropdown-menu-item">
                  <a onClick={() => contextMenu && runUserAction('delete', contextMenu.user)}>
                    <LegacyDeleteIcon /> 删除用户
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
            .then((response) => {
              if (values.generate_count) downloadGeneratedUserCsv(response.buffer);
              return users.refetch();
            })
            .then(() => {
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
              setToolbarDropdownVisible(true);
            })
            .catch(() => undefined)
        }
      />

      <AssignOrderModal user={assigning} plans={planOptions} onClose={() => setAssigning(null)} />

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
    <LegacyModal
      visible={open}
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
          <LegacyInputCompactGroup>
            {!submit.generate_count && (
              <LegacyInput
                className="ant-input"
                placeholder="账号（批量生成请留空）"
                style={{ width: '45%' }}
                value={submit.email_prefix}
                onChange={(event) => setSubmitField('email_prefix', event.target.value)}
              />
            )}
            <LegacyInput
              className="ant-input"
              placeholder="@"
              style={{ width: '10%', textAlign: 'center' }}
              disabled
            />
            <LegacyInput
              className="ant-input"
              placeholder="域"
              style={{ width: '45%' }}
              value={submit.email_suffix}
              onChange={(event) => setSubmitField('email_suffix', event.target.value)}
            />
          </LegacyInputCompactGroup>
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">密码</label>
          <LegacyInput
            className="ant-input"
            value={submit.password}
            placeholder="留空则密码与邮箱相同"
            onChange={(event) => setSubmitField('password', event.target.value)}
          />
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">到期时间</label>
          <div>
            <LegacyDatePicker
              placeholder="请选择用户到期日期，为空则不限制到期时间"
              defaultValue={submit.expired_at ? dayjs(1000 * Number(submit.expired_at)) : undefined}
              style={{ width: '100%' }}
              onChange={onDateChange}
            />
          </div>
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">订阅计划</label>
          <LegacySelect
            placeholder="请选择用户订阅计划"
            style={{ width: '100%' }}
            value={(submit.plan_id || null) as LegacySelectValue}
            options={planSelectOptions(plans, true)}
            onChange={(planId) =>
              setSubmitField('plan_id', planId as GenerateUserSubmit['plan_id'])
            }
          />
        </div>
        {!submit.email_prefix && (
          <div className="form-group">
            <label htmlFor="example-text-input-alt">生成数量</label>
            <LegacyInput
              className="ant-input"
              value={submit.generate_count}
              placeholder="如果为批量生成请输入生成数量"
              onChange={(event) => setSubmitField('generate_count', event.target.value)}
            />
          </div>
        )}
      </div>
    </LegacyModal>
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
    <LegacyModal
      visible={open}
      title="发送邮件"
      onOk={() => onSubmit(submit)}
      okButtonProps={{ loading }}
      onCancel={onClose}
    >
      <div className="form-group">
        <label htmlFor="example-text-input-alt">收件人</label>
        <LegacyInput
          className="ant-input"
          disabled
          value={filter.length ? '过滤用户' : '全部用户'}
        />
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">主题</label>
        <LegacyInput
          className="ant-input"
          placeholder="请输入邮件主题"
          value={submit.subject}
          onChange={(event) => setSubmit((state) => ({ ...state, subject: event.target.value }))}
        />
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">发送内容</label>
        <LegacyTextArea
          className="ant-input"
          rows={12}
          value={submit.content}
          placeholder="请输入邮件内容"
          onChange={(event) => setSubmit((state) => ({ ...state, content: event.target.value }))}
        />
      </div>
    </LegacyModal>
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
  const queryClient = useQueryClient();
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
    <LegacyModal
      title="订单分配"
      visible={Boolean(user)}
      onCancel={close}
      okText={assign.isPending ? <LegacyLoadingIcon /> : '确定'}
      cancelText="取消"
      onOk={() => {
        assign
          .mutateAsync(submit)
          .then(() => queryClient.invalidateQueries({ queryKey: ['admin', 'orders'] }))
          .then(close)
          .catch((error) => showError(message, error));
      }}
    >
      <div className="form-group">
        <label htmlFor="example-text-input-alt">用户邮箱</label>
        <LegacyInput
          className="ant-input"
          placeholder="请输入用户邮箱"
          value={submit.email}
          onChange={(event) => setSubmitField('email', event.target.value)}
        />
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">请选择订阅</label>
        <div>
          <LegacySelect
            value={submit.plan_id}
            style={{ width: '100%' }}
            placeholder="请选择订阅"
            options={planSelectOptions(plans)}
            onChange={(plan_id) =>
              setSubmitField('plan_id', plan_id as AssignOrderSubmit['plan_id'])
            }
          />
        </div>
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">请选择周期</label>
        <div>
          <LegacySelect
            value={submit.period}
            style={{ width: '100%' }}
            placeholder="请选择周期"
            options={PERIOD_OPTIONS}
            onChange={(period) => setSubmitField('period', period as AssignOrderSubmit['period'])}
          />
        </div>
      </div>
      <div className="form-group">
        <label htmlFor="example-text-input-alt">支付金额</label>
        <LegacyInputGroup
          placeholder="请输入需要支付的金额"
          addonAfter="¥"
          value={submit.total_amount}
          onChange={(event) => setSubmitField('total_amount', event.target.value)}
        />
      </div>
    </LegacyModal>
  );
}
