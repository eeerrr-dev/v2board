import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import dayjs from 'dayjs';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import CouponsPage from './coupons';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'coupons.tsx'), 'utf8');
const queriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);

const mocks = vi.hoisted(() => ({
  pathname: '/coupon',
}));

vi.mock('react-router-dom', () => ({
  useLocation: () => ({ pathname: mocks.pathname }),
}));

vi.mock('@/lib/queries', () => ({
  useAdminPlans: () => ({
    data: [{ id: 1, name: 'VIP' }],
  }),
  useAdminCoupons: () => ({
    isLoading: false,
    isFetching: false,
    data: {
      data: [
        {
          id: 1,
          code: 'SAVE10',
          name: '十元优惠',
          type: 1,
          value: 10,
          show: 1,
          limit_use: null,
          limit_use_with_user: null,
          limit_plan_ids: null,
          limit_period: null,
          started_at: 1700000000,
          ended_at: 1700086400,
          created_at: 1700000000,
          updated_at: 1700000000,
        },
      ],
      total: 2,
    },
    refetch: vi.fn(),
  }),
  useAdminGiftcards: () => ({
    isLoading: false,
    isFetching: false,
    data: {
      data: [
        {
          id: 1,
          name: '余额卡',
          code: 'CARD10',
          type: 1,
          value: 10,
          plan_id: 1,
          limit_use: null,
          used_user_ids: null,
          started_at: 1700000000,
          ended_at: 1700086400,
          created_at: 1700000000,
          updated_at: 1700000000,
        },
        {
          id: 2,
          name: '字符串套餐卡',
          code: 'CARD-STRING',
          type: 5,
          value: 30,
          plan_id: '1',
          limit_use: null,
          used_user_ids: null,
          started_at: 1700000000,
          ended_at: 1700086400,
          created_at: 1700000000,
          updated_at: 1700000000,
        },
      ],
      total: 1,
    },
    refetch: vi.fn(),
  }),
  useGenerateCouponMutation: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
  useDropCouponMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useShowCouponMutation: () => ({
    mutate: vi.fn(),
  }),
  useGenerateGiftcardMutation: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
  useDropGiftcardMutation: () => ({
    mutateAsync: vi.fn(),
  }),
}));

beforeEach(() => {
  mocks.pathname = '/coupon';
});

describe('CouponsPage legacy routes', () => {
  it('renders /coupon as the original standalone coupon table', () => {
    const html = renderToStaticMarkup(<CouponsPage />);
    const range = `${dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm')} ~ ${dayjs(
      1700086400 * 1000,
    ).format('YYYY/MM/DD HH:mm')}`;

    expect(html).toContain('class="block border-bottom"');
    expect(html).toContain('class="bg-white"');
    expect(html).toContain('添加优惠券');
    expect(html).toContain('class="ant-btn"');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('<span> 添加优惠券</span>');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain(
      'class="ant-table ant-table-default ant-table-scroll-position-left ant-table-scroll-position-right"',
    );
    expect(html).toContain('class="ant-table-scroll"');
    expect(html).toContain('tabindex="-1" class="ant-table-body" style="overflow-x:scroll"');
    expect(html).toContain('class="ant-table-fixed" style="width:1050px"');
    expect(html).toContain('class="ant-table-fixed-right"');
    expect(html).toContain('class="ant-table-align-left" style="text-align:left"');
    expect(html).toContain(
      'class="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last" style="text-align:right"',
    );
    expect(html).toContain('class="ant-switch-small ant-switch ant-switch-checked"');
    expect(html).toContain('class="ant-switch-inner"');
    expect(html).toContain('启用');
    expect(html).toContain('券名称');
    expect(html).toContain('类型');
    expect(html).toContain('券码');
    expect(html).toContain('剩余次数');
    expect(html).toContain('有效期');
    expect(html).toContain('十元优惠');
    expect(html).toContain('SAVE10');
    expect(html).toContain('金额');
    expect(html).toContain('无限');
    expect(html).toContain('ant-tag');
    expect(html).toContain(range);
    expect(html).toContain('编辑');
    expect(html).toContain('删除');
    expect(html).not.toContain('ant-tabs');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-table-cell');
    expect(html).not.toContain('ant-typography');
  });

  it('renders /giftcard as the original standalone giftcard table', () => {
    mocks.pathname = '/giftcard';
    const html = renderToStaticMarkup(<CouponsPage />);

    expect(html).toContain('class="block border-bottom"');
    expect(html).toContain('添加礼品卡');
    expect(html).toContain('class="ant-btn"');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('<span>添加礼品卡</span>');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain(
      'class="ant-table ant-table-default ant-table-scroll-position-left ant-table-scroll-position-right"',
    );
    expect(html).toContain('class="ant-table-scroll"');
    expect(html).toContain('tabindex="-1" class="ant-table-body" style="overflow-x:scroll"');
    expect(html).toContain('class="ant-table-fixed" style="width:1050px"');
    expect(html).toContain('class="ant-table-fixed-right"');
    expect(html).toContain('class="ant-table-align-left" style="text-align:left"');
    expect(html).toContain('名称');
    expect(html).toContain('卡密');
    expect(html).toContain('套餐');
    expect(html).toContain('余额卡');
    expect(html).toContain('CARD10');
    expect(html).toContain('ant-tag');
    expect(html).toContain('10.00 ¥');
    expect(html).toContain('VIP');
    expect(html).toContain('字符串套餐卡');
    expect(html).toContain('CARD-STRING');
    expect(html).toContain('编辑');
    expect(html).toContain('删除');
    expect(html).not.toContain('ant-tabs');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-table-cell');
    expect(html).not.toContain('ant-typography');
  });

  it('keeps the legacy CSV download for batch coupon and giftcard generation', () => {
    expect(source).toContain("function downloadGeneratedCsv(prefix: 'COUPON' | 'GIFTCARD'");
    expect(source).toContain('window.URL.createObjectURL(blob)');
    expect(source).toContain(
      "if (payload.generate_count) downloadGeneratedCsv('COUPON', response.buffer)",
    );
    expect(source).toContain(
      "if (payload.generate_count) downloadGeneratedCsv('GIFTCARD', response.buffer)",
    );
    expect(source).toContain("`${prefix} ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv`");
    expect(source).not.toContain('useQueryClient');

    const couponDownload = source.indexOf("downloadGeneratedCsv('COUPON', response.buffer)");
    const couponRefetch = source.indexOf('void coupons.refetch();');
    const couponClose = source.indexOf('modalVisible();', couponRefetch);
    expect(couponDownload).toBeGreaterThan(-1);
    expect(couponRefetch).toBeGreaterThan(couponDownload);
    expect(couponClose).toBeGreaterThan(couponRefetch);

    const giftcardDownload = source.indexOf("downloadGeneratedCsv('GIFTCARD', response.buffer)");
    const giftcardRefetch = source.indexOf('void giftcards.refetch();');
    const giftcardClose = source.indexOf('modalVisible();', giftcardRefetch);
    expect(giftcardDownload).toBeGreaterThan(-1);
    expect(giftcardRefetch).toBeGreaterThan(giftcardDownload);
    expect(giftcardClose).toBeGreaterThan(giftcardRefetch);
    expect(source).not.toContain('await coupons.refetch();');
    expect(source).not.toContain('await giftcards.refetch();');

    const generateCouponHook = queriesSource.slice(
      queriesSource.indexOf('export function useGenerateCouponMutation()'),
      queriesSource.indexOf('export function useDropCouponMutation()'),
    );
    const generateGiftcardHook = queriesSource.slice(
      queriesSource.indexOf('export function useGenerateGiftcardMutation()'),
      queriesSource.indexOf('export function useDropGiftcardMutation()'),
    );
    expect(generateCouponHook).not.toContain('onSuccess');
    expect(generateGiftcardHook).not.toContain('onSuccess');
  });

  it('keeps coupon and giftcard row actions fetching from the page after success', () => {
    const showStart = source.indexOf('show.mutate(row.id, {');
    const showRefetch = source.indexOf('void coupons.refetch();', showStart);
    const couponDropStart = source.indexOf('drop.mutateAsync(row.id).then');
    const couponDropRefetch = source.indexOf('void coupons.refetch();', couponDropStart);
    const giftcardDropStart = source.indexOf('drop.mutateAsync(row.id).then', couponDropStart + 1);
    const giftcardDropRefetch = source.indexOf('void giftcards.refetch();', giftcardDropStart);

    expect(showStart).toBeGreaterThan(-1);
    expect(showRefetch).toBeGreaterThan(showStart);
    expect(couponDropStart).toBeGreaterThan(-1);
    expect(couponDropRefetch).toBeGreaterThan(couponDropStart);
    expect(giftcardDropStart).toBeGreaterThan(couponDropStart);
    expect(giftcardDropRefetch).toBeGreaterThan(giftcardDropStart);

    for (const [start, end, queryKey] of [
      [
        'export function useDropCouponMutation()',
        'export function useShowCouponMutation()',
        "['admin', 'coupons']",
      ],
      [
        'export function useShowCouponMutation()',
        'export function useGenerateGiftcardMutation()',
        "['admin', 'coupons']",
      ],
      [
        'export function useDropGiftcardMutation()',
        'export function useSaveKnowledgeMutation()',
        "['admin', 'giftcards']",
      ],
    ] as const) {
      const hook = queriesSource.slice(queriesSource.indexOf(start), queriesSource.indexOf(end));
      expect(hook).not.toContain('onSuccess');
      expect(hook).not.toContain(queryKey);
    }
  });

  it('keeps the legacy table sort payload when sorting is cleared', () => {
    expect(source).toContain(
      'sort: current?.columnKey == null ? undefined : String(current.columnKey)',
    );
    expect(source).not.toContain("sort: String(current?.columnKey ?? '')");
  });

  it('keeps coupon and giftcard table query state in module scope like the old models', () => {
    expect(source).toContain(
      'let legacyCouponQuery: AdminPageQuery = { current: 1, pageSize: 10 };',
    );
    expect(source).toContain(
      'let legacyGiftcardQuery: AdminPageQuery = { current: 1, pageSize: 10 };',
    );
    expect(source).toContain(
      'const [query, setQueryState] = useState<AdminPageQuery>(() => legacyCouponQuery);',
    );
    expect(source).toContain(
      'const [query, setQueryState] = useState<AdminPageQuery>(() => legacyGiftcardQuery);',
    );
    expect(source).toContain('legacyCouponQuery = next;');
    expect(source).toContain('legacyGiftcardQuery = next;');
    expect(source.match(/\.\.\.pagination,/g) ?? []).toHaveLength(0);
    expect(source).not.toContain('current: pagination.current');
    expect(source).not.toContain('pageSize: pagination.pageSize');
    expect(source).not.toContain(
      'const [query, setQuery] = useState<AdminPageQuery>({ current: 1, pageSize: 10 });',
    );
  });

  it('renders coupon and giftcard lists with the legacy standalone table instead of AntD5 Table props', () => {
    expect(source.match(/<LegacyStandaloneTable/g)).toHaveLength(2);
    expect(source.match(/scrollX=\{1050\}/g)).toHaveLength(2);
    expect(source.match(/fixedRightChildren=\{data\.map/g)).toHaveLength(2);
    expect(source.match(/\{ title: '有效期', alignLeft: true \}/g)).toHaveLength(2);
    expect(source.match(/\{ title: '操作', alignRight: true, fixedRight: true \}/g)).toHaveLength(
      2,
    );
    expect(source).toContain('isEmpty={data.length === 0}');
    expect(source).not.toContain('<Table<Coupon>');
    expect(source).not.toContain('<Table<Giftcard>');
    expect(source).not.toContain('total: coupons.data?.total,');
    expect(source).not.toContain('total: giftcards.data?.total,');
    expect(source).not.toContain('pagination={{');
  });

  it('keeps the old unconditional amount scaling before coupon and giftcard generation', () => {
    expect(source).toContain('if (payload.type === 1) payload.value = 100 * Number(payload.value)');
    expect(source).not.toContain('payload.type === 1 && payload.value != null');
    expect(source.match(/payload\.value = 100 \* Number\(payload\.value\)/g)).toHaveLength(2);
  });

  it('uses already-normalized legacy model data instead of re-scaling table rows during render', () => {
    expect(source).toContain('const data = coupons.data?.data ?? [];');
    expect(source).toContain('const data = giftcards.data?.data ?? [];');
    expect(source).not.toContain('coupon.value / 100');
    expect(source).not.toContain('giftcard.value / 100');
  });

  it('keeps the original direct date range indexing for coupon and giftcard forms', () => {
    expect(source.match(/const range = dates as \[Dayjs \| null, Dayjs \| null\];/g)).toHaveLength(
      2,
    );
    expect(
      source.match(/started_at: range\[0\] \? range\[0\]\.format\('X'\) : null/g),
    ).toHaveLength(2);
    expect(source.match(/ended_at: range\[1\] \? range\[1\]\.format\('X'\) : null/g)).toHaveLength(
      2,
    );
    expect(source).not.toContain('dates?.[0]');
    expect(source).not.toContain('dates?.[1]');
  });

  it('uses the original fetchLoading-style page spinner for coupon and giftcard refetches', () => {
    const loadingMatches =
      source.match(/<LegacySpin loading=\{(coupons|giftcards)\.isFetching\}>/g) ?? [];

    expect(loadingMatches).toHaveLength(2);
    expect(source).not.toContain('loading={coupons.isLoading}');
    expect(source).not.toContain('loading={giftcards.isLoading}');
  });

  it('uses the old copy helper for coupon and giftcard code copying', () => {
    expect(source).toContain("import { legacyCopyText } from '@/lib/legacy-copy';");
    expect(source).toContain('legacyCopyText(text)');
    expect(
      source.match(
        /<LegacyTag style=\{\{ cursor: 'pointer' \}\} onClick=\{\(\) => copy\(value\)\}>/g,
      ),
    ).toHaveLength(2);
    expect(source).toContain("import { App } from 'antd';");
    expect(source).toContain("import { LegacyButton } from '@/components/legacy-button';");
    expect(source).toContain("import { LegacyPlusIcon } from '@/components/legacy-ant-icon';");
    expect(source).toContain(
      "import { LegacyRangePicker } from '@/components/legacy-range-picker';",
    );
    expect(source).toContain("import { LegacySwitch } from '@/components/legacy-switch';");
    expect(source).toContain("import { LegacyTag } from '@/components/legacy-tag';");
    expect(source).toContain("} from '@/components/legacy-select';");
    expect(source).toContain('LegacySelect,');
    expect(source).toContain('type LegacySelectOption,');
    expect(source).toContain('type LegacySelectValue,');
    expect(source).toContain('<LegacySwitch');
    expect(source.match(/<LegacySelect/g)).toHaveLength(5);
    expect(source).toContain(
      'LegacyStandaloneTable,\n  legacyTableRowKey,\n  type LegacyStandaloneTableHeader,',
    );
    expect(source).not.toContain("import { PlusOutlined } from '@ant-design/icons';");
    expect(source).not.toContain('Button, DatePicker');
    expect(source).not.toContain('Table, Tag');
    expect(source).not.toContain('Typography.Text');
    expect(source).not.toContain("Typography } from 'antd'");
    expect(source).not.toContain('Modal, Select');
    expect(source).not.toContain('Modal, Tag');
    expect(source).not.toContain('<Tag');
    expect(source).not.toContain('<Select');
    expect(source).not.toContain('Select.Option');
    expect(source).not.toContain('<Switch');
    expect(source).not.toContain('Switch, Tag');
    expect(source).not.toContain('navigator.clipboard?.writeText');
  });

  it('uses the old Ant Design range picker shell for coupon and giftcard validity forms', () => {
    expect(source.match(/<LegacyRangePicker/g)).toHaveLength(2);
    expect(source.match(/showTime=\{\{ format: 'HH:mm' \}\}/g)).toHaveLength(2);
    expect(source.match(/format="YYYY-MM-DD HH:mm"/g)).toHaveLength(2);
    expect(source.match(/placeholder=\{\['Start Time', 'End Time'\]\}/g)).toHaveLength(2);
    expect(
      source.match(/value=\{rangeValue\(submit\.started_at, submit\.ended_at\)\}/g),
    ).toHaveLength(2);
    expect(source.match(/onOk=\{onRangeChange\}/g)).toHaveLength(2);
    expect(source).not.toContain('<DatePicker.RangePicker');
    expect(source).not.toContain("DatePicker } from 'antd'");
  });

  it('uses the old Ant Design modal shell for coupon and giftcard forms', () => {
    const couponModalStart = source.indexOf('<LegacyModal');
    const couponModalEnd = source.indexOf('</LegacyModal>', couponModalStart);
    const couponModalBlock = source.slice(couponModalStart, couponModalEnd);
    const giftcardModalStart = source.indexOf('<LegacyModal', couponModalEnd);
    const giftcardModalEnd = source.indexOf('</LegacyModal>', giftcardModalStart);
    const giftcardModalBlock = source.slice(giftcardModalStart, giftcardModalEnd);

    expect(source).toContain("import { LegacyModal } from '@/components/legacy-modal';");
    expect(source.match(/<LegacyModal/g)).toHaveLength(2);
    expect(source.match(/visible=\{visible\}/g)).toHaveLength(2);
    expect(source.match(/okText="提交"/g)).toHaveLength(2);
    expect(source.match(/cancelText="取消"/g)).toHaveLength(2);
    expect(source.match(/okButtonProps=\{\{ loading: generate\.isPending \}\}/g)).toHaveLength(2);
    expect(couponModalBlock).toContain("title={`${submit.id ? '编辑优惠券' : '新建优惠券'}`}");
    expect(giftcardModalBlock).toContain("title={`${submit.id ? '编辑礼品卡' : '新建礼品卡'}`}");
    expect(source).toContain("import { legacyConfirm } from '@/components/legacy-confirm';");
    expect(source.match(/void legacyConfirm\(\{/g)).toHaveLength(2);
    expect(source).not.toContain('Modal.confirm({');
    expect(source).not.toContain('<Modal');
    expect(source).not.toContain('open={visible}');
  });

  it('uses the legacy coupon and giftcard input shells', () => {
    expect(source).toContain(
      "import { LegacyInput, LegacyInputGroup } from '@/components/legacy-input';",
    );
    expect(source).toContain('<LegacyInput');
    expect(source).toContain('<LegacyInputGroup');
    expect(source).toContain('className="ant-input"');
    expect(source).toContain('placeholder="请输入优惠券名称"');
    expect(source).toContain('placeholder="自定义优惠券码(留空随机生成)"');
    expect(source).toContain('placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"');
    expect(source).toContain('placeholder="限制每个用户可使用次数(为空则不限制)"');
    expect(source).toContain('placeholder="输入数量批量生成"');
    expect(source).toContain('placeholder="请输入礼品卡名称"');
    expect(source).toContain('placeholder="自定义礼品卡卡密(留空随机生成)"');
    expect(source).toContain('addonAfter={submit.type === 1 ?');
    expect(source).toContain('addonAfter={legacyGiftcardValueAddon(submit.type)}');
    expect(source).toContain('addonBefore={');
    expect(source).not.toContain("import { App, Input } from 'antd';");
    expect(source).not.toContain('<Input');
  });

  it('keeps coupon and giftcard limit-use cells wrapped in the old ant tag DOM', () => {
    expect(source).toContain('const renderCouponLimitUse = (value: number | null) => (');
    expect(source).toContain('const renderGiftcardLimitUse = (value: number | null) => (');
    expect(
      source.match(/<LegacyTag>\{value !== null \? value : '无限'\}<\/LegacyTag>/g),
    ).toHaveLength(2);
    expect(source).not.toContain(
      "render: (value: number | null) => (value !== null ? value : '无限'),",
    );
    expect(source).not.toContain(
      "<Typography.Text>{value !== null ? value : '无限'}</Typography.Text>",
    );
  });

  it('keeps the original strict giftcard plan lookup', () => {
    expect(source).toContain('plan.id === id');
    expect(source).not.toContain('String(plan.id) === String(id)');
  });

  it('keeps the original direct coupon and giftcard form value bindings', () => {
    expect(source).toContain('checked={value as unknown as boolean}');
    expect(source).toContain('value={submit.limit_use as string | number | undefined}');
    expect(source).toContain('value={submit.limit_use_with_user as string | number | undefined}');
    expect(source).toContain('value={submit.plan_id as LegacySelectValue | undefined}');
    expect(source.match(/mode="multiple"/g)).toHaveLength(2);
    expect(source).toMatch(
      /plan_id:\s*\(String\(value \?\? ''\)\.length\s*\?\s*value\s*:\s*null\) as GiftcardSubmit\['plan_id'\]/,
    );
    expect(source).not.toContain('checked={Boolean(value)}');
    expect(source).not.toContain('submit.limit_use ?? undefined');
    expect(source).not.toContain('submit.limit_use_with_user ?? undefined');
    expect(source).not.toContain('submit.plan_id ?? undefined');
    expect(source).not.toContain("mode={'single' as 'multiple'}");
  });

  it('keeps the bundled giftcard value unit switch', () => {
    const addonSource = source.slice(
      source.indexOf('function legacyGiftcardValueAddon'),
      source.indexOf('function useCopy'),
    );

    expect(addonSource).toContain('switch (type)');
    expect(addonSource).toContain("case 1:\n      return '¥';");
    expect(addonSource).toContain("case 2:\n      return '天';");
    expect(addonSource).toContain("case 3:\n      return 'GB';");
    expect(addonSource).toContain("case 4:\n      return '';");
    expect(addonSource).toContain("case 5:\n      return '天';");
    expect(source).toContain('addonAfter={legacyGiftcardValueAddon(submit.type)}');
    expect(source).not.toContain("submit.type === 2 || submit.type === 5 ? '天'");
  });

  it('keeps the original coupon and giftcard select option values', () => {
    expect(source).toContain('const COUPON_TYPE_OPTIONS: LegacySelectOption[] = [');
    expect(source).toContain("{ value: 1, label: '按金额优惠' }");
    expect(source).toContain("{ value: 2, label: '按比例优惠' }");
    expect(source).toContain('const GIFTCARD_TYPE_OPTIONS: LegacySelectOption[] = [');
    expect(source).toContain("{ value: 1, label: '增加账户余额' }");
    expect(source).toContain("{ value: 2, label: '增加订阅时长' }");
    expect(source).toContain("{ value: 3, label: '增加套餐流量' }");
    expect(source).toContain("{ value: 4, label: '重置套餐流量' }");
    expect(source).toContain("{ value: 5, label: '兑换订阅套餐' }");
    expect(source).toContain(
      'const PERIOD_OPTIONS: LegacySelectOption[] = Object.keys(PERIOD_TEXT).map',
    );
    expect(source).toContain(
      'function planOptions(plans: Plan[] | undefined): LegacySelectOption[]',
    );
    expect(source).toContain('value: `${plan.id}`');
    expect(source).toContain('label: plan.name');
    expect(source).toContain('options={COUPON_TYPE_OPTIONS}');
    expect(source).toContain('options={GIFTCARD_TYPE_OPTIONS}');
    expect(source).toContain('options={planOptions(plans.data)}');
    expect(source).toContain('options={PERIOD_OPTIONS}');
    expect(source).not.toContain('key={plan.id}');
    expect(source).not.toContain('key={period}');
  });

  it('keeps the original coupon period labels', () => {
    expect(source).toContain("reset_price: '流量重置包'");
    expect(source).not.toContain("reset_price: '重置流量'");
  });

  it('keeps the original coupon and giftcard table row identity behavior', () => {
    expect(source).not.toContain('rowKey="id"');
    expect(source).toContain('{...legacyTableRowKey(index)}');
    expect(source.match(/className="ant-table-row ant-table-row-level-0"/g)).toHaveLength(4);
  });

  it('keeps the original add-button spacing for coupon and giftcard pages', () => {
    expect(source).toContain('<LegacyPlusIcon />');
    expect(source).toContain('<span> 添加优惠券</span>');
    expect(source).toContain('<span>添加礼品卡</span>');
    expect(source).not.toContain('<span> 添加礼品卡</span>');
    expect(source).not.toContain('<PlusOutlined />');
  });

  it('keeps the original edit action wired to the table data index', () => {
    expect(source).toContain('const renderCouponActions = (row: Coupon, index: number) =>');
    expect(source).toContain('const renderGiftcardActions = (row: Giftcard, index: number) =>');
    expect(source).toContain('setSubmit(data[index] as CouponSubmit);');
    expect(source).toContain('setSubmit(data[index] as GiftcardSubmit);');
    expect(source).not.toContain('setSubmit(data[index] ?? row)');
    expect(source).not.toContain('setVisible(true);');
  });

  it('resets coupon and giftcard form state after the modal becomes hidden', () => {
    expect(source.match(/if \(!visible\) setSubmit\(\{ type: 1 \}\);/g)).toHaveLength(2);
    expect(source.match(/setVisible\(\(current\) => !current\);/g)).toHaveLength(2);
    expect(source).not.toContain('if (current) setSubmit({ type: 1 });');
    expect(source).not.toContain('setVisible((current) => {');
  });

  it('keeps the original vertical divider markup in coupon and giftcard action columns', () => {
    expect(source.match(/<div className="ant-divider ant-divider-vertical" \/>/g)).toHaveLength(2);
    expect(source).not.toContain('<span className="ant-divider ant-divider-vertical"');
    expect(source).not.toContain('role="separator"');
  });

  it('keeps the original delete confirm from entering promise loading state', () => {
    expect(source).not.toContain('onOk: () => drop.mutateAsync(row.id)');
    expect(source.match(/void drop\.mutateAsync\(row\.id\)\.then/g)).toHaveLength(2);
  });
});
