import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import dayjs from 'dayjs';
import { describe, expect, it, vi } from 'vitest';
import UsersPage from './users';

const usersSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'users.tsx'), 'utf8');
const generateUserModalSource = usersSource.slice(
  usersSource.indexOf('function GenerateUserModal'),
  usersSource.indexOf('function SendMailModal'),
);
const sendMailModalSource = usersSource.slice(
  usersSource.indexOf('function SendMailModal'),
  usersSource.indexOf('function AssignOrderModal'),
);
const assignOrderModalSource = usersSource.slice(usersSource.indexOf('function AssignOrderModal'));
const userManageDrawerSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../components/user-manage-drawer.tsx'),
  'utf8',
);
const userTrafficModalSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../components/user-traffic-modal.tsx'),
  'utf8',
);
const adminQueriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);
const legacyFilterDrawerSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../components/legacy-filter-drawer.tsx'),
  'utf8',
);

vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({
    removeQueries: vi.fn(),
  }),
}));

vi.mock('@/lib/queries', () => ({
  useAdminUsers: () => ({
    isLoading: false,
    isFetching: false,
    data: {
      data: [
        {
          id: 1,
          email: 'user@example.com',
          password: '',
          balance: '12.00',
          commission_balance: '34.00',
          transfer_enable: '100.00',
          device_limit: 3,
          u: 0,
          d: 0,
          total_used: '5.00',
          alive_ip: 2,
          ips: '127.0.0.1',
          plan_id: 1,
          plan_name: '基础套餐',
          group_id: 1,
          expired_at: 1893456000,
          uuid: 'uuid',
          token: 'token',
          subscribe_url: 'https://example.com/sub',
          banned: 0,
          is_admin: 0,
          is_staff: 0,
          invite_user_id: null,
          discount: null,
          commission_rate: null,
          telegram_id: null,
          last_login_at: 1700000000,
          created_at: 1700000000,
          updated_at: 1700000000,
        },
        {
          id: 2,
          email: 'blocked@example.com',
          password: '',
          balance: '0.00',
          commission_balance: '0.00',
          transfer_enable: '1.00',
          device_limit: null,
          u: 0,
          d: 0,
          total_used: '2.00',
          alive_ip: 0,
          ips: '',
          plan_id: null,
          plan_name: null,
          group_id: null,
          expired_at: null,
          uuid: 'uuid-2',
          token: 'token-2',
          subscribe_url: '',
          banned: 1,
          is_admin: 0,
          is_staff: 0,
          invite_user_id: null,
          discount: null,
          commission_rate: null,
          telegram_id: null,
          last_login_at: null,
          created_at: 1700086400,
          updated_at: 1700086400,
        },
      ],
      total: 2,
    },
  }),
  useAdminUserTraffic: () => ({
    isLoading: false,
    data: {
      data: [{ record_at: 1700000000, u: 1024, d: 2048, server_rate: 1 }],
      total: 1,
    },
  }),
  useAdminPlans: () => ({
    data: [{ id: 1, name: '基础套餐' }],
  }),
  useAdminUserInfo: () => ({
    data: {
      id: 1,
      email: 'user@example.com',
      balance: '12.00',
      commission_balance: '34.00',
      transfer_enable: '100.00',
      device_limit: 3,
      u: 0,
      d: 0,
      plan_id: 1,
      expired_at: 1893456000,
      banned: 0,
      is_admin: 0,
      is_staff: 0,
    },
  }),
  useServerGroups: () => ({
    data: [{ id: 1, name: '默认权限组' }],
  }),
  useUpdateUserMutation: () => ({ mutateAsync: vi.fn() }),
  useDeleteUserMutation: () => ({ mutateAsync: vi.fn() }),
  useResetUserSecretMutation: () => ({ mutateAsync: vi.fn() }),
  useGenerateUserMutation: () => ({ mutateAsync: vi.fn() }),
  useDumpUsersCsvMutation: () => ({ mutateAsync: vi.fn() }),
  useSendMailToUsersMutation: () => ({ mutateAsync: vi.fn() }),
  useBanUsersMutation: () => ({ mutateAsync: vi.fn() }),
  useDeleteAllUsersMutation: () => ({ mutateAsync: vi.fn() }),
  useAssignOrderMutation: () => ({ mutateAsync: vi.fn() }),
}));

describe('UsersPage legacy user manager', () => {
  it('renders the original user table shell, toolbar, columns, and row values', () => {
    const html = renderToStaticMarkup(<UsersPage />);

    expect(html).toContain('class="block border-bottom"');
    expect(html).toContain('class="bg-white"');
    expect(html).toContain('class="v2board-table-action"');
    expect(html).toContain('过滤器');
    expect(html).toContain('操作');
    expect(html).toContain('anticon-user-add');
    expect(html).toContain('ID');
    expect(html).toContain('邮箱');
    expect(html).toContain('状态');
    expect(html).toContain('订阅');
    expect(html).toContain('权限组');
    expect(html).toContain('已用(G)');
    expect(html).toContain('流量(G)');
    expect(html).toContain('设备数');
    expect(html).toContain('到期时间');
    expect(html).toContain('余额');
    expect(html).toContain('佣金');
    expect(html).toContain('加入时间');
    expect(html).toContain('user@example.com');
    expect(html).toContain('基础套餐');
    expect(html).toContain('默认权限组');
    expect(html).toContain('正常');
    expect(html).toContain('封禁');
    expect(html).toContain('5.00');
    expect(html).toContain('100.00');
    expect(html).toContain('2 / 3');
    expect(html).toContain('长期有效');
    expect(html).toContain(dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm'));
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-typography');
  });

  it('opens the legacy user traffic modal instead of a placeholder notice', () => {
    expect(usersSource).not.toContain("message.info('TA的流量记录')");
    expect(usersSource).toContain('<UserTrafficModal');
    expect(userTrafficModalSource).toContain('useAdminUserTraffic');
    expect(userTrafficModalSource).toContain('title="流量记录"');
    expect(userTrafficModalSource).toContain("styles={{ body: { padding: 0 } }}");
    expect(userTrafficModalSource).toContain('footer={false}');
    expect(userTrafficModalSource).not.toContain('footer={null}');
    expect(userTrafficModalSource).toContain('function LegacyTrafficSpin');
    expect(userTrafficModalSource).toContain('className="spinner-grow text-primary"');
    expect(userTrafficModalSource).toContain('<LegacyTrafficSpin loading={records.isFetching}>');
    expect(userTrafficModalSource).toContain('page: 1');
    expect(userTrafficModalSource).toContain('total: 0');
    expect(userTrafficModalSource).toContain('pagination,');
    expect(userTrafficModalSource).toContain('total: records.data?.total,');
    expect(userTrafficModalSource).toContain('total?: number;');
    expect(userTrafficModalSource).toContain('key: \'d\'');
    expect(userTrafficModalSource).not.toContain('page: pagination.current');
    expect(userTrafficModalSource).not.toContain('records.data?.total ?? pagination.total');
    expect(userTrafficModalSource).not.toContain('<Table\n          loading={records.isFetching}');
    expect(userTrafficModalSource).not.toContain('rowKey={(record)');
    expect(adminQueriesSource).toContain(
      'admin.statUser(apiClient, { user_id: userId as number, ...query })',
    );
  });

  it('uses the shared legacy user management drawer for row edits', () => {
    expect(usersSource).not.toContain('<UserEditModal');
    expect(usersSource).toContain('<UserManageDrawer');
    expect(usersSource).toContain('onSaved={() => {\n          void users.refetch();\n        }}');
    expect(userManageDrawerSource).toContain('width="80%"');
    expect(userManageDrawerSource).toContain('title="用户管理"');
    expect(userManageDrawerSource).toContain('v2board-drawer-action');
    expect(userManageDrawerSource).toContain('function LegacyDrawerLoadingIcon');
    expect(userManageDrawerSource).toContain('className="anticon anticon-loading"');
    expect(userManageDrawerSource).toContain("color: '#415A94'");
    expect(userManageDrawerSource).toContain('transfer_enable: user.transfer_enable as unknown as number');
    expect(userManageDrawerSource).toContain('balance: user.balance as unknown as number');
    expect(userManageDrawerSource).toContain('expired_at: user.expired_at');
    expect(userManageDrawerSource).toContain('is_admin: user.is_admin');
    expect(userManageDrawerSource).toContain('is_staff: user.is_staff');
    expect(userManageDrawerSource).toContain('function legacyExpiredAtDefaultValue(value: UserManageFormValues');
    expect(userManageDrawerSource).toContain('value !== null && dayjs(1000 * Number(value))');
    expect(userManageDrawerSource).toContain('checked={values.is_admin as unknown as boolean}');
    expect(userManageDrawerSource).toContain("onChange={(value) => formChange('is_admin', value ? 1 : 0)}");
    expect(userManageDrawerSource).toContain('checked={values.is_staff as unknown as boolean}');
    expect(userManageDrawerSource).toContain("onChange={(value) => formChange('is_staff', value ? 1 : 0)}");
    expect(userManageDrawerSource).toContain('transfer_enable: scaled(values.transfer_enable, BYTE_GB)');
    expect(userManageDrawerSource).toContain('function scaled(value: unknown, multiplier: number)');
    expect(userManageDrawerSource).not.toContain('transfer_enable: scaledRounded(values.transfer_enable, BYTE_GB)');
    expect(userManageDrawerSource).toContain('u: scaledRounded(values.u, BYTE_GB)');
    expect(userManageDrawerSource).toContain('d: scaledRounded(values.d, BYTE_GB)');
    expect(userManageDrawerSource).toContain('balance: scaledRounded(values.balance, 100)');
    expect(userManageDrawerSource).not.toContain('Number(value ?? 0)');
    expect(userManageDrawerSource).not.toContain('Boolean(user.is_admin)');
    expect(userManageDrawerSource).not.toContain('Boolean(user.is_staff)');
    expect(userManageDrawerSource).not.toContain('scaledFixed(');
    expect(userManageDrawerSource).not.toContain("password: '',");
    expect(userManageDrawerSource).toContain('function legacyDefaultValue(value: unknown)');
    expect(userManageDrawerSource).toContain('defaultValue={values.email}');
    expect(userManageDrawerSource).toContain(
      'defaultValue={legacyDefaultValue(values.invite_user_email)}',
    );
    expect(userManageDrawerSource).toContain('defaultValue={values.password}');
    expect(userManageDrawerSource).toContain('defaultValue={values.transfer_enable}');
    expect(userManageDrawerSource).toContain(
      'defaultValue={legacyDefaultValue(values.device_limit)}',
    );
    expect(userManageDrawerSource).toContain('defaultValue={legacyExpiredAtDefaultValue(values.expired_at)}');
    expect(userManageDrawerSource).toContain("onChange={(value) => formChange('expired_at', value ? value.format('X') : null)}");
    expect(userManageDrawerSource).toContain('defaultValue={values.plan_id || null}');
    expect(userManageDrawerSource).toContain('defaultValue={values.banned ? 1 : 0}');
    expect(userManageDrawerSource).toContain(
      'defaultValue={parseInt(values.commission_type as string)}',
    );
    expect(userManageDrawerSource).toContain(
      'defaultValue={legacyDefaultValue(values.commission_rate)}',
    );
    expect(userManageDrawerSource).toContain('defaultValue={legacyDefaultValue(values.discount)}');
    expect(userManageDrawerSource).toContain(
      'defaultValue={legacyDefaultValue(values.speed_limit)}',
    );
    expect(userManageDrawerSource).toContain('defaultValue={legacyDefaultValue(values.remarks)}');
    expect(userManageDrawerSource).not.toContain('value={values.email}');
    expect(userManageDrawerSource).not.toContain('value={values.password ??');
    expect(userManageDrawerSource).not.toContain('value={values.plan_id || null}');
    expect(userManageDrawerSource).not.toContain('defaultValue={Number(values.commission_type ?? 0)}');
    expect(userManageDrawerSource).not.toContain('defaultValue={values.invite_user_email ?? undefined}');
    expect(userManageDrawerSource).not.toContain('defaultValue={values.device_limit ?? undefined}');
    expect(userManageDrawerSource).not.toContain('expiredAt ? expiredAt.unix() : null');
    expect(userManageDrawerSource).not.toContain('expired_at: user.expired_at == null ? null : dayjs');
    expect(userManageDrawerSource).not.toContain('defaultValue={values.commission_rate ?? undefined}');
    expect(userManageDrawerSource).not.toContain('defaultValue={values.discount ?? undefined}');
    expect(userManageDrawerSource).not.toContain('defaultValue={values.speed_limit ?? undefined}');
    expect(userManageDrawerSource).not.toContain('defaultValue={values.remarks ?? undefined}');
    expect(userManageDrawerSource).toContain(
      'if ((payload as Record<string, unknown>).invite_user) {',
    );
    expect(userManageDrawerSource).toContain('delete (payload as Record<string, unknown>).invite_user');
    expect(userManageDrawerSource).toContain('<Select.Option key={Math.random()} value={plan.id}>');
    expect(userManageDrawerSource).not.toContain('<Select.Option key={plan.id} value={plan.id}>');
    expect(userManageDrawerSource).not.toContain('<Form');
    expect(userManageDrawerSource).not.toContain('<Spin');
  });

  it('keeps user update dispatching the page fetch before the drawer closes', () => {
    const updateStart = userManageDrawerSource.indexOf('.mutateAsync(toPayload(values, userId))');
    const updateRefetch = userManageDrawerSource.indexOf('onSaved?.();', updateStart);
    const updateHide = userManageDrawerSource.indexOf('hide();', updateRefetch);
    const updateUserHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useUpdateUserMutation()'),
      adminQueriesSource.indexOf('export function useDeleteUserMutation()'),
    );

    expect(updateStart).toBeGreaterThan(-1);
    expect(updateRefetch).toBeGreaterThan(updateStart);
    expect(updateHide).toBeGreaterThan(updateRefetch);
    expect(userManageDrawerSource).toContain('onSaved?: () => void;');
    expect(userManageDrawerSource).toContain('        onSaved?.();\n        hide();');
    expect(userManageDrawerSource).not.toContain('await onSaved?.();');
    expect(userManageDrawerSource).not.toContain("message.success('操作成功')");
    expect(updateUserHook).not.toContain('onSuccess');
    expect(updateUserHook).not.toContain("queryKey: ['admin', 'users']");
  });

  it('keeps the legacy assigned-order modal loading OK text', () => {
    expect(usersSource).toContain("okText={assign.isPending ? <LoadingOutlined /> : '确定'}");
    expect(usersSource).toContain('function assignOrderSubmit(email?: string): AssignOrderSubmit');
    expect(usersSource).toContain('email: email || undefined');
    expect(usersSource).toContain('plan_id: undefined');
    expect(usersSource).toContain('period: undefined');
    expect(usersSource).toContain('total_amount: undefined');
    expect(assignOrderModalSource).toContain('setSubmit(assignOrderSubmit(user.email));');
    expect(assignOrderModalSource).toContain('setSubmit(assignOrderSubmit(user?.email));');
    expect(assignOrderModalSource).not.toContain('setSubmit({ email: user.email });');
    expect(assignOrderModalSource).toContain('.mutateAsync(submit)');
    expect(assignOrderModalSource).toContain('<label htmlFor="example-text-input-alt">请选择订阅</label>');
    expect(assignOrderModalSource).toContain('<Select.Option value={plan.value} key={Math.random()}>');
    expect(assignOrderModalSource).toContain('<Select.Option value={period} key={Math.random()}>');
    expect(assignOrderModalSource).not.toContain('options={plans}');
    expect(assignOrderModalSource).not.toContain('options={Object.entries(PERIOD_TEXT)');
    expect(assignOrderModalSource).not.toContain('<Form');
    expect(assignOrderModalSource).not.toContain('rules={[{ required: true }]}');
  });

  it('keeps the legacy create-user modal stateful layout and CSV download', () => {
    expect(generateUserModalSource).toContain('title="创建用户"');
    expect(generateUserModalSource).toContain('okText="生成"');
    expect(generateUserModalSource).toContain('okButtonProps={{ loading }}');
    expect(generateUserModalSource).toContain('<Input.Group compact>');
    expect(generateUserModalSource).toContain('!submit.generate_count');
    expect(generateUserModalSource).toContain('!submit.email_prefix');
    expect(generateUserModalSource).toContain('<DatePicker');
    expect(generateUserModalSource).toContain('defaultValue={submit.expired_at ? dayjs(1000 * Number(submit.expired_at)) : undefined}');
    expect(generateUserModalSource).not.toContain('value={submit.expired_at ? dayjs(1000 * Number(submit.expired_at)) : null}');
    expect(generateUserModalSource).toContain('<label htmlFor="example-text-input-alt">订阅计划</label>');
    expect(generateUserModalSource).toContain('<Select.Option value={null}>无</Select.Option>');
    expect(generateUserModalSource).toContain('<Select.Option key={Math.random()} value={plan.value}>');
    expect(generateUserModalSource).not.toContain("options={[{ value: null, label: '无' }, ...plans]}");
    expect(generateUserModalSource).not.toContain('id="generate-user-plan"');
    expect(generateUserModalSource).not.toContain('<Form');
    expect(generateUserModalSource).not.toContain('rules={[{ required: true }]}');
    expect(usersSource).toContain('downloadGeneratedUserCsv(response.buffer)');
    expect(usersSource).toContain('void users.refetch();');
    expect(usersSource).toContain('void users.refetch();\n              setCreating(false);');
    expect(usersSource).not.toContain('await users.refetch();');
    expect(usersSource).toContain("USER ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv");
    expect(usersSource).not.toContain("message.success('操作成功')");

    const downloadIndex = usersSource.indexOf('downloadGeneratedUserCsv(response.buffer)');
    const refetchIndex = usersSource.indexOf('void users.refetch();', downloadIndex);
    const closeIndex = usersSource.indexOf('setCreating(false);', refetchIndex);
    expect(downloadIndex).toBeGreaterThan(-1);
    expect(refetchIndex).toBeGreaterThan(downloadIndex);
    expect(closeIndex).toBeGreaterThan(refetchIndex);

    const generateUserHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useGenerateUserMutation()'),
      adminQueriesSource.indexOf('export function useDumpUsersCsvMutation()'),
    );
    expect(generateUserHook).not.toContain('onSuccess');
    expect(generateUserHook).not.toContain("queryKey: ['admin', 'users']");
  });

  it('keeps the legacy send-mail modal stateful layout', () => {
    expect(sendMailModalSource).toContain('title="发送邮件"');
    expect(sendMailModalSource).toContain('okButtonProps={{ loading }}');
    expect(sendMailModalSource).toContain('收件人');
    expect(sendMailModalSource).toContain('<label htmlFor="example-text-input-alt">收件人</label>');
    expect(sendMailModalSource).toContain('<label htmlFor="example-text-input-alt">主题</label>');
    expect(sendMailModalSource).toContain('<label htmlFor="example-text-input-alt">发送内容</label>');
    expect(sendMailModalSource).toContain("filter.length ? '过滤用户' : '全部用户'");
    expect(sendMailModalSource).toContain('placeholder="请输入邮件主题"');
    expect(sendMailModalSource).toContain('rows={12}');
    expect(sendMailModalSource).toContain('placeholder="请输入邮件内容"');
    expect(sendMailModalSource).toContain('onOk={() => onSubmit(submit)}');
    expect(sendMailModalSource).not.toContain('send-mail-recipient');
    expect(sendMailModalSource).not.toContain('send-mail-subject');
    expect(sendMailModalSource).not.toContain('send-mail-content');
    expect(sendMailModalSource).not.toContain('<Form');
    expect(sendMailModalSource).not.toContain('rules={[{ required: true }]}');
    expect(usersSource).toContain('.mutateAsync({ filter: query.filter, ...values })');
  });

  it('uses the original drawer-style filter with select and date filter types', () => {
    expect(usersSource).not.toContain('function LegacyFilterButton');
    expect(usersSource).toContain('<LegacyFilterDrawer');
    expect(usersSource).toContain('key={query.filter.length}');
    expect(usersSource).toContain("type={query.filter.length > 0 ? 'primary' : ('' as ButtonProps['type'])}");
    expect(usersSource).not.toContain("type={query.filter.length > 0 ? 'primary' : 'default'}");
    expect(usersSource).toContain("key: 'expired_at'");
    expect(usersSource).toContain("type: 'date'");
    expect(usersSource).toContain('const filterPlanOptions = useMemo');
    expect(usersSource).toContain('key: plan.name');
    expect(usersSource).toContain("{ key: '无订阅', value: 'null' }");
    expect(usersSource).toContain("{ key: '正常', value: 0 }");
    expect(usersSource).toContain("{ key: '是', value: 1 }");
    expect(legacyFilterDrawerSource).toContain('className="v2board-filter-drawer"');
    expect(legacyFilterDrawerSource).toContain('footer={<></>}');
    expect(legacyFilterDrawerSource).not.toContain('footer={null}');
    expect(legacyFilterDrawerSource).toContain('<label>字段名</label>');
    expect(legacyFilterDrawerSource).toContain('<label>条件</label>');
    expect(legacyFilterDrawerSource).toContain('<label>欲检索内容</label>');
    expect(legacyFilterDrawerSource).toContain('return (\n            <>');
    expect(legacyFilterDrawerSource).not.toContain('key={`${filter.key}-${index}`}');
    expect(legacyFilterDrawerSource).toContain('keys.find((key) => key.key === filter.key)!');
    expect(legacyFilterDrawerSource).not.toContain('?? keys[0]!');
    expect(legacyFilterDrawerSource).toContain('<Select.Option');
    expect(legacyFilterDrawerSource).toContain('key={optionIndex}');
    expect(legacyFilterDrawerSource).toContain('value={item.key}');
    expect(legacyFilterDrawerSource).toContain('keys[keyIndex]!.condition.map');
    expect(legacyFilterDrawerSource).toContain('selected.options!.map');
    expect(legacyFilterDrawerSource).not.toContain('keys[keyIndex]?.condition ?? []');
    expect(legacyFilterDrawerSource).not.toContain('selected.options ?? []');
    expect(legacyFilterDrawerSource).toContain('<Select.Option value={option.value}>{option.key}</Select.Option>');
    expect(legacyFilterDrawerSource).not.toContain('legacy-filter-key');
    expect(legacyFilterDrawerSource).not.toContain('legacy-filter-condition');
    expect(legacyFilterDrawerSource).not.toContain('legacy-filter-value');
    expect(legacyFilterDrawerSource).not.toContain('htmlFor={`legacy-filter');
    expect(legacyFilterDrawerSource).not.toContain('options={keys.map');
    expect(legacyFilterDrawerSource).not.toContain('label: option.label');
    expect(legacyFilterDrawerSource).toContain('<DatePicker');
    expect(legacyFilterDrawerSource).toContain('添加条件');
    expect(legacyFilterDrawerSource).toContain('欲检索内容不能为空');
    expect(legacyFilterDrawerSource).toContain('v2board-drawer-action');
    expect(legacyFilterDrawerSource).toContain('useState<AdminFilter[]>(value || [])');
    expect(legacyFilterDrawerSource).not.toContain('if (!open) setFilters(value)');
    expect(legacyFilterDrawerSource).toContain('...item,');
    expect(legacyFilterDrawerSource).toContain('condition: first.condition[0]!');
    expect(legacyFilterDrawerSource).toContain('condition: next.condition[0]!');
    expect(legacyFilterDrawerSource).not.toContain("?? '='");
    expect(legacyFilterDrawerSource).not.toContain(
      "key: next.key,\n            condition: next.condition[0]!,\n            value: '',",
    );
    expect(legacyFilterDrawerSource).toContain("return value === '';");
    expect(legacyFilterDrawerSource).toContain('defaultValue={filter.value || undefined}');
    expect(legacyFilterDrawerSource).toContain("date && date.format('X')");
    expect(legacyFilterDrawerSource).not.toContain("date ? date.format('X') : ''");
    expect(legacyFilterDrawerSource).not.toContain('keys[keyIndex]?.condition');
  });

  it('preserves the original row right-click action menu', () => {
    expect(usersSource).toContain('id="v2board-table-dropdown"');
    expect(usersSource).toContain('ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical');
    expect(usersSource).toContain('onContextMenu: (event) =>');
    expect(usersSource).toContain('event.preventDefault()');
    expect(usersSource).toContain('event.clientY');
    expect(usersSource).toContain('event.clientX');
    expect(usersSource).toContain("display: contextMenu ? 'unset' : 'none'");
    expect(usersSource).toContain("runUserAction('traffic', contextMenu.user)");
    expect(usersSource).toContain("runUserAction('delete', contextMenu.user)");
  });

  it('keeps the original anchor labels in user operation dropdowns', () => {
    const rowActionSource = usersSource.slice(
      usersSource.indexOf("title: '操作'"),
      usersSource.indexOf('[groupMap, runUserAction]'),
    );
    const toolbarSource = usersSource.slice(
      usersSource.indexOf('className="v2board-table-action"'),
      usersSource.indexOf('<span className="float-right">'),
    );

    expect(rowActionSource).toContain("{ key: 'edit', label: <a><EditOutlined /> 编辑</a> }");
    expect(rowActionSource).toContain("{ key: 'assign', label: <a><PlusOutlined /> 分配订单</a> }");
    expect(rowActionSource).toContain("{ key: 'copy', label: <a><CopyOutlined /> 复制订阅URL</a> }");
    expect(rowActionSource).toContain("{ key: 'reset', label: <a><ReloadOutlined /> 重置UUID及订阅URL</a> }");
    expect(rowActionSource).toContain("{ key: 'orders', label: <a><AccountBookOutlined /> TA的订单</a> }");
    expect(rowActionSource).toContain("{ key: 'invite', label: <a><UsergroupAddOutlined /> TA的邀请</a> }");
    expect(rowActionSource).toContain("{ key: 'traffic', label: <a><SolutionOutlined /> TA的流量记录</a> }");
    expect(rowActionSource).toContain("{ key: 'delete', label: <a><DeleteOutlined /> 删除用户</a> }");
    expect(rowActionSource).not.toContain('label: <span>');

    expect(usersSource).toContain('type AnchorHTMLAttributes');
    expect(usersSource).toContain('function legacyDisabledAnchorProps(disabled: boolean): AnchorHTMLAttributes<HTMLAnchorElement>');
    expect(usersSource).toContain('return { disabled } as unknown as AnchorHTMLAttributes<HTMLAnchorElement>;');
    expect(toolbarSource).toContain("{ key: 'csv', label: <a><FileExcelOutlined /> 导出CSV</a> }");
    expect(toolbarSource).toContain("{ key: 'mail', label: <a><MailOutlined /> 发送邮件</a> }");
    expect(toolbarSource).toContain(
      'label: <a {...legacyDisabledAnchorProps(!query.filter.length)}><StopOutlined /> 批量封禁</a>',
    );
    expect(toolbarSource).toContain(
      'label: <a {...legacyDisabledAnchorProps(!query.filter.length)}><DeleteOutlined /> 批量删除</a>',
    );
    expect(toolbarSource).not.toContain('label: <span>');
  });

  it('keeps the toolbar operation dropdown on the original default trigger', () => {
    const toolbarSource = usersSource.slice(
      usersSource.indexOf('className="v2board-table-action"'),
      usersSource.indexOf('<span className="float-right">'),
    );

    expect(toolbarSource).toContain('<Dropdown');
    expect(toolbarSource).not.toContain("trigger={['click']}");
  });

  it('keeps the original refetch loading mask around the whole user table block', () => {
    expect(usersSource).toContain('function LegacySpin');
    expect(usersSource).toContain('<LegacySpin loading={users.isFetching}>');
    expect(usersSource).not.toContain('loading={users.isLoading}');
  });

  it('keeps the legacy main table keying and confirm-button behavior', () => {
    expect(usersSource).not.toContain('rowKey="id"');
    expect(usersSource).toContain('onOk: () => {\n        void resetSecret');
    expect(usersSource).toContain('onOk: () => {\n        void remove');
    expect(usersSource).toContain('void banUsers\n                                .mutateAsync(query.filter)');
    expect(usersSource).toContain('void deleteAll\n                                .mutateAsync(query.filter)');
    expect(usersSource).not.toContain('onOk: () =>\n        resetSecret');
    expect(usersSource).not.toContain('onOk: () =>\n        remove');
    expect(usersSource).not.toContain('onOk: () => banUsers.mutateAsync(query.filter)');
    expect(usersSource).not.toContain('onOk: () => deleteAll.mutateAsync(query.filter)');
  });

  it('keeps reset-secret and delete-user success messages before user refetch', () => {
    const resetSuccess = usersSource.indexOf("message.success('重置成功')");
    const resetRefetch = usersSource.indexOf('void users.refetch();', resetSuccess);
    const deleteSuccess = usersSource.indexOf("message.success('删除成功')");
    const deleteRefetch = usersSource.indexOf('void users.refetch();', deleteSuccess);

    expect(resetSuccess).toBeGreaterThan(-1);
    expect(resetRefetch).toBeGreaterThan(resetSuccess);
    expect(deleteSuccess).toBeGreaterThan(-1);
    expect(deleteRefetch).toBeGreaterThan(deleteSuccess);

    const deleteUserHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useDeleteUserMutation()'),
      adminQueriesSource.indexOf('export function useResetUserSecretMutation()'),
    );
    const resetSecretHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useResetUserSecretMutation()'),
      adminQueriesSource.indexOf('export function useGenerateUserMutation()'),
    );
    expect(deleteUserHook).not.toContain('onSuccess');
    expect(resetSecretHook).not.toContain('onSuccess');
    expect(deleteUserHook).not.toContain("queryKey: ['admin', 'users']");
    expect(resetSecretHook).not.toContain("queryKey: ['admin', 'users']");
  });

  it('keeps bulk ban and delete-all fetching after the request succeeds', () => {
    const banStart = usersSource.indexOf('void banUsers\n                                .mutateAsync(query.filter)');
    const banRefetch = usersSource.indexOf('void users.refetch();', banStart);
    const deleteAllStart = usersSource.indexOf(
      'void deleteAll\n                                .mutateAsync(query.filter)',
    );
    const deleteAllRefetch = usersSource.indexOf('void users.refetch();', deleteAllStart);

    expect(banStart).toBeGreaterThan(-1);
    expect(banRefetch).toBeGreaterThan(banStart);
    expect(deleteAllStart).toBeGreaterThan(-1);
    expect(deleteAllRefetch).toBeGreaterThan(deleteAllStart);

    const banUsersHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useBanUsersMutation()'),
      adminQueriesSource.indexOf('export function useDeleteAllUsersMutation()'),
    );
    const deleteAllUsersHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useDeleteAllUsersMutation()'),
      adminQueriesSource.indexOf('export function useMarkOrderPaidMutation()'),
    );
    expect(banUsersHook).not.toContain('onSuccess');
    expect(deleteAllUsersHook).not.toContain('onSuccess');
    expect(banUsersHook).not.toContain("queryKey: ['admin', 'users']");
    expect(deleteAllUsersHook).not.toContain("queryKey: ['admin', 'users']");
  });

  it('uses the old online badge calculation from the row t field only', () => {
    expect(usersSource).toContain("const legacyOnlineAt = (row as AdminUserRow & { t?: number | null }).t");
    expect(usersSource).toContain('Date.now() / 1000 - 600 > Number(legacyOnlineAt)');
    expect(usersSource).toContain("legacyOnlineAt ? `最后在线${formatDateTime(Number(legacyOnlineAt))}` : '从未在线'");
    expect(usersSource).toContain("<Badge status={online ? 'success' : 'default'} />{email}");
    expect(usersSource).not.toContain("<Badge status={online ? 'success' : 'default'} /> {email}");
    expect(usersSource).not.toContain('row.last_login_at ?? 0');
  });

  it('keeps the legacy device-count null handling and sorter', () => {
    expect(usersSource).toContain("key: 'updated_at'");
    expect(usersSource).toContain("sorter: (a, b) => (a.alive_ip as number) - (b.alive_ip as number)");
    expect(usersSource).toContain('const deviceCount = row.alive_ip !== null ? row.alive_ip : 0;');
    expect(usersSource).toContain("const deviceLimit = row.device_limit !== null ? row.device_limit : '∞';");
    expect(usersSource).not.toContain('row.alive_ip ?? 0');
    expect(usersSource).not.toContain("value ?? '∞'");
  });

  it('preserves the legacy remembered user table page size habit', () => {
    expect(usersSource).toContain("const LEGACY_HABIT_KEY = 'habit'");
    expect(usersSource).toContain("const LEGACY_USER_PAGE_SIZE_KEY = 'user_manage_page_size'");
    expect(usersSource).toContain('function readLegacyUserPageSize()');
    expect(usersSource).toContain('pageSize: readLegacyUserPageSize()');
    expect(usersSource).toContain('writeLegacyHabit(LEGACY_USER_PAGE_SIZE_KEY, pagination.pageSize)');
    expect(usersSource).toContain('const legacyHabit = stored as unknown as Record<string, unknown>;');
    expect(usersSource).toContain('legacyHabit[key] = value;');
    expect(usersSource).toContain(
      'window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify(legacyHabit));',
    );
    expect(usersSource).toContain(
      'window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify({ [key]: value }));',
    );
    expect(usersSource).not.toContain('const parsed = stored ? JSON.parse(stored) : {};');
    expect(usersSource).not.toContain('next[key] = value;');
  });

  it('keeps the original user model cleanup and full pagination merge on table changes', () => {
    expect(usersSource).toContain("import { useQueryClient } from '@tanstack/react-query';");
    expect(usersSource).toContain('const queryClient = useQueryClient();');
    expect(usersSource).toContain("queryClient.removeQueries({ queryKey: ['admin', 'users'] });");
    expect(usersSource).toContain("queryClient.removeQueries({ queryKey: ['admin', 'user'] });");
    expect(usersSource).toContain('...pagination,');
    expect(usersSource).toContain("sort_type: singleSorter?.order === 'ascend' ? 'ASC' : 'DESC'");
    expect(usersSource).not.toContain('current: pagination.current ?? state.current');
    expect(usersSource).not.toContain('const nextPageSize = pagination.pageSize ?? state.pageSize');
  });

  it('keeps the bundled user pagination total as the direct response field', () => {
    expect(usersSource).toContain('total: users.data?.total,');
    expect(usersSource).not.toContain('total: users.data?.total ?? 0');
  });

  it('keeps the legacy user toolbar button group spacing', () => {
    expect(usersSource).toContain('<Button.Group>');
    expect(usersSource).toContain('</Button.Group>');
    expect(usersSource).not.toContain('<Space>');
    expect(usersSource).not.toContain('  Space,');
  });

  it('uses the old copy helper for subscription URL copying', () => {
    expect(usersSource).toContain("import { legacyCopyText } from '@/lib/legacy-copy';");
    expect(usersSource).toContain('legacyCopyText(row.subscribe_url)');
    expect(usersSource).not.toContain('navigator.clipboard?.writeText');
  });

  it('keeps the legacy CSV export loading message lifecycle', () => {
    expect(usersSource).toContain("message.loading('导出中')");
    expect(usersSource).toContain('message.destroy()');
    expect(usersSource).toContain("type: 'text/plain,charset=UTF-8'");
    expect(usersSource).not.toContain("type: 'text/plain;charset=UTF-8'");
    expect(usersSource).toContain('downloadText(`${formatDateTime(Date.now() / 1000)}.csv`');
    expect(usersSource).toContain('downloadText(`${formatDateTime(Date.now() / 1000)}.csv`, response.buffer)');
    expect(usersSource).toContain('new Blob([buffer as BlobPart]');
    expect(usersSource).not.toContain("String(text ?? '')");
    expect(usersSource).not.toContain('payload.buffer ?? payload.data');
  });
});
