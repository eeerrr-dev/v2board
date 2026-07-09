import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import PlansPage from './plans';

// The admin subscription-plan manager is a redesigned shadcn island (PageHeader
// + DataTable + Sheet editor) replacing the antd drawer / drag-sort / ant-table
// replica. Legacy DOM and source byte-pins are retired; the drag handle is
// swapped for accessible move buttons. What stays covered is the Tier-1
// contract anchored on the shared backend: the /plan/fetch shape, the
// /plan/save payload with every field passed through verbatim (per-period
// prices stay raw strings — the ×100 cents scaling lives in the api-client — and
// untouched/emptied prices stay `null`), group_id, the /plan/sort id-list
// reorder, the /plan/drop id, and the /plan/update { id, [key]: value } toggles.

function makePlans() {
  return [
    {
      id: 1,
      sort: 1,
      show: 1,
      renew: 0,
      name: '基础套餐',
      count: 3,
      transfer_enable: 100,
      device_limit: null,
      group_id: 2,
      month_price: 12.34,
      quarter_price: null,
      half_year_price: 56.78,
      year_price: 100,
      two_year_price: null,
      three_year_price: null,
      onetime_price: 300,
      reset_price: null,
      content: '<p>features</p>',
      speed_limit: null,
      capacity_limit: null,
      reset_traffic_method: null,
      created_at: 1,
      updated_at: 1,
    },
    {
      id: 2,
      sort: 2,
      show: 0,
      renew: 1,
      name: '高级套餐',
      count: 0,
      transfer_enable: 200,
      device_limit: 3,
      group_id: 2,
      month_price: 20,
      quarter_price: null,
      half_year_price: null,
      year_price: null,
      two_year_price: null,
      three_year_price: null,
      onetime_price: null,
      reset_price: null,
      content: null,
      speed_limit: null,
      capacity_limit: null,
      reset_traffic_method: null,
      created_at: 1,
      updated_at: 1,
    },
  ];
}

const mocks = vi.hoisted(() => ({
  data: [] as ReturnType<typeof makePlans>,
  groups: [{ id: 2, name: '默认权限组' }],
  refetch: vi.fn(),
  saveMutateAsync: vi.fn(),
  dropMutate: vi.fn(),
  updateMutate: vi.fn(),
  sortMutate: vi.fn(),
  confirm: vi.fn(),
}));

vi.mock('@/lib/queries', () => ({
  useAdminPlans: () => ({
    isFetching: false,
    isPending: false,
    error: undefined,
    refetch: mocks.refetch,
    data: mocks.data,
  }),
  useServerGroups: () => ({ data: mocks.groups, isFetching: false, refetch: vi.fn() }),
  useConfig: () => ({ data: { site: { currency_symbol: '¥' } }, refetch: vi.fn() }),
  useSavePlanMutation: () => ({ mutateAsync: mocks.saveMutateAsync, isPending: false }),
  useDropPlanMutation: () => ({ mutate: mocks.dropMutate }),
  useUpdatePlanMutation: () => ({ mutate: mocks.updateMutate }),
  useSortPlansMutation: () => ({ mutate: mocks.sortMutate, isPending: false }),
}));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

describe('PlansPage', () => {
  beforeEach(() => {
    mocks.data = makePlans();
    mocks.groups = [{ id: 2, name: '默认权限组' }];
    mocks.refetch.mockReset().mockResolvedValue(undefined);
    mocks.saveMutateAsync.mockReset().mockResolvedValue(undefined);
    mocks.dropMutate.mockReset();
    mocks.updateMutate.mockReset();
    mocks.sortMutate.mockReset();
    mocks.confirm.mockReset().mockResolvedValue(true);
    // Radix Select pointer + scroll shims for happy-dom.
    window.HTMLElement.prototype.scrollIntoView = vi.fn();
    window.HTMLElement.prototype.hasPointerCapture = vi.fn(() => false);
    window.HTMLElement.prototype.setPointerCapture = vi.fn();
    window.HTMLElement.prototype.releasePointerCapture = vi.fn();
  });

  it('renders plan rows with name, traffic, formatted prices and group', () => {
    render(<PlansPage />);
    const table = screen.getByTestId('plans-table');
    expect(within(table).getByText('基础套餐')).toBeInTheDocument();
    expect(within(table).getByText('100 GB')).toBeInTheDocument();
    // Prices are formatted to two decimals; null prices render '-'.
    expect(within(table).getByText('12.34')).toBeInTheDocument();
    expect(within(table).getByText('56.78')).toBeInTheDocument();
    expect(within(table).getByText('100.00')).toBeInTheDocument();
    expect(within(table).getByText('300.00')).toBeInTheDocument();
    expect(within(table).getAllByText('默认权限组').length).toBeGreaterThan(0);
  });

  it('creates a plan passing prices through verbatim, keeping untouched prices null', async () => {
    const user = userEvent.setup();
    render(<PlansPage />);

    await user.click(screen.getByTestId('plan-create'));
    const sheet = await screen.findByTestId('plan-editor');
    await user.type(within(sheet).getByTestId('plan-name'), '新套餐');
    // Decimal prices go through fireEvent to avoid number-input keystroke quirks.
    fireEvent.change(within(sheet).getByTestId('plan-price-month_price'), {
      target: { value: '12.34' },
    });
    await user.click(within(sheet).getByTestId('plan-submit'));

    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalled());
    const payload = mocks.saveMutateAsync.mock.calls[0]![0];
    // Page forwards the raw yuan string; the ×100 cents scaling is the
    // api-client's job (serializePlanForSave).
    expect(payload).toMatchObject({ name: '新套餐', month_price: '12.34' });
    // Untouched prices stay null so the api-client leaves them null (not NaN).
    expect(payload.quarter_price).toBeNull();
    expect(payload.year_price).toBeNull();
    expect(payload.onetime_price).toBeNull();
    expect(payload.reset_price).toBeNull();
    await waitFor(() => expect(mocks.refetch).toHaveBeenCalled());
  });

  it('clears an emptied price back to null on edit', async () => {
    const user = userEvent.setup();
    render(<PlansPage />);

    await user.click(screen.getByTestId('plan-edit-1'));
    const sheet = await screen.findByTestId('plan-editor');
    expect(within(sheet).getByTestId('plan-name')).toHaveValue('基础套餐');
    fireEvent.change(within(sheet).getByTestId('plan-price-month_price'), {
      target: { value: '' },
    });
    await user.click(within(sheet).getByTestId('plan-submit'));

    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalled());
    const payload = mocks.saveMutateAsync.mock.calls[0]![0];
    expect(payload.id).toBe(1);
    expect(payload.month_price).toBeNull();
  });

  it('sends the selected group_id as a number', async () => {
    const user = userEvent.setup();
    render(<PlansPage />);

    await user.click(screen.getByTestId('plan-create'));
    const sheet = await screen.findByTestId('plan-editor');
    await user.click(within(sheet).getByTestId('plan-group'));
    await user.click(await screen.findByRole('option', { name: '默认权限组' }));
    await user.click(within(sheet).getByTestId('plan-submit'));

    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalled());
    expect(mocks.saveMutateAsync.mock.calls[0]![0].group_id).toBe(2);
  });

  it('reorders with sort.mutate over the new id order, then refetches', async () => {
    const user = userEvent.setup();
    render(<PlansPage />);

    // Move the first row (id 1) down → new order [2, 1].
    await user.click(within(screen.getByTestId('plans-table')).getAllByLabelText('下移')[0]!);
    expect(mocks.sortMutate).toHaveBeenCalledWith(
      [2, 1],
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    );
    mocks.sortMutate.mock.calls[0]![1].onSuccess();
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('toggles the show and renew flags by id with the inverted value', async () => {
    const user = userEvent.setup();
    render(<PlansPage />);

    // show is 1 → toggles to 0.
    await user.click(screen.getByLabelText('切换「基础套餐」销售状态'));
    expect(mocks.updateMutate).toHaveBeenCalledWith(
      { id: 1, key: 'show', value: 0 },
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    );
    mocks.updateMutate.mock.calls[0]![1].onSuccess();
    expect(mocks.refetch).toHaveBeenCalled();

    // renew is 0 → toggles to 1.
    await user.click(screen.getByLabelText('切换「基础套餐」续费'));
    expect(mocks.updateMutate).toHaveBeenCalledWith(
      { id: 1, key: 'renew', value: 1 },
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    );
  });

  it('drops a plan by id only after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<PlansPage />);

    await user.click(screen.getByTestId('plan-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() =>
      expect(mocks.dropMutate).toHaveBeenCalledWith(
        1,
        expect.objectContaining({ onSuccess: expect.any(Function) }),
      ),
    );
    mocks.dropMutate.mock.calls[0]![1].onSuccess();
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('does not drop a plan when the confirm dialog is dismissed', async () => {
    mocks.confirm.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<PlansPage />);

    await user.click(screen.getByTestId('plan-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    expect(mocks.dropMutate).not.toHaveBeenCalled();
  });
});
