import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import dayjs from 'dayjs';
import { describe, expect, it, vi } from 'vitest';
import UsersPage from './users';

const usersSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'users.tsx'),
  'utf8',
);
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
const legacyDatePickerSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../components/legacy-date-picker.tsx'),
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
    expect(html).toContain(
      'class="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last" style="text-align:right"',
    );
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
    expect(userTrafficModalSource).toContain('styles={{ body: { padding: 0 } }}');
    expect(userTrafficModalSource).toContain('footer={false}');
    expect(userTrafficModalSource).not.toContain('footer={null}');
    expect(userTrafficModalSource).toContain(
      "import { LegacySpin } from '@/components/legacy-spin';",
    );
    expect(userTrafficModalSource).toContain('<LegacySpin loading={records.isFetching}>');
    expect(userTrafficModalSource).not.toContain('<Spin');
    expect(userTrafficModalSource).toContain('page: 1');
    expect(userTrafficModalSource).toContain('total: 0');
    expect(userTrafficModalSource).toContain('pagination,');
    expect(userTrafficModalSource).toContain(
      'const lastUserIdRef = useRef<number | null | undefined>(undefined);',
    );
    expect(userTrafficModalSource).toContain('if (!open || userId == null) return;');
    expect(userTrafficModalSource).toContain(
      'lastUserIdRef.current !== undefined && lastUserIdRef.current !== userId',
    );
    expect(userTrafficModalSource).toContain('lastUserIdRef.current = userId;');
    expect(userTrafficModalSource).toContain(
      'const total = records.data?.total ?? pagination.total;',
    );
    expect(userTrafficModalSource).toContain('total?: number;');
    expect(userTrafficModalSource).toContain(
      'import {\n  LegacyStandaloneTable,\n  LegacyTablePagination,\n  legacyTableRowKey,',
    );
    expect(userTrafficModalSource).toContain('<LegacyStandaloneTable');
    expect(userTrafficModalSource).toContain('<LegacyTablePagination');
    expect(userTrafficModalSource).toContain('const headers: LegacyStandaloneTableHeader[] = [');
    expect(userTrafficModalSource).toContain("{ title: '上行', alignRight: true }");
    expect(userTrafficModalSource).toContain("{ title: '下行', alignRight: true }");
    expect(userTrafficModalSource).toContain("{ title: '倍率', alignRight: true }");
    expect(userTrafficModalSource).toContain('className="ant-table-align-right"');
    expect(userTrafficModalSource).toContain(
      'className="ant-table-align-right ant-table-row-cell-last"',
    );
    expect(userTrafficModalSource).toContain(
      '<tr key={index} className="ant-table-row ant-table-row-level-0" {...legacyTableRowKey(index)}>',
    );
    expect(userTrafficModalSource).not.toContain("import { Modal, Table } from 'antd';");
    expect(userTrafficModalSource).not.toContain('page: pagination.current');
    expect(userTrafficModalSource).not.toContain(
      'if (open) setPagination({ page: 1, pageSize: 10, total: 0 });',
    );
    expect(userTrafficModalSource).not.toContain('<Table\n          loading={records.isFetching}');
    expect(userTrafficModalSource).not.toContain('rowKey={(record)');
    expect(adminQueriesSource).toContain(
      'admin.statUser(apiClient, { user_id: userId as number, ...query })',
    );
  });

  it('uses the shared legacy user management drawer for row edits', () => {
    expect(usersSource).not.toContain('<UserEditModal');
    expect(usersSource).toContain('<UserManageDrawer');
    expect(usersSource).toContain('onSaved={() => users.refetch()}');
    expect(userManageDrawerSource).toContain("import { LegacyDrawer } from './legacy-drawer';");
    expect(userManageDrawerSource).toContain("import { LegacyButton } from './legacy-button';");
    expect(userManageDrawerSource).toContain(
      "import { LegacyInput, LegacyInputGroup, LegacyTextArea } from './legacy-input';",
    );
    expect(userManageDrawerSource).toContain(
      "import { LegacySelect, type LegacySelectOption, type LegacySelectValue } from './legacy-select';",
    );
    expect(userManageDrawerSource).toContain('<LegacyDrawer');
    expect(userManageDrawerSource).toContain('cancelText="取消"');
    expect(userManageDrawerSource).toContain('width="80%"');
    expect(userManageDrawerSource).toContain('title="用户管理"');
    expect(userManageDrawerSource).toContain('v2board-drawer-action');
    expect(userManageDrawerSource).toContain('LegacyLoadingIcon,');
    expect(userManageDrawerSource).toContain(
      "className={`ant-btn ant-btn-primary${update.isPending ? ' ant-btn-loading' : ''}`}",
    );
    expect(userManageDrawerSource).toContain(
      '{update.isPending ? <LegacyLoadingIcon /> : null}',
    );
    expect(userManageDrawerSource).toContain(
      "<LegacyLoadingIcon style={{ fontSize: 24, color: '#415A94' }} />",
    );
    expect(userManageDrawerSource).not.toContain('function LegacyDrawerLoadingIcon');
    expect(userManageDrawerSource).toContain("color: '#415A94'");
    expect(userManageDrawerSource).toContain(
      'transfer_enable: user.transfer_enable as unknown as number',
    );
    expect(userManageDrawerSource).toContain('balance: user.balance as unknown as number');
    expect(userManageDrawerSource).toContain('expired_at: user.expired_at');
    expect(userManageDrawerSource).toContain('is_admin: user.is_admin');
    expect(userManageDrawerSource).toContain('is_staff: user.is_staff');
    expect(userManageDrawerSource).toContain(
      'function legacyExpiredAtDefaultValue(value: UserManageFormValues',
    );
    expect(userManageDrawerSource).toContain('value !== null && dayjs(1000 * Number(value))');
    expect(userManageDrawerSource).toContain("import { LegacySwitch } from './legacy-switch';");
    expect(userManageDrawerSource).not.toContain('function LegacySwitch({');
    expect(userManageDrawerSource).not.toContain('className={`ant-switch${checked ?');
    expect(userManageDrawerSource).not.toContain("aria-checked={checked ? 'true' : 'false'}");
    expect(userManageDrawerSource).toContain('checked={Boolean(values.is_admin)}');
    expect(userManageDrawerSource).toContain(
      "onChange={(value) => formChange('is_admin', value ? 1 : 0)}",
    );
    expect(userManageDrawerSource).toContain('checked={Boolean(values.is_staff)}');
    expect(userManageDrawerSource).toContain(
      "onChange={(value) => formChange('is_staff', value ? 1 : 0)}",
    );
    expect(userManageDrawerSource).toContain(
      'transfer_enable: scaled(values.transfer_enable, BYTE_GB)',
    );
    expect(userManageDrawerSource).toContain('function scaled(value: unknown, multiplier: number)');
    expect(userManageDrawerSource).not.toContain(
      'transfer_enable: scaledRounded(values.transfer_enable, BYTE_GB)',
    );
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
    expect(userManageDrawerSource).toContain('defaultValue={legacyDefaultValue(values.balance)}');
    expect(userManageDrawerSource).toContain(
      'defaultValue={legacyDefaultValue(values.commission_balance)}',
    );
    expect(userManageDrawerSource).toContain('defaultValue={legacyDefaultValue(values.u)}');
    expect(userManageDrawerSource).toContain('defaultValue={legacyDefaultValue(values.d)}');
    expect(userManageDrawerSource).toContain(
      'defaultValue={legacyDefaultValue(values.transfer_enable)}',
    );
    expect(userManageDrawerSource).toContain(
      'defaultValue={legacyDefaultValue(values.device_limit)}',
    );
    expect(userManageDrawerSource).toContain(
      'defaultValue={legacyExpiredAtDefaultValue(values.expired_at)}',
    );
    expect(userManageDrawerSource).toContain(
      "onChange={(value) => formChange('expired_at', value ? value.format('X') : null)}",
    );
    expect(userManageDrawerSource).toContain('<LegacyDatePicker');
    expect(userManageDrawerSource).not.toContain('<DatePicker');
    expect(userManageDrawerSource).not.toContain(
      "import { App, DatePicker, Tooltip } from 'antd';",
    );
    expect(userManageDrawerSource).toContain("import { LegacyTooltip } from './legacy-tooltip';");
    expect(userManageDrawerSource).toContain(
      '<LegacyTooltip title="设置后该用户购买任何订阅将始终享受该折扣" placement="top">',
    );
    expect(userManageDrawerSource).not.toContain("Tooltip } from 'antd'");
    expect(userManageDrawerSource).not.toContain('<Tooltip');
    expect(userManageDrawerSource).toContain(
      'defaultValue={(values.plan_id || null) as LegacySelectValue}',
    );
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
    expect(userManageDrawerSource).not.toContain('value={legacyDefaultValue(values.balance)}');
    expect(userManageDrawerSource).not.toContain(
      'value={legacyDefaultValue(values.commission_balance)}',
    );
    expect(userManageDrawerSource).not.toContain('value={legacyDefaultValue(values.u)}');
    expect(userManageDrawerSource).not.toContain('value={legacyDefaultValue(values.d)}');
    expect(userManageDrawerSource).not.toContain(
      'value={legacyDefaultValue(values.transfer_enable)}',
    );
    expect(userManageDrawerSource).not.toContain(
      'value={(values.plan_id || null) as LegacySelectValue}',
    );
    expect(userManageDrawerSource).not.toContain('value={values.banned ? 1 : 0}');
    expect(userManageDrawerSource).not.toContain(
      'value={parseInt(values.commission_type as string)}',
    );
    expect(userManageDrawerSource).not.toContain(
      'value={legacyDefaultValue(values.commission_rate)}',
    );
    expect(userManageDrawerSource).not.toContain('value={legacyDefaultValue(values.discount)}');
    expect(userManageDrawerSource).not.toContain('value={legacyDefaultValue(values.speed_limit)}');
    expect(userManageDrawerSource).not.toContain('defaultValue={values.plan_id || null}');
    expect(userManageDrawerSource).not.toContain(
      'defaultValue={Number(values.commission_type ?? 0)}',
    );
    expect(userManageDrawerSource).not.toContain(
      'defaultValue={values.invite_user_email ?? undefined}',
    );
    expect(userManageDrawerSource).not.toContain('defaultValue={values.device_limit ?? undefined}');
    expect(userManageDrawerSource).not.toContain('expiredAt ? expiredAt.unix() : null');
    expect(userManageDrawerSource).not.toContain(
      'expired_at: user.expired_at == null ? null : dayjs',
    );
    expect(userManageDrawerSource).not.toContain(
      'defaultValue={values.commission_rate ?? undefined}',
    );
    expect(userManageDrawerSource).not.toContain('defaultValue={values.discount ?? undefined}');
    expect(userManageDrawerSource).not.toContain('defaultValue={values.speed_limit ?? undefined}');
    expect(userManageDrawerSource).not.toContain('defaultValue={values.remarks ?? undefined}');
    expect(userManageDrawerSource).toContain(
      'if ((payload as Record<string, unknown>).invite_user) {',
    );
    expect(userManageDrawerSource).toContain(
      'delete (payload as Record<string, unknown>).invite_user',
    );
    expect(userManageDrawerSource).toContain('const planOptions: LegacySelectOption[] = [');
    expect(userManageDrawerSource).toContain("{ value: null, label: '无' }");
    expect(userManageDrawerSource).toContain('options={planOptions}');
    expect(userManageDrawerSource).toContain('options={LEGACY_ACCOUNT_STATUS_OPTIONS}');
    expect(userManageDrawerSource).toContain('options={LEGACY_COMMISSION_TYPE_OPTIONS}');
    expect(userManageDrawerSource).not.toContain('<Select.Option');
    expect(userManageDrawerSource).not.toContain('<Form');
    expect(userManageDrawerSource).not.toContain('<Spin');
    expect(userManageDrawerSource).not.toContain('<Drawer');
    expect(userManageDrawerSource).not.toContain('<Button');
    expect(userManageDrawerSource).not.toContain('<Input');
    expect(userManageDrawerSource).not.toContain('<Switch');
  });

  it('keeps user update triggering the page fetch before the drawer closes', () => {
    const updateStart = userManageDrawerSource.indexOf('.mutateAsync(toPayload(values, userId))');
    const updateRefetch = userManageDrawerSource.indexOf('await onSaved?.();', updateStart);
    const updateHide = userManageDrawerSource.indexOf('hide();', updateRefetch);
    const updateUserHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useUpdateUserMutation()'),
      adminQueriesSource.indexOf('export function useDeleteUserMutation()'),
    );

    expect(updateStart).toBeGreaterThan(-1);
    expect(updateRefetch).toBeGreaterThan(updateStart);
    expect(updateHide).toBeGreaterThan(updateRefetch);
    expect(userManageDrawerSource).toContain('onSaved?: () => void | Promise<unknown>;');
    expect(userManageDrawerSource).toContain('        await onSaved?.();\n        hide();');
    expect(userManageDrawerSource).not.toContain('void onSaved?.();');
    expect(userManageDrawerSource).not.toContain("message.success('操作成功')");
    expect(updateUserHook).not.toContain('onSuccess');
    expect(updateUserHook).not.toContain("queryKey: ['admin', 'users']");
  });

  it('keeps the legacy assigned-order modal loading OK text', () => {
    expect(usersSource).toContain("import { LegacyModal } from '@/components/legacy-modal';");
    expect(usersSource).toContain("import { legacyConfirm } from '@/components/legacy-confirm';");
    expect(usersSource).toContain("import { App } from 'antd';");
    expect(usersSource).toContain('LegacyInputCompactGroup,');
    expect(usersSource).toContain('LegacyDropdownMenu,');
    expect(usersSource).toContain('LegacyDropdownMenuItem,');
    expect(usersSource).toContain("} from '@/components/legacy-select';");
    expect(usersSource).toContain('LegacySelect,');
    expect(usersSource).toContain('type LegacySelectOption,');
    expect(usersSource).toContain('type LegacySelectValue,');
    expect(assignOrderModalSource).toContain('<LegacyModal');
    expect(assignOrderModalSource).toContain('visible={Boolean(user)}');
    expect(usersSource).toContain("okText={assign.isPending ? <LegacyLoadingIcon /> : '确定'}");
    expect(usersSource).not.toContain('LoadingOutlined');
    expect(usersSource).not.toContain('@ant-design/icons');
    expect(usersSource).toContain('function assignOrderSubmit(email?: string): AssignOrderSubmit');
    expect(usersSource).toContain('email: email || undefined');
    expect(usersSource).toContain('plan_id: undefined');
    expect(usersSource).toContain('period: undefined');
    expect(usersSource).toContain('total_amount: undefined');
    expect(assignOrderModalSource).toContain('setSubmit(assignOrderSubmit(user.email));');
    expect(assignOrderModalSource).toContain('setSubmit(assignOrderSubmit(user?.email));');
    expect(assignOrderModalSource).not.toContain('setSubmit({ email: user.email });');
    expect(assignOrderModalSource).toContain('.mutateAsync(submit)');
    expect(assignOrderModalSource).toContain(
      ".then(() => queryClient.invalidateQueries({ queryKey: ['admin', 'orders'] }))",
    );
    expect(assignOrderModalSource).toContain('.then(close)');
    expect(assignOrderModalSource.indexOf('.mutateAsync(submit)')).toBeLessThan(
      assignOrderModalSource.indexOf(
        ".then(() => queryClient.invalidateQueries({ queryKey: ['admin', 'orders'] }))",
      ),
    );
    expect(
      assignOrderModalSource.indexOf(
        ".then(() => queryClient.invalidateQueries({ queryKey: ['admin', 'orders'] }))",
      ),
    ).toBeLessThan(assignOrderModalSource.indexOf('.then(close)'));
    expect(assignOrderModalSource).toContain('<LegacyInput');
    expect(assignOrderModalSource).toContain('placeholder="请输入用户邮箱"');
    expect(assignOrderModalSource).toContain('<LegacyInputGroup');
    expect(assignOrderModalSource).toContain('placeholder="请输入需要支付的金额"');
    expect(assignOrderModalSource).toContain('addonAfter="¥"');
    expect(assignOrderModalSource).toContain(
      '<label htmlFor="example-text-input-alt">请选择订阅</label>',
    );
    expect(assignOrderModalSource).toContain('<LegacySelect');
    expect(assignOrderModalSource).toContain('options={planSelectOptions(plans)}');
    expect(assignOrderModalSource).toContain('options={PERIOD_OPTIONS}');
    expect(usersSource).toContain(
      'const PERIOD_OPTIONS: LegacySelectOption[] = Object.keys(PERIOD_TEXT).map',
    );
    expect(assignOrderModalSource).not.toContain('<Select.Option');
    expect(assignOrderModalSource).not.toContain('options={plans}');
    expect(assignOrderModalSource).not.toContain('options={Object.entries(PERIOD_TEXT)');
    expect(assignOrderModalSource).not.toContain('<Modal');
    expect(assignOrderModalSource).not.toContain('<Input');
    expect(assignOrderModalSource).not.toContain('open={Boolean(user)}');
    expect(assignOrderModalSource).not.toContain('<Form');
    expect(assignOrderModalSource).not.toContain('rules={[{ required: true }]}');
  });

  it('keeps the legacy create-user modal stateful layout and CSV download', () => {
    expect(usersSource.match(/<LegacyModal/g)).toHaveLength(3);
    expect(usersSource).not.toContain('<Modal');
    expect(generateUserModalSource).toContain('<LegacyModal');
    expect(generateUserModalSource).toContain('visible={open}');
    expect(generateUserModalSource).toContain('title="创建用户"');
    expect(generateUserModalSource).toContain('okText="生成"');
    expect(generateUserModalSource).toContain('okButtonProps={{ loading }}');
    expect(generateUserModalSource).toContain('<LegacyInputCompactGroup>');
    expect(generateUserModalSource).toContain('<LegacyInput');
    expect(generateUserModalSource).toContain('className="ant-input"');
    expect(generateUserModalSource).toContain('placeholder="账号（批量生成请留空）"');
    expect(generateUserModalSource).toContain('placeholder="留空则密码与邮箱相同"');
    expect(generateUserModalSource).toContain('placeholder="如果为批量生成请输入生成数量"');
    expect(generateUserModalSource).toContain('!submit.generate_count');
    expect(generateUserModalSource).toContain('!submit.email_prefix');
    expect(generateUserModalSource).toContain('<LegacyDatePicker');
    expect(generateUserModalSource).not.toContain('<DatePicker');
    expect(generateUserModalSource).not.toContain('<Input');
    expect(generateUserModalSource).not.toContain('Input.Group');
    expect(usersSource).not.toContain("Input } from 'antd'");
    expect(usersSource).not.toContain(
      "import { App, DatePicker, Dropdown, Input, Menu, Modal, Select, Tooltip } from 'antd';",
    );
    expect(generateUserModalSource).toContain(
      'defaultValue={submit.expired_at ? dayjs(1000 * Number(submit.expired_at)) : undefined}',
    );
    expect(generateUserModalSource).not.toContain(
      'value={submit.expired_at ? dayjs(1000 * Number(submit.expired_at)) : null}',
    );
    expect(generateUserModalSource).toContain(
      '<label htmlFor="example-text-input-alt">订阅计划</label>',
    );
    expect(generateUserModalSource).toContain('<LegacySelect');
    expect(generateUserModalSource).toContain('options={planSelectOptions(plans, true)}');
    expect(usersSource).toContain(
      "const GENERATE_USER_EMPTY_PLAN_OPTION: LegacySelectOption = { value: null, label: '无' };",
    );
    expect(generateUserModalSource).not.toContain('<Select.Option');
    expect(generateUserModalSource).not.toContain(
      "options={[{ value: null, label: '无' }, ...plans]}",
    );
    expect(generateUserModalSource).not.toContain('id="generate-user-plan"');
    expect(generateUserModalSource).not.toContain('<Modal');
    expect(generateUserModalSource).not.toContain('open={open}');
    expect(generateUserModalSource).not.toContain('<Form');
    expect(generateUserModalSource).not.toContain('rules={[{ required: true }]}');
    expect(usersSource).toContain('downloadGeneratedUserCsv(response.buffer)');
    expect(usersSource).toContain('return users.refetch();');
    expect(usersSource).toContain('return users.refetch();\n            })\n            .then(() => {\n              setCreating(false);');
    expect(usersSource).not.toContain('void users.refetch();\n              setCreating(false);');
    expect(usersSource).not.toContain('await users.refetch();');
    expect(usersSource).toContain("USER ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv");
    expect(usersSource).not.toContain("message.success('操作成功')");

    const downloadIndex = usersSource.indexOf('downloadGeneratedUserCsv(response.buffer)');
    const refetchIndex = usersSource.indexOf('return users.refetch();', downloadIndex);
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
    expect(usersSource).not.toContain('<Select');
    expect(usersSource).not.toContain('Select.Option');
    expect(usersSource).not.toContain("Select, Tooltip } from 'antd'");
  });

  it('keeps the original user list tooltip placements', () => {
    expect(usersSource).toContain("import { LegacyTooltip } from '@/components/legacy-tooltip';");
    expect(usersSource).toMatch(
      /<LegacyTooltip\s+placement="top"\s+title=\{legacyOnlineAt \? `最后在线\$\{formatDateTime\(Number\(legacyOnlineAt\)\)\}` : '从未在线'\}/,
    );
    expect(usersSource).toContain('<LegacyTooltip placement="top" title={row.ips}>');
    expect(usersSource).toMatch(
      /<LegacyTooltip\s+title="Tips：可以使用过滤器过滤后再使用操作对过滤的用户进行操作。"\s+placement="right"/,
    );
    expect(usersSource).not.toContain("Tooltip } from 'antd'");
    expect(usersSource).not.toContain('<Tooltip');
  });

  it('keeps the legacy send-mail modal stateful layout', () => {
    expect(sendMailModalSource).toContain('<LegacyModal');
    expect(sendMailModalSource).toContain('visible={open}');
    expect(sendMailModalSource).toContain('title="发送邮件"');
    expect(sendMailModalSource).toContain('okButtonProps={{ loading }}');
    expect(sendMailModalSource).toContain('收件人');
    expect(sendMailModalSource).toContain('<label htmlFor="example-text-input-alt">收件人</label>');
    expect(sendMailModalSource).toContain('<label htmlFor="example-text-input-alt">主题</label>');
    expect(sendMailModalSource).toContain(
      '<label htmlFor="example-text-input-alt">发送内容</label>',
    );
    expect(sendMailModalSource).toContain("filter.length ? '过滤用户' : '全部用户'");
    expect(sendMailModalSource).toContain('<LegacyInput');
    expect(sendMailModalSource).toContain('<LegacyTextArea');
    expect(sendMailModalSource).toContain('className="ant-input"');
    expect(sendMailModalSource).toContain('placeholder="请输入邮件主题"');
    expect(sendMailModalSource).toContain('rows={12}');
    expect(sendMailModalSource).toContain('placeholder="请输入邮件内容"');
    expect(sendMailModalSource).toContain('onOk={() => onSubmit(submit)}');
    expect(sendMailModalSource).not.toContain('send-mail-recipient');
    expect(sendMailModalSource).not.toContain('send-mail-subject');
    expect(sendMailModalSource).not.toContain('send-mail-content');
    expect(sendMailModalSource).not.toContain('<Modal');
    expect(sendMailModalSource).not.toContain('<Input');
    expect(sendMailModalSource).not.toContain('Input.TextArea');
    expect(sendMailModalSource).not.toContain('open={open}');
    expect(sendMailModalSource).not.toContain('<Form');
    expect(sendMailModalSource).not.toContain('rules={[{ required: true }]}');
    expect(usersSource).toContain('.mutateAsync({ filter: query.filter, ...values })');
  });

  it('uses the original drawer-style filter with select and date filter types', () => {
    expect(usersSource).not.toContain('function LegacyFilterButton');
    expect(usersSource).toContain('<LegacyFilterDrawer');
    expect(usersSource).toContain('key={query.filter.length}');
    expect(usersSource).toContain(
      "className={`ant-btn${query.filter.length > 0 ? ' ant-btn-primary' : ''}`}",
    );
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
    expect(legacyFilterDrawerSource).toContain('return (\n            <Fragment key={index}>');
    expect(legacyFilterDrawerSource).toContain('</Fragment>');
    expect(legacyFilterDrawerSource).not.toContain('key={`${filter.key}-${index}`}');
    expect(legacyFilterDrawerSource).toContain('keys.find((key) => key.key === filter.key)!');
    expect(legacyFilterDrawerSource).not.toContain('?? keys[0]!');
    expect(legacyFilterDrawerSource).toContain('<LegacyDrawer');
    expect(legacyFilterDrawerSource).toContain('width={256}');
    expect(legacyFilterDrawerSource).toContain('<LegacyDivider>');
    expect(legacyFilterDrawerSource).toContain('<LegacyDeleteIcon');
    expect(legacyFilterDrawerSource).toContain('<LegacySelect');
    expect(legacyFilterDrawerSource).toContain(
      'options={keys.map((item) => ({ value: item.key, label: item.title }))}',
    );
    expect(legacyFilterDrawerSource).toContain(
      'onChange={(key) => update(index, { key: key as string })}',
    );
    expect(legacyFilterDrawerSource).toContain('keys[keyIndex]!.condition.map');
    expect(legacyFilterDrawerSource).toContain('selected.options!.map');
    expect(legacyFilterDrawerSource).not.toContain('keys[keyIndex]?.condition ?? []');
    expect(legacyFilterDrawerSource).not.toContain('selected.options ?? []');
    expect(legacyFilterDrawerSource).toContain(
      'defaultValue={(filter.value || undefined) as LegacySelectValue | undefined}',
    );
    expect(legacyFilterDrawerSource).not.toContain(
      'value={(filter.value || undefined) as LegacySelectValue | undefined}',
    );
    expect(legacyFilterDrawerSource).toContain(
      'label: String(option.key ?? option.label ?? option.value)',
    );
    expect(legacyFilterDrawerSource).not.toContain('<Select.Option');
    expect(legacyFilterDrawerSource).not.toContain(
      "import { App, Button, DatePicker, Divider, Drawer, Input, Select } from 'antd';",
    );
    expect(legacyFilterDrawerSource).not.toContain("import { App, DatePicker } from 'antd';");
    expect(legacyFilterDrawerSource).not.toContain('@ant-design/icons');
    expect(legacyFilterDrawerSource).not.toContain('legacy-filter-key');
    expect(legacyFilterDrawerSource).not.toContain('legacy-filter-condition');
    expect(legacyFilterDrawerSource).not.toContain('legacy-filter-value');
    expect(legacyFilterDrawerSource).not.toContain('htmlFor={`legacy-filter');
    expect(legacyFilterDrawerSource).not.toContain('label: option.label');
    expect(legacyFilterDrawerSource).toContain('<LegacyDatePicker');
    expect(legacyFilterDrawerSource).not.toContain('<DatePicker');
    expect(legacyFilterDrawerSource).toContain('添加条件');
    expect(legacyFilterDrawerSource).toContain('欲检索内容不能为空');
    expect(legacyFilterDrawerSource).toContain('v2board-drawer-action');
    expect(legacyFilterDrawerSource).toContain('className="ant-btn ant-btn-danger"');
    expect(legacyFilterDrawerSource).not.toContain('danger onClick={reset}');
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
    expect(legacyFilterDrawerSource).toContain('let valid = true;');
    expect(legacyFilterDrawerSource).toContain('filters.forEach((filter) => {');
    expect(legacyFilterDrawerSource).toContain('valid = false;');
    expect(legacyFilterDrawerSource).toContain('if (!valid) return;');
    expect(legacyFilterDrawerSource).not.toContain(
      'filters.some((filter) => isBlank(filter.value))',
    );
    expect(legacyFilterDrawerSource).toContain('defaultValue={filter.value || undefined}');
    expect(legacyFilterDrawerSource).toContain(
      "onChange={(date) => update(index, { value: date && date.format('X') })}",
    );
    expect(legacyFilterDrawerSource).toContain('showTime');
    expect(legacyDatePickerSource).toContain('onChange(date, nextValue);');
    expect(legacyDatePickerSource).toContain("onChange(null, '');");
    expect(legacyFilterDrawerSource).not.toContain("date ? date.format('X') : ''");
    expect(legacyFilterDrawerSource).not.toContain('keys[keyIndex]?.condition');
  });

  it('preserves the original row right-click action menu', () => {
    expect(usersSource).toContain('id="v2board-table-dropdown"');
    expect(usersSource).toContain(
      'ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical',
    );
    expect(usersSource).toContain('onContextMenu={(event) =>');
    expect(usersSource).toContain('event.preventDefault()');
    expect(usersSource).toContain('event.clientY');
    expect(usersSource).toContain('event.clientX');
    expect(usersSource).toContain("display: contextMenu ? 'unset' : 'none'");
    expect(usersSource).toContain("runUserAction('traffic', contextMenu.user)");
    expect(usersSource).toContain("runUserAction('delete', contextMenu.user)");
  });

  it('keeps the original anchor labels in user operation dropdowns', () => {
    const rowActionStart = usersSource.indexOf('const renderUserActions');
    const rowActionSource = usersSource.slice(
      rowActionStart,
      usersSource.indexOf('return (', rowActionStart),
    );
    const toolbarSource = usersSource.slice(
      usersSource.indexOf('className="v2board-table-action"'),
      usersSource.indexOf('<LegacyButton className="ant-btn ml-2"'),
    );

    expect(usersSource).not.toContain("import type { DropdownProps } from 'antd';");
    expect(usersSource).not.toContain('popupRender={() => overlay}');
    expect(usersSource).not.toContain('<Menu>');
    expect(rowActionSource).toContain('<LegacyDropdown');
    expect(rowActionSource).toContain('trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}');
    expect(rowActionSource).toContain('overlay={');
    expect(rowActionSource).toContain(
      '<LegacyDropdownMenuItem key="edit" onContextMenu={(event) => event.stopPropagation()}>',
    );
    expect(rowActionSource).toContain("runUserAction('edit', row)");
    expect(rowActionSource).toContain('<LegacyEditIcon /> 编辑');
    expect(rowActionSource).toContain("runUserAction('assign', row)");
    expect(rowActionSource).toContain('<LegacyPlusIcon /> 分配订单');
    expect(rowActionSource).toContain("runUserAction('copy', row)");
    expect(rowActionSource).toContain('<LegacyCopyIcon /> 复制订阅URL');
    expect(rowActionSource).toContain("runUserAction('reset', row)");
    expect(rowActionSource).toContain('<LegacyReloadIcon /> 重置UUID及订阅URL');
    expect(rowActionSource).toContain(
      '<LegacyDropdownMenuItem key="orders" onClick={() => runUserAction(\'orders\', row)}>',
    );
    expect(rowActionSource).toContain('<LegacyAccountBookIcon /> TA的订单');
    expect(rowActionSource).toContain(
      '<LegacyDropdownMenuItem key="invite" onClick={() => runUserAction(\'invite\', row)}>',
    );
    expect(rowActionSource).toContain('<LegacyUsergroupAddIcon /> TA的邀请');
    expect(rowActionSource).toContain("runUserAction('traffic', row)");
    expect(rowActionSource).toContain('<LegacySolutionIcon /> TA的流量记录');
    expect(rowActionSource).toContain("runUserAction('delete', row)");
    expect(rowActionSource).toContain('<LegacyDeleteIcon /> 删除用户');
    expect(rowActionSource).not.toContain('label: <span>');
    expect(rowActionSource).not.toContain('menu={{');
    expect(rowActionSource).not.toContain('items: [');

    expect(usersSource).toContain('type AnchorHTMLAttributes');
    expect(usersSource).toContain(
      'function legacyDisabledAnchorProps(disabled: boolean): AnchorHTMLAttributes<HTMLAnchorElement>',
    );
    expect(usersSource).toContain(
      'return { disabled } as unknown as AnchorHTMLAttributes<HTMLAnchorElement>;',
    );
    expect(toolbarSource).toContain('<LegacyDropdown');
    expect(toolbarSource).toContain('overlay={');
    expect(toolbarSource).toContain('<LegacyFileExcelIcon /> 导出CSV');
    expect(toolbarSource).toContain('onClick={() => setMailOpen(true)}');
    expect(toolbarSource).toContain('<LegacyMailIcon /> 发送邮件');
    expect(toolbarSource).toContain(
      '<LegacyDropdownMenuItem key="ban" disabled={!query.filter.length}>',
    );
    expect(toolbarSource).toContain('{...legacyDisabledAnchorProps(!query.filter.length)}');
    expect(toolbarSource).toContain('<LegacyStopIcon /> 批量封禁');
    expect(toolbarSource).toContain('<LegacyDeleteIcon /> 批量删除');
    expect(toolbarSource).not.toContain('label: <span>');
    expect(toolbarSource).not.toContain('menu={{');
    expect(toolbarSource).not.toContain('items: [');
  });

  it('keeps the toolbar operation dropdown on the original default trigger', () => {
    const toolbarSource = usersSource.slice(
      usersSource.indexOf('className="v2board-table-action"'),
      usersSource.indexOf('<span className="float-right">'),
    );

    expect(toolbarSource).toContain('<LegacyDropdown');
    expect(toolbarSource).not.toContain("trigger={['click']}");
    expect(toolbarSource).not.toContain('trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}');
  });

  it('keeps the original refetch loading mask around the whole user table block', () => {
    expect(usersSource).toContain("import { LegacySpin } from '@/components/legacy-spin';");
    expect(usersSource).toContain('<LegacySpin loading={users.isFetching}>');
    expect(usersSource).not.toContain('loading={users.isLoading}');
  });

  it('keeps the legacy main table keying and confirm-button behavior', () => {
    expect(usersSource).not.toContain('rowKey="id"');
    expect(usersSource).toContain(
      'className="ant-table-align-right ant-table-row-cell-last"',
    );
    expect(usersSource).toContain(
      'className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"',
    );
    expect(usersSource).not.toContain('modal.confirm({');
    expect(usersSource.match(/void legacyConfirm\(\{/g)).toHaveLength(4);
    expect(usersSource).toContain('onOk: () => {\n        void resetSecret');
    expect(usersSource).toContain('onOk: () => {\n        void remove');
    expect(usersSource).toContain('void banUsers');
    expect(usersSource).toContain('void deleteAll');
    expect(usersSource).toContain('.mutateAsync(query.filter)');
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
    const banStart = usersSource.indexOf('void banUsers');
    const banMutate = usersSource.indexOf('.mutateAsync(query.filter)', banStart);
    const banRefetch = usersSource.indexOf('void users.refetch();', banMutate);
    const deleteAllStart = usersSource.indexOf('void deleteAll');
    const deleteAllMutate = usersSource.indexOf('.mutateAsync(query.filter)', deleteAllStart);
    const deleteAllRefetch = usersSource.indexOf('void users.refetch();', deleteAllMutate);

    expect(banStart).toBeGreaterThan(-1);
    expect(banMutate).toBeGreaterThan(banStart);
    expect(banRefetch).toBeGreaterThan(banMutate);
    expect(deleteAllStart).toBeGreaterThan(-1);
    expect(deleteAllMutate).toBeGreaterThan(deleteAllStart);
    expect(deleteAllRefetch).toBeGreaterThan(deleteAllMutate);

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
    expect(usersSource).toContain(
      'const legacyOnlineAt = (row as AdminUserRow & { t?: number | null }).t',
    );
    expect(usersSource).toContain('Date.now() / 1000 - 600 > Number(legacyOnlineAt)');
    expect(usersSource).toContain(
      "legacyOnlineAt ? `最后在线${formatDateTime(Number(legacyOnlineAt))}` : '从未在线'",
    );
    expect(usersSource).toContain(
      "<LegacyBadge status={online ? 'success' : 'default'} />",
    );
    expect(usersSource).toContain("import { LegacyBadge } from '@/components/legacy-badge';");
    expect(usersSource).not.toContain("<Badge status={online ? 'success' : 'default'} /> {email}");
    expect(usersSource).not.toContain('row.last_login_at ?? 0');
  });

  it('keeps the legacy device-count null handling and sorter', () => {
    expect(usersSource).toContain("sortableHeader('设备数', 'updated_at')");
    expect(usersSource).toContain('const deviceCount = row.alive_ip !== null ? row.alive_ip : 0;');
    expect(usersSource).toContain(
      "const deviceLimit = row.device_limit !== null ? row.device_limit : '∞';",
    );
    expect(usersSource).not.toContain('row.alive_ip ?? 0');
    expect(usersSource).not.toContain("value ?? '∞'");
  });

  it('preserves the legacy remembered user table page size habit', () => {
    expect(usersSource).toContain("const LEGACY_HABIT_KEY = 'habit'");
    expect(usersSource).toContain("const LEGACY_USER_PAGE_SIZE_KEY = 'user_manage_page_size'");
    expect(usersSource).toContain('function readLegacyUserPageSize()');
    expect(usersSource).toContain('pageSize: readLegacyUserPageSize()');
    expect(usersSource).toContain(
      'writeLegacyHabit(LEGACY_USER_PAGE_SIZE_KEY, pagination.pageSize)',
    );
    expect(usersSource).toContain(
      'const legacyHabit = stored as unknown as Record<string, unknown>;',
    );
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
    expect(usersSource).toContain('return { ...state, ...pagination };');
    expect(usersSource).toContain(
      "sort_type: state.sort === sort && state.sort_type === 'ASC' ? 'DESC' : 'ASC'",
    );
    expect(usersSource).not.toContain('current: pagination.current ?? state.current');
    expect(usersSource).not.toContain('const nextPageSize = pagination.pageSize ?? state.pageSize');
  });

  it('keeps the bundled user pagination total as the direct response field', () => {
    expect(usersSource).toContain('total={users.data?.total}');
    expect(usersSource).not.toContain('total: users.data?.total ?? 0');
  });

  it('keeps the legacy user toolbar button group spacing', () => {
    expect(usersSource).toContain('<div className="ant-btn-group">');
    expect(usersSource).toContain('</div>');
    expect(usersSource).not.toContain('<Space>');
    expect(usersSource).not.toContain('  Space,');
  });

  it('uses the old copy helper for subscription URL copying', () => {
    expect(usersSource).toContain("import { legacyCopyText } from '@/lib/legacy-copy';");
    expect(usersSource).toContain('legacyCopyText(row.subscribe_url)');
    expect(usersSource).not.toContain("message.success('复制成功')");
    expect(usersSource).not.toContain('navigator.clipboard?.writeText');
  });

  it('keeps the legacy CSV export loading message lifecycle', () => {
    expect(usersSource).toContain("message.loading('导出中')");
    expect(usersSource).toContain('message.destroy()');
    expect(usersSource).toContain("type: 'text/plain,charset=UTF-8'");
    expect(usersSource).not.toContain("type: 'text/plain;charset=UTF-8'");
    expect(usersSource).toContain('`${formatDateTime(Date.now() / 1000)}.csv`');
    expect(usersSource).toContain('response.buffer');
    expect(usersSource).toContain('new Blob([buffer as BlobPart]');
    expect(usersSource).not.toContain("String(text ?? '')");
    expect(usersSource).not.toContain('payload.buffer ?? payload.data');
  });
});
