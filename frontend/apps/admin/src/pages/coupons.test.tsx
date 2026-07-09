import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import dayjs from 'dayjs';
import CouponsPage from './coupons';

// The admin coupon/giftcard manager is a redesigned shadcn island (PageHeader +
// DataTable + Sheet editor). Legacy ant-table / ant-modal DOM byte-pins are
// retired. What stays covered is the Tier-1 contract: the fetch page shape, the
// generate payload with its cents (×100 for amount) / unix-second date
// encoding, the plan/period restriction arrays, drop/show, and the type-driven
// value semantics.

const COUPON = {
  id: 1,
  code: 'SAVE10',
  name: '十元优惠',
  type: 1,
  value: 10, // api-client already divided cents → yuan for type 1
  show: 1,
  limit_use: null,
  limit_use_with_user: null,
  limit_plan_ids: null,
  limit_period: null,
  started_at: 1700000000,
  ended_at: 1700086400,
  created_at: 1700000000,
  updated_at: 1700000000,
};

const GIFTCARD = {
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
};

const mocks = vi.hoisted(() => ({
  pathname: '/coupon',
  couponQueries: [] as Array<Record<string, unknown>>,
  giftcardQueries: [] as Array<Record<string, unknown>>,
  couponTotal: 1,
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
    return { isPending: false, refetch: mocks.refetch, data: { data: [COUPON], total: mocks.couponTotal } };
  },
  useAdminGiftcards: (query: Record<string, unknown>) => {
    mocks.giftcardQueries.push(query);
    return { isPending: false, refetch: mocks.refetch, data: { data: [GIFTCARD], total: 1 } };
  },
  useAdminPlans: () => ({ data: [{ id: 1, name: 'VIP' }] }),
  useGenerateCouponMutation: () => ({ isPending: false, mutateAsync: mocks.generateCoupon }),
  useDropCouponMutation: () => ({ mutateAsync: mocks.dropCoupon }),
  useShowCouponMutation: () => ({ mutate: mocks.showCoupon }),
  useGenerateGiftcardMutation: () => ({ isPending: false, mutateAsync: mocks.generateGiftcard }),
  useDropGiftcardMutation: () => ({ mutateAsync: mocks.dropGiftcard }),
}));

beforeEach(() => {
  mocks.pathname = '/coupon';
  mocks.couponQueries = [];
  mocks.giftcardQueries = [];
  mocks.couponTotal = 1;
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

describe('CouponsView', () => {
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
        `${dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm')} ~ ${dayjs(
          1700086400 * 1000,
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

  it('creates an amount coupon with the value scaled to cents (×100) and no CSV download', async () => {
    const createObjectURL = vi.fn(() => 'blob:x');
    Object.assign(window.URL, { createObjectURL, revokeObjectURL: vi.fn() });
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-create'));
    const sheet = await screen.findByTestId('coupon-editor');
    await user.type(within(sheet).getByTestId('coupon-name'), '新券');
    await user.type(within(sheet).getByTestId('coupon-value'), '10');
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({ name: '新券', type: 1, value: 1000 }),
      ),
    );
    expect(mocks.generateCoupon.mock.calls[0]?.[0]).not.toHaveProperty('generate_count');
    expect(createObjectURL).not.toHaveBeenCalled();
    await waitFor(() => expect(mocks.refetch).toHaveBeenCalled());
  });

  it('keeps the percent coupon value un-scaled', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-create'));
    const sheet = await screen.findByTestId('coupon-editor');
    await user.click(within(sheet).getByTestId('coupon-type'));
    await user.click(await screen.findByRole('option', { name: '按比例优惠' }));
    await user.type(within(sheet).getByTestId('coupon-value'), '15');
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
    fireEvent.change(within(sheet).getByTestId('coupon-start'), {
      target: { value: '2023-11-14T22:00' },
    });
    fireEvent.change(within(sheet).getByTestId('coupon-end'), {
      target: { value: '2023-11-15T22:00' },
    });
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({
          started_at: dayjs('2023-11-14T22:00').format('X'),
          ended_at: dayjs('2023-11-15T22:00').format('X'),
        }),
      ),
    );
  });

  it('encodes the plan and period restriction arrays', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-create'));
    const sheet = await screen.findByTestId('coupon-editor');
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
    await user.type(within(sheet).getByTestId('coupon-name'), '批量券');
    await user.type(within(sheet).getByTestId('coupon-value'), '5');
    await user.type(within(sheet).getByTestId('coupon-generate-count'), '3');
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({ generate_count: '3', value: 500 }),
      ),
    );
    await waitFor(() => expect(createObjectURL).toHaveBeenCalled());
  });

  it('edits a coupon, sending the id and cents-scaled value back', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('coupon-edit-1'));
    const sheet = await screen.findByTestId('coupon-editor');
    expect(within(sheet).getByText('编辑优惠券')).toBeInTheDocument();
    expect(within(sheet).getByTestId('coupon-code')).toHaveValue('SAVE10');
    await user.click(within(sheet).getByTestId('coupon-submit'));

    await waitFor(() =>
      expect(mocks.generateCoupon).toHaveBeenCalledWith(
        expect.objectContaining({ id: 1, type: 1, value: 1000, code: 'SAVE10' }),
      ),
    );
  });

  it('drops a coupon by id after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);
    await user.click(screen.getByTestId('coupon-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.dropCoupon).toHaveBeenCalledWith(1));
    await waitFor(() => expect(mocks.refetch).toHaveBeenCalled());
  });

  it('does not drop a coupon when the confirm dialog is dismissed', async () => {
    mocks.confirm.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<CouponsPage />);
    await user.click(screen.getByTestId('coupon-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    expect(mocks.dropCoupon).not.toHaveBeenCalled();
  });

  it('toggles coupon visibility by id', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);
    await user.click(screen.getByLabelText('切换优惠券「十元优惠」启用'));
    expect(mocks.showCoupon).toHaveBeenCalled();
    expect(mocks.showCoupon.mock.calls[0]?.[0]).toBe(1);
  });

  it('sends the new current on pagination change', async () => {
    mocks.couponTotal = 25;
    const user = userEvent.setup();
    render(<CouponsPage />);
    mocks.couponQueries = [];
    await user.click(screen.getByRole('button', { name: '2' }));
    expect(mocks.couponQueries[mocks.couponQueries.length - 1]).toEqual({ current: 2, pageSize: 10 });
  });
});

describe('GiftcardsView', () => {
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

  it('creates an amount giftcard with the value scaled to cents (×100)', async () => {
    const user = userEvent.setup();
    render(<CouponsPage />);

    await user.click(screen.getByTestId('giftcard-create'));
    const sheet = await screen.findByTestId('giftcard-editor');
    await user.type(within(sheet).getByTestId('giftcard-name'), '充值卡');
    await user.type(within(sheet).getByTestId('giftcard-value'), '20');
    await user.click(within(sheet).getByTestId('giftcard-submit'));

    await waitFor(() =>
      expect(mocks.generateGiftcard).toHaveBeenCalledWith(
        expect.objectContaining({ name: '充值卡', type: 1, value: 2000 }),
      ),
    );
  });
});
