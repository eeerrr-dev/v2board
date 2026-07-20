import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import dayjs from 'dayjs';
import KnowledgePage from './knowledge';

// The knowledge manager is a redesigned shadcn island (PageHeader + DataTable +
// a right-side Sheet editor) replacing the antd drawer / bundled markdown editor
// / drag-sort ant-table replica. The DOM and source byte-pins are retired; the
// drag handle is swapped for accessible move buttons and the ace editor for a
// monospace Textarea. What stays covered is the Tier-1 contract: the article
// keyed detail query on edit, the canonical validated save payload, the show toggle,
// the sort.mutate id-list reorder, and the delete confirm + drop.

function makeKnowledge() {
  return [
    {
      id: 1,
      category: '帮助',
      title: '入门指南',
      sort: 1,
      show: true,
      updated_at: '2023-11-14T22:13:20Z',
    },
    {
      id: 2,
      category: '教程',
      title: '进阶用法',
      sort: 2,
      show: false,
      updated_at: '2023-11-14T22:13:20Z',
    },
  ];
}

function makeKnowledgeDetail(id: number) {
  const second = id === 2;
  return {
    id,
    category: second ? '教程' : '帮助',
    title: second ? '进阶详情' : '入门指南',
    sort: id,
    show: !second,
    body: second ? '第二篇正文' : '正文内容',
    language: second ? 'en-US' : 'zh-CN',
    created_at: '2023-11-14T22:13:20Z',
    updated_at: '2023-11-14T22:13:20Z',
  };
}

const mocks = vi.hoisted(() => ({
  data: [] as ReturnType<typeof makeKnowledge>,
  refetch: vi.fn(),
  saveMutateAsync: vi.fn(),
  dropMutateAsync: vi.fn(),
  showMutate: vi.fn(),
  sortMutate: vi.fn(),
  confirm: vi.fn(),
  detailData: {} as Record<number, ReturnType<typeof makeKnowledgeDetail> | undefined>,
  detailPendingIds: new Set<number>(),
  detailErrorIds: new Set<number>(),
  detailHook: vi.fn(),
  detailRefetch: vi.fn(),
  categoriesError: false,
  categoriesRefetch: vi.fn(),
}));

vi.mock('@/lib/queries', () => ({
  useAdminKnowledge: () => ({
    isFetching: false,
    isPending: false,
    error: undefined,
    refetch: mocks.refetch,
    data: mocks.data,
  }),
  useAdminKnowledgeCategories: () => ({
    data: mocks.categoriesError ? undefined : ['帮助', '教程'],
    isError: mocks.categoriesError,
    refetch: mocks.categoriesRefetch,
  }),
  useAdminKnowledgeDetail: (id: number | undefined, open: boolean) => {
    mocks.detailHook(id, open);
    const activeId = open && id != null ? id : undefined;
    return {
      data: activeId == null ? undefined : mocks.detailData[activeId],
      isPending: activeId != null && mocks.detailPendingIds.has(activeId),
      isError: activeId != null && mocks.detailErrorIds.has(activeId),
      refetch: () => mocks.detailRefetch(id),
    };
  },
  useSaveKnowledgeMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.saveMutateAsync(payload)).then(
        options?.onSuccess,
        () => undefined,
      );
    },
    isPending: false,
  }),
  useDropKnowledgeMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.dropMutateAsync(payload)).then(options?.onSuccess);
    },
  }),
  useShowKnowledgeMutation: () => ({ mutate: mocks.showMutate }),
  useSortKnowledgeMutation: () => ({ mutate: mocks.sortMutate }),
}));

vi.mock('@v2board/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

vi.mock('@/lib/toast', () => ({
  toast: { success: vi.fn(), error: vi.fn(), loading: vi.fn(), dismiss: vi.fn() },
}));

describe('KnowledgePage', () => {
  beforeEach(() => {
    mocks.data = makeKnowledge();
    mocks.refetch.mockReset().mockResolvedValue(undefined);
    mocks.saveMutateAsync.mockReset().mockResolvedValue(undefined);
    mocks.dropMutateAsync.mockReset().mockResolvedValue(undefined);
    mocks.showMutate.mockReset();
    mocks.sortMutate.mockReset();
    mocks.confirm.mockReset().mockResolvedValue(true);
    mocks.detailData = { 1: makeKnowledgeDetail(1), 2: makeKnowledgeDetail(2) };
    mocks.detailPendingIds = new Set();
    mocks.detailErrorIds = new Set();
    mocks.detailHook.mockReset();
    mocks.detailRefetch.mockReset().mockResolvedValue(undefined);
    mocks.categoriesError = false;
    mocks.categoriesRefetch.mockReset().mockResolvedValue(undefined);
  });

  it('renders the knowledge rows with formatted update time', () => {
    render(<KnowledgePage />);
    expect(screen.getByText('入门指南')).toBeInTheDocument();
    expect(screen.getByText('进阶用法')).toBeInTheDocument();
    expect(screen.getByText('教程')).toBeInTheDocument();
    expect(
      screen.getAllByText(dayjs('2023-11-14T22:13:20Z').format('YYYY/MM/DD HH:mm')).length,
    ).toBeGreaterThan(0);
  });

  it('blocks knowledge editors and retries when categories fail', async () => {
    mocks.categoriesError = true;
    const user = userEvent.setup();
    render(<KnowledgePage />);

    expect(screen.getByText('知识分类加载失败')).toBeInTheDocument();
    expect(screen.getByTestId('knowledge-create')).toBeDisabled();
    expect(screen.queryByTestId('knowledge-edit-1')).not.toBeInTheDocument();
    await user.click(screen.getByTestId('error-state-retry'));
    expect(mocks.categoriesRefetch).toHaveBeenCalledOnce();
  });

  it('toggles show through the query-layer invalidating mutation', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getAllByRole('switch')[0]!);
    // §6.3 (W10): the toggle sends the explicit target value.
    expect(mocks.showMutate).toHaveBeenCalledWith({ id: 1, show: false });
  });

  it('reorders with sort.mutate over the new id order', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    // Move the first row (id 1) down → new order [2, 1].
    await user.click(within(screen.getByTestId('knowledge-table')).getAllByLabelText('下移')[0]!);
    expect(mocks.sortMutate).toHaveBeenCalledWith(
      [2, 1],
      expect.objectContaining({ onSettled: expect.any(Function) }),
    );
  });

  it('enables the keyed article-detail query when opening the edit sheet', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));
    await waitFor(() => expect(mocks.detailHook).toHaveBeenCalledWith(1, true));
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('入门指南'));
    expect(screen.getByLabelText('内容')).toHaveValue('正文内容');
  });

  it('keeps detail loading visible in the sheet and blocks submit', async () => {
    mocks.detailPendingIds.add(1);
    mocks.detailData[1] = undefined;
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));

    expect(await screen.findByRole('status')).toHaveTextContent('加载中');
    expect(screen.getByTestId('knowledge-submit')).toBeDisabled();
    expect(screen.queryByLabelText('标题')).not.toBeInTheDocument();
  });

  it('hydrates a non-default locale after async detail loading and preserves it on save', async () => {
    mocks.detailPendingIds.add(1);
    mocks.detailData[1] = undefined;
    const user = userEvent.setup();
    const view = render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));
    expect(await screen.findByRole('status')).toHaveTextContent('加载中');

    mocks.detailPendingIds.delete(1);
    mocks.detailData[1] = { ...makeKnowledgeDetail(1), language: 'en-US' };
    view.rerender(<KnowledgePage />);

    await waitFor(() => expect(screen.getByLabelText('语言')).toHaveTextContent('English'));
    await user.click(screen.getByTestId('knowledge-submit'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        id: 1,
        title: '入门指南',
        category: '帮助',
        language: 'en-US',
        body: '正文内容',
      }),
    );
  });

  it('shows detail errors in the sheet, blocks submit, and retries the keyed query', async () => {
    mocks.detailErrorIds.add(1);
    mocks.detailData[1] = undefined;
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));

    const error = await screen.findByTestId('knowledge-detail-error');
    expect(error).toHaveTextContent('知识详情加载失败');
    expect(screen.getByTestId('knowledge-submit')).toBeDisabled();
    await user.click(within(error).getByTestId('error-state-retry'));
    expect(mocks.detailRefetch).toHaveBeenCalledWith(1);
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
  });

  it('does not leak a late first detail into a quickly opened second editor', async () => {
    mocks.detailPendingIds.add(1);
    mocks.detailData[1] = undefined;
    const user = userEvent.setup();
    const view = render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));
    expect(await screen.findByRole('status')).toHaveTextContent('加载中');
    await user.click(screen.getByRole('button', { name: '取消' }));
    await user.click(screen.getByTestId('knowledge-edit-2'));

    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('进阶详情'));
    expect(screen.getByLabelText('内容')).toHaveValue('第二篇正文');

    mocks.detailPendingIds.delete(1);
    mocks.detailData[1] = makeKnowledgeDetail(1);
    view.rerender(<KnowledgePage />);

    expect(screen.getByLabelText('标题')).toHaveValue('进阶详情');
    expect(screen.getByLabelText('内容')).toHaveValue('第二篇正文');
    expect(mocks.detailHook).toHaveBeenCalledWith(1, true);
    expect(mocks.detailHook).toHaveBeenCalledWith(2, true);
  });

  it('resets the RHF values when switching records so an unsaved draft never leaks', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('入门指南'));
    await user.clear(screen.getByLabelText('标题'));
    await user.type(screen.getByLabelText('标题'), '未保存草稿');
    await user.click(screen.getByRole('button', { name: '取消' }));

    await user.click(screen.getByTestId('knowledge-edit-2'));
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('进阶详情'));
    expect(screen.getByLabelText('分类')).toHaveValue('教程');
    expect(screen.getByLabelText('内容')).toHaveValue('第二篇正文');
    expect(screen.queryByDisplayValue('未保存草稿')).toBeNull();
  });

  it('blocks invalid create values inline without issuing a save request', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-create'));
    await user.click(screen.getByTestId('knowledge-submit'));

    expect(await screen.findByText('标题不能为空')).toBeInTheDocument();
    expect(screen.getByText('分类不能为空')).toBeInTheDocument();
    expect(screen.getByText('内容不能为空')).toBeInTheDocument();
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    expect(screen.getByTestId('knowledge-editor')).toBeInTheDocument();
  });

  it('rejects a detail locale outside the supported backend locale enum', async () => {
    mocks.detailData[1] = { ...makeKnowledgeDetail(1), language: 'fr-FR' };
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('入门指南'));
    await user.click(screen.getByTestId('knowledge-submit'));

    expect(await screen.findByText('语言不能为空')).toBeInTheDocument();
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    expect(screen.getByTestId('knowledge-editor')).toBeInTheDocument();
  });

  it('saves an edited article with only the canonical backend payload', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('入门指南'));

    await user.clear(screen.getByLabelText('标题'));
    await user.type(screen.getByLabelText('标题'), '新标题');
    await user.click(screen.getByTestId('knowledge-submit'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        id: 1,
        title: '新标题',
        category: '帮助',
        language: 'zh-CN',
        body: '正文内容',
      }),
    );
  });

  it('saves a new article with the entered title, category, and body', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-create'));
    await waitFor(() => expect(screen.getByTestId('knowledge-editor')).toBeInTheDocument());
    // The editor opens, but the detail hook keeps its query disabled without an id.
    expect(mocks.detailHook).toHaveBeenCalledWith(undefined, true);

    await user.type(screen.getByLabelText('标题'), '标题A');
    await user.type(screen.getByLabelText('分类'), '分类A');
    await user.click(screen.getByLabelText('语言'));
    await user.click(await screen.findByRole('option', { name: 'English' }));
    await user.type(screen.getByTestId('knowledge-body'), '正文A');
    await user.click(screen.getByTestId('knowledge-submit'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        title: '标题A',
        category: '分类A',
        language: 'en-US',
        body: '正文A',
      }),
    );
  });

  it('keeps the editor open with its values when the save mutation fails', async () => {
    mocks.saveMutateAsync.mockRejectedValue(new Error('save failed'));
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('入门指南'));
    await user.clear(screen.getByLabelText('标题'));
    await user.type(screen.getByLabelText('标题'), '失败后保留');
    await user.click(screen.getByTestId('knowledge-submit'));

    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1));
    expect(screen.getByTestId('knowledge-editor')).toBeInTheDocument();
    expect(screen.getByLabelText('标题')).toHaveValue('失败后保留');
  });

  it('deletes only after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.dropMutateAsync).toHaveBeenCalledWith(1));
  });

  it('does not delete when the confirm dialog is dismissed', async () => {
    mocks.confirm.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    expect(mocks.dropMutateAsync).not.toHaveBeenCalled();
  });
});
