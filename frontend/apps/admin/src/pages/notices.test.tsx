import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import dayjs from 'dayjs';
import NoticesPage from './notices';

// The notice manager is a redesigned shadcn island (PageHeader + DataTable +
// shadcn Dialog form) replacing the antd-modal / ant-table replica. The DOM and
// source byte-pins are retired. What stays covered is behavior: the
// save→refetch→close order, the empty-tags→null payload rule, the show/drop
// mutations that refetch on success, the shared save endpoint for create+edit,
// and the confirm gate on delete.

const NOTICE = {
  id: 1,
  title: '维护通知',
  content: 'content',
  img_url: null,
  tags: ['system'],
  show: 1,
  created_at: 1700000000,
  updated_at: 1700000000,
};

const mocks = vi.hoisted(() => ({
  refetch: vi.fn(),
  saveMutateAsync: vi.fn(),
  dropMutate: vi.fn(),
  showMutate: vi.fn(),
  confirm: vi.fn(),
}));

vi.mock('@/lib/queries', () => ({
  useAdminNotices: () => ({
    isPending: false,
    refetch: mocks.refetch,
    data: { data: [NOTICE], total: 1 },
  }),
  useSaveNoticeMutation: () => ({ isPending: false, mutateAsync: mocks.saveMutateAsync }),
  useDropNoticeMutation: () => ({ mutate: mocks.dropMutate }),
  useShowNoticeMutation: () => ({ mutate: mocks.showMutate }),
}));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

describe('NoticesPage', () => {
  beforeEach(() => {
    mocks.refetch.mockReset().mockResolvedValue(undefined);
    mocks.saveMutateAsync.mockReset().mockResolvedValue(undefined);
    mocks.dropMutate.mockReset();
    mocks.showMutate.mockReset();
    mocks.confirm.mockReset().mockResolvedValue(true);
  });

  it('renders the notice row with the formatted created time', () => {
    render(<NoticesPage />);

    expect(screen.getByText('公告管理')).toBeInTheDocument();
    expect(screen.getByText('维护通知')).toBeInTheDocument();
    expect(
      screen.getByText(dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm')),
    ).toBeInTheDocument();
  });

  it('opens an empty editor titled 新建公告 from the add button', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-create'));
    const dialog = screen.getByTestId('notice-dialog');
    expect(within(dialog).getByText('新建公告')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('标题')).toHaveValue('');
  });

  it('saves a new notice, then refetches, then closes the dialog in order', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-create'));
    await user.type(within(screen.getByTestId('notice-dialog')).getByLabelText('标题'), '新公告');
    await user.click(screen.getByTestId('notice-submit'));

    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ title: '新公告' }));
    expect(mocks.refetch).toHaveBeenCalled();
    expect(mocks.saveMutateAsync.mock.invocationCallOrder[0]!).toBeLessThan(
      mocks.refetch.mock.invocationCallOrder[0]!,
    );
    await waitFor(() => expect(screen.queryByTestId('notice-dialog')).not.toBeInTheDocument());
  });

  it('prefills the editor for an existing notice and saves through the shared endpoint', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-edit-1'));
    const dialog = screen.getByTestId('notice-dialog');
    expect(within(dialog).getByText('编辑公告')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('标题')).toHaveValue('维护通知');

    await user.click(screen.getByTestId('notice-submit'));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith(expect.objectContaining({ id: 1 })),
    );
  });

  it('sends tags: null when the last tag is removed', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-edit-1'));
    await user.click(screen.getByLabelText('移除标签 system'));
    await user.click(screen.getByTestId('notice-submit'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith(expect.objectContaining({ tags: null })),
    );
  });

  it('toggles a notice visibility through show.mutate and refetches on success', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByRole('switch'));
    expect(mocks.showMutate).toHaveBeenCalledWith(1, expect.objectContaining({ onSuccess: expect.any(Function) }));
    // The success callback refetches the page list.
    mocks.showMutate.mock.calls[0]![1].onSuccess();
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('deletes only after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() =>
      expect(mocks.dropMutate).toHaveBeenCalledWith(1, expect.objectContaining({ onSuccess: expect.any(Function) })),
    );
  });

  it('does not delete when the confirm dialog is dismissed', async () => {
    mocks.confirm.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    expect(mocks.dropMutate).not.toHaveBeenCalled();
  });
});
