import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import dayjs from 'dayjs';
import { admin } from '@v2board/api-client';
import KnowledgePage from './knowledge';

// The knowledge manager is a redesigned shadcn island (PageHeader + DataTable +
// a right-side Sheet editor) replacing the antd drawer / bundled markdown editor
// / drag-sort ant-table replica. The DOM and source byte-pins are retired; the
// drag handle is swapped for accessible move buttons and the ace editor for a
// monospace Textarea. What stays covered is the Tier-1 contract: the article
// detail fetch on edit, the save payload (Partial<Knowledge>), the show toggle,
// the sort.mutate id-list reorder, and the delete confirm + drop.

function makeKnowledge() {
  return [
    { id: 1, category: '帮助', title: '入门指南', sort: 1, show: 1 as const, updated_at: 1700000000 },
    { id: 2, category: '教程', title: '进阶用法', sort: 2, show: 0 as const, updated_at: 1700000000 },
  ];
}

const mocks = vi.hoisted(() => ({
  data: [] as ReturnType<typeof makeKnowledge>,
  refetch: vi.fn(),
  saveMutateAsync: vi.fn(),
  dropMutateAsync: vi.fn(),
  showMutate: vi.fn(),
  sortMutate: vi.fn(),
  confirm: vi.fn(),
  knowledgeDetail: vi.fn(),
}));

vi.mock('@/lib/queries', () => ({
  useAdminKnowledge: () => ({
    isFetching: false,
    isPending: false,
    error: undefined,
    refetch: mocks.refetch,
    data: mocks.data,
  }),
  useAdminKnowledgeCategories: () => ({ data: ['帮助', '教程'] }),
  useSaveKnowledgeMutation: () => ({ mutateAsync: mocks.saveMutateAsync, isPending: false }),
  useDropKnowledgeMutation: () => ({ mutateAsync: mocks.dropMutateAsync }),
  useShowKnowledgeMutation: () => ({ mutate: mocks.showMutate }),
  useSortKnowledgeMutation: () => ({ mutate: mocks.sortMutate }),
}));

vi.mock('@/lib/api', () => ({ apiClient: {} }));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

vi.mock('@/lib/toast', () => ({
  toast: { success: vi.fn(), error: vi.fn(), loading: vi.fn(), dismiss: vi.fn() },
}));

vi.mock('@v2board/api-client', () => ({
  admin: {
    knowledgeDetail: mocks.knowledgeDetail,
    saveKnowledge: vi.fn(),
  },
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
    mocks.knowledgeDetail.mockReset().mockResolvedValue({
      id: 1,
      category: '帮助',
      title: '入门指南',
      sort: 1,
      show: 1,
      body: '正文内容',
      language: 'zh-CN',
      created_at: 1,
      updated_at: 1700000000,
    });
  });

  it('renders the knowledge rows with formatted update time', () => {
    render(<KnowledgePage />);
    expect(screen.getByText('入门指南')).toBeInTheDocument();
    expect(screen.getByText('进阶用法')).toBeInTheDocument();
    expect(screen.getByText('教程')).toBeInTheDocument();
    expect(
      screen.getAllByText(dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm')).length,
    ).toBeGreaterThan(0);
  });

  it('toggles show through show.mutate and refetches on success', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getAllByRole('switch')[0]!);
    expect(mocks.showMutate).toHaveBeenCalledWith(
      1,
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    );
    mocks.showMutate.mock.calls[0]![1].onSuccess();
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('reorders with sort.mutate over the new id order, then refetches', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    // Move the first row (id 1) down → new order [2, 1].
    await user.click(within(screen.getByTestId('knowledge-table')).getAllByLabelText('下移')[0]!);
    expect(mocks.sortMutate).toHaveBeenCalledWith(
      [2, 1],
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    );
    mocks.sortMutate.mock.calls[0]![1].onSuccess();
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('fetches the article detail when opening the edit sheet', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));
    await waitFor(() => expect(admin.knowledgeDetail).toHaveBeenCalledWith({}, 1));
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('入门指南'));
    expect(screen.getByLabelText('内容')).toHaveValue('正文内容');
  });

  it('saves an edited article with the full detail payload, then refetches', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-edit-1'));
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('入门指南'));

    await user.clear(screen.getByLabelText('标题'));
    await user.type(screen.getByLabelText('标题'), '新标题');
    await user.click(screen.getByTestId('knowledge-submit'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith(
        expect.objectContaining({
          id: 1,
          title: '新标题',
          category: '帮助',
          language: 'zh-CN',
          body: '正文内容',
        }),
      ),
    );
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('saves a new article with the entered title, category, and body', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-create'));
    await waitFor(() => expect(screen.getByTestId('knowledge-editor')).toBeInTheDocument());
    // A new article does not fetch any detail.
    expect(admin.knowledgeDetail).not.toHaveBeenCalled();

    await user.type(screen.getByLabelText('标题'), '标题A');
    await user.type(screen.getByLabelText('分类'), '分类A');
    await user.type(screen.getByTestId('knowledge-body'), '正文A');
    await user.click(screen.getByTestId('knowledge-submit'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith(
        expect.objectContaining({ title: '标题A', category: '分类A', body: '正文A' }),
      ),
    );
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('deletes only after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<KnowledgePage />);

    await user.click(screen.getByTestId('knowledge-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.dropMutateAsync).toHaveBeenCalledWith(1));
    expect(mocks.refetch).toHaveBeenCalled();
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
