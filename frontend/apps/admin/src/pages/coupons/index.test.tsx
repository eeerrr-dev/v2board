import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import dayjs from 'dayjs';
import CouponsPage from './coupons';

// The admin coupon/giftcard manager is a redesigned shadcn island (PageHeader +
// DataTable + Sheet editor). Legacy ant-table / ant-modal DOM byte-pins are
// retired. What stays covered is the Tier-1 contract: the fetch page shape, the
// generate payload with raw display decimals (the api-client owns cents
// conversion) / unix-second date encoding, the plan/period restriction arrays,
// drop/show, and the type-driven value semantics.

const COUPON = {
  id: 1,
  code: 'SAVE10',
  name: '十元优惠',
  type: 1,
  value: 10, // api-client already divided cents → yuan for type 1
  show: true,
  limit_use: null,
  limit_use_with_user: null,
  limit_plan_ids: null,
  limit_period: null,
  started_at: '2023-11-14T22:13:20Z',
  ended_at: '2023-11-15T22:13:20Z',
  created_at: '2023-11-14T22:13:20Z',
  updated_at: '2023-11-14T22:13:20Z',
};

const GIFTCARD = {
  id: 1,
  name: '余额卡',
  code: 'CARD10',
  type: 1,
  value: 10,
  plan_id: 1,
  limit_use: null,
  used_user_ids: [],
  started_at: '2023-11-14T22:13:20Z',
  ended_at: '2023-11-15T22:13:20Z',
  created_at: '2023-11-14T22:13:20Z',
  updated_at: '2023-11-14T22:13:20Z',
};

const mocks = vi.hoisted(() => ({
  pathname: '/coupon',
  couponQueries: [] as Array<Record<string, unknown>>,
  giftcardQueries: [] as Array<Record<string, unknown>>,
  couponTotal: 1,
  plansError: false,
  plansRefetch: vi.fn(),
  refetch: vi.fn(),
  generateCoupon: vi.fn(),
  dropCoupon: vi.fn(),
  showCoupon: vi.fn(),
  generateGiftcard: vi.fn(),
  dropGiftcard: vi.fn(),
  confirm: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock('react-router', () => ({ useLocation: () => ({ pathname: mocks.pathname }) }));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

vi.mock('@/lib/toast', () => ({
  toast: { success: mocks.toastSuccess, error: vi.fn(), loading: vi.fn(), dismiss: vi.fn() },
}));

vi.mock('@/lib/queries', () => ({
  useAdminCoupons: (query: Record<string, unknown>) => {
    mocks.couponQueries.push(query);
    return {
      isPending: false,
      refetch: mocks.refetch,
      // §6.3 (W10): dialect `{items, total}` page shape.
      data: { items: [COUPON], total: mocks.couponTotal },
    };
  },
  useAdminGiftcards: (query: Record<string, unknown>) => {
    mocks.giftcardQueries.push(query);
    return { isPending: false, refetch: mocks.refetch, data: { items: [GIFTCARD], total: 1 } };
  },
  useAdminPlans: () => ({
    data: mocks.plansError ? undefined : [{ id: 1, name: 'VIP' }],
    isError: mocks.plansError,
    refetch: mocks.plansRefetch,
  }),
  useGenerateCouponMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.generateCoupon(payload)).then(options?.onSuccess);
    },
  }),
  useDropCouponMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.dropCoupon(payload)).then(options?.onSuccess);
    },
  }),
  useShowCouponMutation: () => ({ mutate: mocks.showCoupon }),
  useGenerateGiftcardMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.generateGiftcard(payload)).then(options?.onSuccess);
    },
  }),
  useDropGiftcardMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.dropGiftcard(payload)).then(options?.onSuccess);
    },
  }),
}));

beforeEach(() => {
  mocks.pathname = '/coupon';
  mocks.couponQueries = [];
  mocks.giftcardQueries = [];
  mocks.couponTotal = 1;
  mocks.plansError = false;
  mocks.plansRefetch.mockReset().mockResolvedValue(undefined);
  mocks.refetch.mockReset().mockResolvedValue(undefined);
  mocks.generateCoupon.mockReset().mockResolvedValue({ buffer: new ArrayBuffer(8) });
  mocks.dropCoupon.mockReset().mockResolvedValue(true);
  mocks.showCoupon.mockReset();
  mocks.generateGiftcard.mockReset().mockResolvedValue({ buffer: new ArrayBuffer(8) });
  mocks.dropGiftcard.mockReset().mockResolvedValue(true);
  mocks.confirm.mockReset().mockResolvedValue(true);
  mocks.toastSuccess.mockReset();
  // Radix Select / Checkbox pointer + scroll shims for happy-dom.
  window.HTMLElement.prototype.scrollIntoView = vi.fn();
  window.HTMLElement.prototype.hasPointerCapture = vi.fn(() => false);
  window.HTMLElement.prototype.setPointerCapture = vi.fn();
  window.HTMLElement.prototype.releasePointerCapture = vi.fn();
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

function completeCouponForm(
  sheet: HTMLElement,
  values: { name?: string; value?: string; start?: string; end?: string } = {},
) {
  fireEvent.change(within(sheet).getByTestId('coupon-name'), {
    target: { value: values.name ?? '有效优惠券' },
  });
  fireEvent.change(within(sheet).getByTestId('coupon-value'), {
    target: { value: values.value ?? '10' },
  });
  fireEvent.input(within(sheet).getByTestId('coupon-start'), {
    target: { value: values.start ?? '2023-11-14T22:00' },
  });
  fireEvent.input(within(sheet).getByTestId('coupon-end'), {
    target: { value: values.end ?? '2023-11-15T22:00' },
  });
}

function completeGiftcardForm(
  sheet: HTMLElement,
  values: { name?: string; value?: string; start?: string; end?: string } = {},
) {
  fireEvent.change(within(sheet).getByTestId('giftcard-name'), {
    target: { value: values.name ?? '有效礼品卡' },
  });
  fireEvent.change(within(sheet).getByTestId('giftcard-value'), {
    target: { value: values.value ?? '10' },
  });
  fireEvent.input(within(sheet).getByTestId('giftcard-start'), {
    target: { value: values.start ?? '2023-11-14T22:00' },
  });
  fireEvent.input(within(sheet).getByTestId('giftcard-end'), {
    target: { value: values.end ?? '2023-11-15T22:00' },
  });
}

describe('CouponsView', () => {
  it('blocks coupon editors and exposes retry when plans fail', async () => {
    mocks.plansError = true;
    const user = userEvent.setup();
    render(<CouponsPage />);

    expect(screen.getByText('订阅列表加载失败')).toBeInTheDocument();
    expect(screen.getByTestId('coupon-create')).toBeDisabled();
    expect(screen.queryByTestId('coupon-edit-1')).not.toBeInTheDocument();
    await user.click(screen.getByTestId('error-state-retry'));
    expect(mocks.plansRefetch).toHaveBeenCalledOnce();
  });

  it('fetches the first coupon page with the { current, pageSize } shape', () => {
    render(<CouponsPage />);
    expect(mocks.couponQueries[0]).toEqual({ current: 1, pageSize: 10 });
  });

  it('renders coupon rows with type label, code, unlimited uses and the validity range', () => {
    render(<CouponsPage />);
    const table = screen.getByTestId('coupons-table');
    expect(within(table).getByText('十元优惠')).toBeInTheDocument();
    expect(within(table).getByText('SAVE10')).toBeInTheDocument();
    expect(within(table).getByText('金额')).toBeInTheDocument();
    expect(within(table).getByText('无限')).toBeInTheDocument();
    expect(
      within(table).getByText(
        `${dayjs('2023-11-14T22:13:20Z').format('YYYY/MM/DD HH:mm')} ~ ${dayjs(
          '2023-11-15T22:13:20Z',
        ).format('YYYY/MM/DD HH:mm')}`,
      ),
    ).toBeInTheDocument();
  });

  it('copies the coupon code and toasts on click', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);
    await user.click(within(screen.getByTestId('coupons-table')).getByText('SAVE10'));
    expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功');
  });

  it('passes an amount coupon decimal to the api-client and does not download a CSV', async () => {
    const createObjectURL = vi.fn(() => 'blob:x');
    Object.assign(window.URL, { createObjectURL, revokeObjectURL: vi.fn() });
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-create'));
    const sheet = await screen.findByTestId('coupon-editor');
    completeCouponForm(sheet, { name: '新券', value: '10' });
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({ name: '新券', type: 1, value: '10' }),
      ),
    );
    expect(mocks.generateCoupon.mock.calls[0]?.[0]).not.toHaveProperty('generate_count');
    expect(createObjectURL).not.toHaveBeenCalled();
  });

  it('keeps the percent coupon value un-scaled', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-create'));
    const sheet = await screen.findByTestId('coupon-editor');
    await user.click(within(sheet).getByTestId('coupon-type'));
    await user.click(await screen.findByRole('option', { name: '按比例优惠' }));
    completeCouponForm(sheet, { value: '15' });
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({ type: 2, value: '15' }),
      ),
    );
  });

  it('encodes the validity window as unix-second strings', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-create'));
    const sheet = await screen.findByTestId('coupon-editor');
    completeCouponForm(sheet, {
      start: '2023-11-14T22:00',
      end: '2023-11-15T22:00',
    });
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({
          started_at: String(dayjs('2023-11-14T22:00').unix()),
          ended_at: String(dayjs('2023-11-15T22:00').unix()),
        }),
      ),
    );
  });

  it('encodes the plan and period restriction arrays', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-create'));
    const sheet = await screen.findByTestId('coupon-editor');
    completeCouponForm(sheet);
    await user.click(within(within(sheet).getByTestId('coupon-plan-ids')).getByRole('checkbox'));
    await user.click(
      within(within(sheet).getByTestId('coupon-periods')).getByRole('checkbox', { name: '月付' }),
    );
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({ limit_plan_ids: ['1'], limit_period: ['month_price'] }),
      ),
    );
  });

  it('batch-generates coupons with generate_count and downloads the CSV buffer', async () => {
    const createObjectURL = vi.fn(() => 'blob:x');
    const revokeObjectURL = vi.fn();
    Object.assign(window.URL, { createObjectURL, revokeObjectURL });
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-create'));
    const sheet = await screen.findByTestId('coupon-editor');
    completeCouponForm(sheet, { name: '批量券', value: '5' });
    await user.type(within(sheet).getByTestId('coupon-generate-count'), '3');
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({ generate_count: '3', value: '5' }),
      ),
    );
    await waitFor(() => expect(createObjectURL).toHaveBeenCalled());
  });

  it('blocks incomplete and inverted validity payloads before the mutation', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-create'));
    const sheet = await screen.findByTestId('coupon-editor');
    completeCouponForm(sheet, { name: '', start: '2023-11-16T22:00', end: '2023-11-15T22:00' });
    await user.click(within(sheet).getByTestId('coupon-submit'));

    expect(mocks.generateCoupon).not.toHaveBeenCalled();
    expect(await within(sheet).findByText('名称不能为空')).toBeInTheDocument();
    expect(within(sheet).getByText('结束时间必须晚于开始时间')).toBeInTheDocument();
  });

  it('edits a coupon, sending the id and display decimal to the api-client', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-edit-1'));
    const sheet = await screen.findByTestId('coupon-editor');
    expect(within(sheet).getByText('编辑优惠券')).toBeInTheDocument();
    expect(within(sheet).getByTestId('coupon-code')).toHaveValue('SAVE10');
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({ id: 1, type: 1, value: 10, code: 'SAVE10' }),
      ),
    );
  });

  it('drops a coupon by id after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);
    await user.click(screen.getByTestId('coupon-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.dropCoupon).toHaveBeenCalledWith(1));
  });

  it('does not drop a coupon when the confirm dialog is dismissed', async () => {
    mocks.confirm.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<CouponsPage />);
    await user.click(screen.getByTestId('coupon-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    expect(mocks.dropCoupon).not.toHaveBeenCalled();
  });

  it('toggles coupon visibility with the explicit target value', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);
    await user.click(screen.getByLabelText('切换优惠券「十元优惠」启用'));
    expect(mocks.showCoupon).toHaveBeenCalled();
    // §6.3 (W10): PATCH `{show}` carries the target value, not a server flip.
    expect(mocks.showCoupon.mock.calls[0]?.[0]).toEqual({ id: 1, show: false });
  });

  it('sends the new current on pagination change', async () => {
    mocks.couponTotal = 25;
    const user = userEvent.setup();
    render(<CouponsPage />);
    mocks.couponQueries = [];
    await user.click(screen.getByRole('button', { name: '2' }));
    expect(mocks.couponQueries[mocks.couponQueries.length - 1]).toEqual({
      current: 2,
      pageSize: 10,
    });
  });
});

describe('GiftcardsView', () => {
  it('blocks gift-card editors and exposes retry when plans fail', async () => {
    mocks.pathname = '/giftcard';
    mocks.plansError = true;
    const user = userEvent.setup();
    render(<CouponsPage />);

    expect(screen.getByText('订阅列表加载失败')).toBeInTheDocument();
    expect(screen.getByTestId('giftcard-create')).toBeDisabled();
    expect(screen.queryByTestId('giftcard-edit-1')).not.toBeInTheDocument();
    await user.click(screen.getByTestId('error-state-retry'));
    expect(mocks.plansRefetch).toHaveBeenCalledOnce();
  });

  beforeEach(() => {
    mocks.pathname = '/giftcard';
  });

  it('fetches the first giftcard page and renders formatted value + plan name', () => {
    render(<CouponsPage />);
    expect(mocks.giftcardQueries[0]).toEqual({ current: 1, pageSize: 10 });
    const table = screen.getByTestId('giftcards-table');
    expect(within(table).getByText('余额卡')).toBeInTheDocument();
    expect(within(table).getByText('CARD10')).toBeInTheDocument();
    expect(within(table).getByText('10.00 ¥')).toBeInTheDocument();
    expect(within(table).getByText('VIP')).toBeInTheDocument();
  });

  it('passes an amount giftcard decimal to the api-client', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('giftcard-create'));
    const sheet = await screen.findByTestId('giftcard-editor');
    completeGiftcardForm(sheet, { name: '充值卡', value: '20' });
    await user.click(within(sheet).getByTestId('giftcard-submit'));

    await waitFor(() =>
      expect(mocks.generateGiftcard).toHaveBeenCalledWith(
        expect.objectContaining({ name: '充值卡', type: 1, value: '20' }),
      ),
    );
    expect(mocks.generateGiftcard.mock.calls[0]?.[0]).not.toHaveProperty('generate_count');
  });
});
