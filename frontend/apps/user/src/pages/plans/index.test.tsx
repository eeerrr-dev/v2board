import type { ComponentProps } from 'react';
import { screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Plan } from '@v2board/types';
import { renderWithProviders } from '@/test/render';
import PlansPage from './index';

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  refetch: vi.fn(),
  plans: [] as Plan[],
  plansError: false,
}));

const labels: Record<string, string> = {
  'plan.pick_title': '选择最适合你的计划',
  'plan.pick_best_for_you': '选择合适的订阅周期和流量包。',
  'plan.filter_all': '全部',
  'plan.filter_period': '按周期',
  'plan.filter_traffic': '按流量',
  'plan.monthly': '月付',
  'plan.onetime': '一次性',
  'plan.almost_sold_out': '即将售罄',
  'plan.sold_out': '已售罄',
  'plan.buy_now': '立即订阅',
  'plan.no_plan': '暂无可用订阅',
};

function resetPlans() {
  mocks.plans = [
    {
      id: 1,
      group_id: 1,
      transfer_enable: 100,
      device_limit: null,
      speed_limit: null,
      reset_traffic_method: null,
      name: 'Legacy Monthly',
      show: 1,
      sort: 0,
      renew: 1,
      content: JSON.stringify([{ feature: 'Feature A', support: true }]),
      month_price: 1000,
      quarter_price: null,
      half_year_price: null,
      year_price: 9000,
      two_year_price: null,
      three_year_price: null,
      onetime_price: 50000,
      reset_price: null,
      capacity_limit: 4,
      created_at: 0,
      updated_at: 0,
    },
    {
      id: 2,
      group_id: 1,
      transfer_enable: 100,
      device_limit: null,
      speed_limit: null,
      reset_traffic_method: null,
      name: 'Legacy Traffic',
      show: 1,
      sort: 1,
      renew: 1,
      content: '<p>Raw HTML</p>',
      month_price: null,
      quarter_price: null,
      half_year_price: null,
      year_price: null,
      two_year_price: null,
      three_year_price: null,
      onetime_price: 5500,
      reset_price: null,
      capacity_limit: 0,
      created_at: 0,
      updated_at: 0,
    },
  ];
}

vi.mock('react-router', () => ({
  // Purchasable plan cards render as real <a> links; the mock mirrors Link's
  // contract (user onClick first; navigation skipped when it preventDefaults).
  Link: ({
    to,
    onClick,
    children,
    ...rest
  }: { to: string } & Omit<ComponentProps<'a'>, 'href'>) => (
    <a
      href={to}
      onClick={(event) => {
        onClick?.(event);
        if (!event.defaultPrevented) {
          event.preventDefault();
          mocks.navigate(to);
        }
      }}
      {...rest}
    >
      {children}
    </a>
  ),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => labels[key] ?? key }),
}));

vi.mock('@/lib/queries', () => ({
  usePlans: () => ({
    data: mocks.plans,
    isLoading: false,
    isError: mocks.plansError,
    refetch: mocks.refetch,
  }),
  useCommConfig: () => ({ data: { currency_symbol: '¥' } }),
}));

describe('PlansPage shadcn commerce list rendering', () => {
  beforeEach(() => {
    resetPlans();
    mocks.navigate.mockClear();
    mocks.refetch.mockClear();
    mocks.plansError = false;
  });

  it('renders tabs, plan cards, stock labels, and price priority', () => {
    renderWithProviders(<PlansPage />);

    expect(screen.getByText('选择最适合你的计划')).toBeInTheDocument();
    expect(screen.getByText('选择合适的订阅周期和流量包。')).toBeInTheDocument();
    expect(screen.getByTestId('plan-tabs')).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: '全部' })).toBeInTheDocument();

    const [monthly, traffic] = screen.getAllByTestId('plan-card');
    expect(within(monthly!).getByTestId('plan-card-title')).toHaveTextContent('Legacy Monthly');
    // Price priority: the earliest configured period wins (month over year/onetime).
    expect(monthly).toHaveTextContent('¥ 10.00');
    expect(monthly).toHaveTextContent('月付');
    // capacity_limit 4 => almost sold out, still purchasable.
    expect(within(monthly!).getByTestId('plan-stock-badge')).toHaveTextContent('即将售罄');
    expect(monthly).toHaveTextContent('Feature A');
    expect(monthly).toHaveTextContent('立即订阅');
    // Purchasable card is a real link carrying the hash-route href.
    expect(monthly).toHaveAttribute('href', '/plan/1');

    expect(within(traffic!).getByTestId('plan-card-title')).toHaveTextContent('Legacy Traffic');
    expect(traffic).toHaveTextContent('¥ 55.00');
    expect(traffic).toHaveTextContent('一次性');
    expect(traffic).toHaveTextContent('Raw HTML');
    // capacity_limit 0 => sold out: a non-interactive card, not a link.
    expect(traffic).toHaveTextContent('已售罄');
    expect(traffic).not.toHaveAttribute('href');
    expect(traffic).toHaveAttribute('aria-disabled', 'true');
  });

  it('keeps the original all-null-price card instead of hiding it', () => {
    mocks.plans = [
      {
        id: 3,
        group_id: 1,
        transfer_enable: 100,
        device_limit: null,
        speed_limit: null,
        reset_traffic_method: null,
        name: 'Legacy Empty Price',
        show: 1,
        sort: 2,
        renew: 1,
        content: '',
        month_price: null,
        quarter_price: null,
        half_year_price: null,
        year_price: null,
        two_year_price: null,
        three_year_price: null,
        onetime_price: null,
        reset_price: null,
        capacity_limit: null,
        created_at: 0,
        updated_at: 0,
      },
    ];

    renderWithProviders(<PlansPage />);

    const card = screen.getByTestId('plan-card');
    expect(card).toHaveTextContent('Legacy Empty Price');
    expect(card).toHaveTextContent('¥ NaN');
    expect(card).toHaveTextContent('立即订阅');
  });

  it('shows the empty state for an empty plan list', () => {
    mocks.plans = [];

    renderWithProviders(<PlansPage />);

    expect(screen.getByTestId('plan-empty')).toHaveTextContent('暂无可用订阅');
    expect(screen.queryByTestId('plan-card')).not.toBeInTheDocument();
  });

  it('surfaces a retryable error state instead of a permanent spinner when the fetch fails', async () => {
    // A failed plan fetch must not spin forever: show the shared ErrorState
    // (with a retry) instead of the loading/empty card.
    mocks.plans = [];
    mocks.plansError = true;

    const { user } = renderWithProviders(<PlansPage />);

    expect(screen.getByTestId('plan-error')).toBeInTheDocument();
    expect(screen.queryByTestId('plan-empty')).not.toBeInTheDocument();
    expect(screen.queryByTestId('plan-card')).not.toBeInTheDocument();

    await user.click(screen.getByTestId('error-state-retry'));
    expect(mocks.refetch).toHaveBeenCalledTimes(1);
  });
});

describe('PlansPage shadcn commerce list behavior', () => {
  beforeEach(() => {
    resetPlans();
    mocks.navigate.mockClear();
    mocks.refetch.mockClear();
    mocks.plansError = false;
  });

  it('navigates from purchasable cards and blocks sold-out card navigation', async () => {
    const { user } = renderWithProviders(<PlansPage />);

    const cards = screen.getAllByTestId('plan-card');
    expect(cards).toHaveLength(2);
    expect(cards[0]).toHaveAttribute('href', '/plan/1');
    expect(cards[1]).not.toHaveAttribute('href');
    expect(cards[1]).toHaveAttribute('aria-disabled', 'true');

    await user.click(cards[0]!);
    expect(mocks.navigate).toHaveBeenCalledTimes(1);
    expect(mocks.navigate).toHaveBeenCalledWith('/plan/1');

    // The sold-out card is a non-interactive element, so clicking it never
    // navigates.
    await user.click(cards[1]!);
    expect(mocks.navigate).toHaveBeenCalledTimes(1);
  });

  it('filters period and traffic tabs with the commerce contract boolean checks', async () => {
    const { user } = renderWithProviders(<PlansPage />);

    expect(screen.getAllByTestId('plan-card')).toHaveLength(2);

    await user.click(screen.getByRole('radio', { name: '按周期' }));
    expect(screen.getAllByTestId('plan-card')).toHaveLength(1);
    expect(screen.getByTestId('plan-card')).toHaveTextContent('Legacy Monthly');
    expect(screen.queryByText('Legacy Traffic')).not.toBeInTheDocument();

    await user.click(screen.getByRole('radio', { name: '按流量' }));
    expect(screen.getAllByTestId('plan-card')).toHaveLength(2);
  });
});
