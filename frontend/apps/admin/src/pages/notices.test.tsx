import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import dayjs from 'dayjs';
import NoticesPage from './notices';

// The notice manager is a redesigned shadcn island (PageHeader + DataTable +
// shadcn Dialog form) replacing the antd-modal / ant-table replica. The DOM and
// source byte-pins are retired. What stays covered is behavior: the
// save→close order, the empty-tags→null payload rule, the show/drop mutations,
// the shared save endpoint for create+edit, and the confirm gate on delete.
// Cache invalidation belongs to the shared mutation hooks, not this page.

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

const NOTICE_TWO = {
  ...NOTICE,
  id: 2,
  title: '第二条公告',
  content: 'second content',
  tags: ['news'],
};

const mocks = vi.hoisted(() => ({
  refetch: vi.fn(),
  saveMutateAsync: vi.fn(),
  dropMutate: vi.fn(),
  showMutate: vi.fn(),
  confirm: vi.fn(),
  data: [] as Array<typeof NOTICE>,
  listError: false,
}));

vi.mock('@/lib/queries', () => ({
  useAdminNotices: () => ({
    isPending: false,
    isError: mocks.listError,
    isSuccess: !mocks.listError,
    refetch: mocks.refetch,
    data: { data: mocks.data, total: mocks.data.length },
  }),
  useSaveNoticeMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.saveMutateAsync(payload)).then(
        options?.onSuccess,
        () => undefined,
      );
    },
  }),
  useDropNoticeMutation: () => ({ mutate: mocks.dropMutate }),
  useShowNoticeMutation: () => ({ mutate: mocks.showMutate }),
}));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

describe('NoticesPage', () => {
  beforeEach(() => {
    mocks.data = [NOTICE];
    mocks.listError = false;
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

  it('does not render an empty state when the notice query failed', () => {
    mocks.data = [];
    mocks.listError = true;
    render(<NoticesPage />);

    expect(screen.getByText('公告列表加载失败')).toBeInTheDocument();
    expect(screen.queryByTestId('notices-empty')).not.toBeInTheDocument();
  });

  it('opens an empty editor titled 新建公告 from the add button', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-create'));
    const dialog = screen.getByTestId('notice-dialog');
    expect(within(dialog).getByText('新建公告')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('标题')).toHaveValue('');
  });

  it('saves a new notice, then closes the dialog', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-create'));
    const dialog = screen.getByTestId('notice-dialog');
    await user.type(within(dialog).getByLabelText('标题'), '  新公告  ');
    await user.type(within(dialog).getByLabelText('公告内容'), '  新正文  ');
    await user.click(screen.getByTestId('notice-submit'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        title: '新公告',
        content: '新正文',
        img_url: null,
        tags: null,
      }),
    );
    await waitFor(() => expect(screen.queryByTestId('notice-dialog')).not.toBeInTheDocument());
  });

  it('blocks an invalid notice inline without issuing a save request', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-create'));
    await user.click(screen.getByTestId('notice-submit'));

    expect(await screen.findByText('标题不能为空')).toBeInTheDocument();
    expect(screen.getByText('内容不能为空')).toBeInTheDocument();
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    expect(screen.getByTestId('notice-dialog')).toBeInTheDocument();
  });

  it('rejects a malformed image URL before the request', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-create'));
    const dialog = screen.getByTestId('notice-dialog');
    await user.type(within(dialog).getByLabelText('标题'), '公告');
    await user.type(within(dialog).getByLabelText('公告内容'), '正文');
    await user.type(within(dialog).getByLabelText('图片URL'), 'not-a-url');
    await user.click(screen.getByTestId('notice-submit'));

    expect(await screen.findByText('图片URL格式不正确')).toBeInTheDocument();
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
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
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        id: 1,
        title: '维护通知',
        content: 'content',
        img_url: null,
        tags: ['system'],
      }),
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

  it('resets the form when switching records so an unsaved draft never leaks', async () => {
    mocks.data = [NOTICE, NOTICE_TWO];
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-edit-1'));
    await user.clear(screen.getByLabelText('标题'));
    await user.type(screen.getByLabelText('标题'), '未保存草稿');
    await user.click(screen.getByRole('button', { name: '取消' }));

    await user.click(screen.getByTestId('notice-edit-2'));
    expect(screen.getByLabelText('标题')).toHaveValue('第二条公告');
    expect(screen.getByLabelText('公告内容')).toHaveValue('second content');
    expect(screen.queryByDisplayValue('未保存草稿')).toBeNull();
  });

  it('keeps the dialog open when the save mutation fails', async () => {
    mocks.saveMutateAsync.mockRejectedValue(new Error('save failed'));
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-edit-1'));
    await user.click(screen.getByTestId('notice-submit'));

    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1));
    expect(screen.getByTestId('notice-dialog')).toBeInTheDocument();
    expect(screen.getByLabelText('标题')).toHaveValue('维护通知');
  });

  it('toggles a notice visibility through the query-layer invalidating mutation', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByRole('switch'));
    expect(mocks.showMutate).toHaveBeenCalledWith(1);
  });

  it('deletes only after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<NoticesPage />);

    await user.click(screen.getByTestId('notice-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.dropMutate).toHaveBeenCalledWith(1));
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
