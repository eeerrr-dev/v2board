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
          value: 1000,
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
          value: 1000,
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
    expect(html).toContain(range);
    expect(html).toContain('编辑');
    expect(html).toContain('删除');
    expect(html).not.toContain('ant-tabs');
    expect(html).not.toContain('ant-card');
  });

  it('renders /giftcard as the original standalone giftcard table', () => {
    mocks.pathname = '/giftcard';
    const html = renderToStaticMarkup(<CouponsPage />);

    expect(html).toContain('class="block border-bottom"');
    expect(html).toContain('添加礼品卡');
    expect(html).toContain('名称');
    expect(html).toContain('卡密');
    expect(html).toContain('套餐');
    expect(html).toContain('余额卡');
    expect(html).toContain('CARD10');
    expect(html).toContain('10.00 ¥');
    expect(html).toContain('VIP');
    expect(html).toContain('字符串套餐卡');
    expect(html).toContain('CARD-STRING');
    expect(html).toContain('编辑');
    expect(html).toContain('删除');
    expect(html).not.toContain('ant-tabs');
    expect(html).not.toContain('ant-card');
  });

  it('keeps the legacy CSV download for batch coupon and giftcard generation', () => {
    expect(source).toContain("function downloadGeneratedCsv(prefix: 'COUPON' | 'GIFTCARD'");
    expect(source).toContain('window.URL.createObjectURL(blob)');
    expect(source).toContain("if (payload.generate_count) downloadGeneratedCsv('COUPON', response.buffer)");
    expect(source).toContain("if (payload.generate_count) downloadGeneratedCsv('GIFTCARD', response.buffer)");
    expect(source).toContain("`${prefix} ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv`");
    expect(source).not.toContain('useQueryClient');

    const couponDownload = source.indexOf("downloadGeneratedCsv('COUPON', response.buffer)");
    const couponRefetch = source.indexOf('void coupons.refetch();');
    const couponClose = source.indexOf('modalVisible();', couponRefetch);
    expect(couponDownload).toBeGreaterThan(-1);
    expect(couponRefetch).toBeGreaterThan(couponDownload);
    expect(couponClose).toBeGreaterThan(couponRefetch);
    expect(source).not.toContain('await coupons.refetch();');

    const giftcardDownload = source.indexOf("downloadGeneratedCsv('GIFTCARD', response.buffer)");
    const giftcardRefetch = source.indexOf('void giftcards.refetch();');
    const giftcardClose = source.indexOf('modalVisible();', giftcardRefetch);
    expect(giftcardDownload).toBeGreaterThan(-1);
    expect(giftcardRefetch).toBeGreaterThan(giftcardDownload);
    expect(giftcardClose).toBeGreaterThan(giftcardRefetch);
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
      ['export function useDropCouponMutation()', 'export function useShowCouponMutation()', "['admin', 'coupons']"],
      ['export function useShowCouponMutation()', 'export function useGenerateGiftcardMutation()', "['admin', 'coupons']"],
      ['export function useDropGiftcardMutation()', 'export function useSaveKnowledgeMutation()', "['admin', 'giftcards']"],
    ] as const) {
      const hook = queriesSource.slice(queriesSource.indexOf(start), queriesSource.indexOf(end));
      expect(hook).not.toContain('onSuccess');
      expect(hook).not.toContain(queryKey);
    }
  });

  it('keeps the legacy table sort payload when sorting is cleared', () => {
    expect(source).toContain('sort: current?.columnKey == null ? undefined : String(current.columnKey)');
    expect(source).not.toContain("sort: String(current?.columnKey ?? '')");
  });

  it('keeps coupon and giftcard table query state in module scope like the old models', () => {
    expect(source).toContain('let legacyCouponQuery: AdminPageQuery = { current: 1, pageSize: 10 };');
    expect(source).toContain('let legacyGiftcardQuery: AdminPageQuery = { current: 1, pageSize: 10 };');
    expect(source).toContain(
      'const [query, setQueryState] = useState<AdminPageQuery>(() => legacyCouponQuery);',
    );
    expect(source).toContain(
      'const [query, setQueryState] = useState<AdminPageQuery>(() => legacyGiftcardQuery);',
    );
    expect(source).toContain('legacyCouponQuery = next;');
    expect(source).toContain('legacyGiftcardQuery = next;');
    expect(source.match(/\\.\\.\\.pagination,/g)).toHaveLength(2);
    expect(source).not.toContain('current: pagination.current');
    expect(source).not.toContain('pageSize: pagination.pageSize');
    expect(source).not.toContain(
      'const [query, setQuery] = useState<AdminPageQuery>({ current: 1, pageSize: 10 });',
    );
  });

  it('keeps bundled coupon and giftcard pagination totals as direct response fields', () => {
    expect(source).toContain('total: coupons.data?.total,');
    expect(source).toContain('total: giftcards.data?.total,');
    expect(source).not.toContain('total: coupons.data?.total ?? 0');
    expect(source).not.toContain('total: giftcards.data?.total ?? 0');
  });

  it('keeps the old unconditional amount scaling before coupon and giftcard generation', () => {
    expect(source).toContain('if (payload.type === 1) payload.value = 100 * Number(payload.value)');
    expect(source).not.toContain('payload.type === 1 && payload.value != null');
    expect(source.match(/payload\.value = 100 \* Number\(payload\.value\)/g)).toHaveLength(2);
  });

  it('keeps the original direct date range indexing for coupon and giftcard forms', () => {
    expect(source.match(/const range = dates as \[Dayjs \| null, Dayjs \| null\];/g)).toHaveLength(2);
    expect(source.match(/started_at: range\[0\] \? range\[0\]\.format\('X'\) : null/g)).toHaveLength(2);
    expect(source.match(/ended_at: range\[1\] \? range\[1\]\.format\('X'\) : null/g)).toHaveLength(2);
    expect(source).not.toContain('dates?.[0]');
    expect(source).not.toContain('dates?.[1]');
  });

  it('uses the original fetchLoading-style page spinner for coupon and giftcard refetches', () => {
    const loadingMatches = source.match(/<LegacySpin loading=\{(coupons|giftcards)\.isFetching\}>/g) ?? [];

    expect(loadingMatches).toHaveLength(2);
    expect(source).not.toContain('loading={coupons.isLoading}');
    expect(source).not.toContain('loading={giftcards.isLoading}');
  });

  it('uses the old copy helper for coupon and giftcard code copying', () => {
    expect(source).toContain("import { legacyCopyText } from '@/lib/legacy-copy';");
    expect(source).toContain('legacyCopyText(text)');
    expect(source).not.toContain('navigator.clipboard?.writeText');
  });

  it('keeps the original strict giftcard plan lookup', () => {
    expect(source).toContain('plan.id === id');
    expect(source).not.toContain('String(plan.id) === String(id)');
  });

  it('keeps the original direct coupon and giftcard form value bindings', () => {
    expect(source).toContain('checked={value as unknown as boolean}');
    expect(source).toContain('value={submit.limit_use as string | number | undefined}');
    expect(source).toContain('value={submit.limit_use_with_user as string | number | undefined}');
    expect(source).toContain('value={submit.plan_id as string | number | undefined}');
    expect(source).toContain("mode={'single' as 'multiple'}");
    expect(source).toContain(
      "plan_id: ((value as string).length ? value : null) as GiftcardSubmit['plan_id']",
    );
    expect(source).not.toContain('checked={Boolean(value)}');
    expect(source).not.toContain('submit.limit_use ?? undefined');
    expect(source).not.toContain('submit.limit_use_with_user ?? undefined');
    expect(source).not.toContain('submit.plan_id ?? undefined');
  });

  it('keeps the original random keys for dynamic coupon and giftcard select options', () => {
    expect(source.match(/key=\{Math\.random\(\)\}/g)).toHaveLength(3);
    expect(source).not.toContain('key={plan.id}');
    expect(source).not.toContain('key={period}');
  });

  it('keeps the original coupon period labels', () => {
    expect(source).toContain("reset_price: '流量重置包'");
    expect(source).not.toContain("reset_price: '重置流量'");
  });

  it('keeps the original coupon and giftcard table row identity behavior', () => {
    expect(source).not.toContain('rowKey="id"');
  });

  it('keeps the original add-button spacing for coupon and giftcard pages', () => {
    expect(source).toContain('<PlusOutlined /> 添加优惠券');
    expect(source).toContain('<PlusOutlined />添加礼品卡');
    expect(source).not.toContain('<PlusOutlined />\n                添加礼品卡');
  });

  it('keeps the original edit action wired to the table data index', () => {
    expect(source).toContain('render: (_value, row, index) =>');
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
