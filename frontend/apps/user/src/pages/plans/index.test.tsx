import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Plan } from '@v2board/types';
import PlansPage from './index';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  plans: [
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
  ] as Plan[],
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
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => labels[key] ?? key }),
}));

vi.mock('@/lib/queries', () => ({
  usePlans: () => ({ data: mocks.plans, isLoading: false }),
  useCommConfig: () => ({ data: { currency_symbol: '¥' } }),
}));

describe('PlansPage shadcn commerce list markup', () => {
  beforeEach(() => {
    resetPlans();
  });

  it('renders shadcn tabs, plan cards, labels, and price priority', () => {
    const html = renderToStaticMarkup(<PlansPage />);

    expect(html).toContain('选择最适合你的计划');
    expect(html).toContain('选择合适的订阅周期和流量包。');
    expect(html).toContain('data-testid="plan-tabs"');
    expect(html).toContain('data-testid="plan-card"');
    expect(html).toContain('data-testid="plan-card-title"');
    expect(html).toContain('data-testid="plan-stock-badge"');
    expect(html).toContain('¥ 10.00');
    expect(html).toContain('月付');
    expect(html).toContain('¥ 55.00');
    expect(html).toContain('一次性');
    expect(html).toContain('已售罄');
    expect(html).toContain('lucide-check');
    expect(html).not.toContain('block block-link-pop');
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

    const html = renderToStaticMarkup(<PlansPage />);

    expect(html).toContain('Legacy Empty Price');
    expect(html).toContain('¥ NaN');
    expect(html).toContain('立即订阅');
  });

  it('shows a shadcn empty card for an empty plan list', () => {
    mocks.plans = [];

    const html = renderToStaticMarkup(<PlansPage />);

    expect(html).toContain('data-testid="plan-empty"');
    expect(html).toContain('暂无可用订阅');
    expect(html).not.toContain('data-testid="plan-card"');
  });
});

describe('PlansPage shadcn commerce list behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    resetPlans();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.navigate.mockClear();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  it('navigates from purchasable cards and blocks sold-out card navigation', async () => {
    await act(async () => {
      root.render(<PlansPage />);
      await Promise.resolve();
    });

    const cards = Array.from(
      container.querySelectorAll<HTMLButtonElement>('button[data-testid="plan-card"]'),
    );
    expect(cards).toHaveLength(2);
    expect(cards[0]!.disabled).toBe(false);
    expect(cards[1]!.disabled).toBe(true);

    await act(async () => {
      cards[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      cards[1]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.navigate).toHaveBeenCalledTimes(1);
    expect(mocks.navigate).toHaveBeenCalledWith('/plan/1');
  });

  it('filters period and traffic tabs with the commerce contract boolean checks', async () => {
    await act(async () => {
      root.render(<PlansPage />);
      await Promise.resolve();
    });

    const tabs = Array.from(container.querySelectorAll<HTMLButtonElement>('[data-testid="plan-tabs"] [role="radio"]'));
    expect(container.querySelectorAll('[data-testid="plan-card"]')).toHaveLength(2);

    await act(async () => {
      tabs[1]!.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0 }));
      await Promise.resolve();
    });

    expect(container.querySelectorAll('[data-testid="plan-card"]')).toHaveLength(1);
    expect(container.textContent).toContain('Legacy Monthly');
    expect(container.textContent).not.toContain('Legacy Traffic');

    await act(async () => {
      tabs[2]!.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0 }));
      await Promise.resolve();
    });

    expect(container.querySelectorAll('[data-testid="plan-card"]')).toHaveLength(2);
  });
});
