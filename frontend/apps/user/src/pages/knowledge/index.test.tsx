// @vitest-environment jsdom
import { screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import KnowledgePage from './index';

const mocks = vi.hoisted(() => {
  const defaultGroups: Record<
    string,
    Array<{ id: number; title: string; body: string; category: string; updated_at: number }>
  > = {
    General: [
      {
        id: 1,
        title: 'Copy Article',
        body: '',
        category: 'General',
        updated_at: 1_700_000_000,
      },
    ],
    Router: [
      {
        id: 2,
        title: 'Router Guide',
        body: '',
        category: 'Router',
        updated_at: 1_700_086_400,
      },
    ],
  };

  const defaultDetailById = {
    1: {
      id: 1,
      title: 'Copy Article',
      body: '<button onclick="copy(`token`)">copy</button>',
      category: 'General',
      updated_at: 1_700_000_000,
      language: 'zh-CN',
      sort: null,
      show: 1,
      created_at: 1_700_000_000,
    },
    2: {
      id: 2,
      title: 'Router Guide',
      body: '<a onclick="jump(1)">jump</a>',
      category: 'Router',
      updated_at: 1_700_086_400,
      language: 'zh-CN',
      sort: null,
      show: 1,
      created_at: 1_700_086_400,
    },
  } as Record<string, unknown>;

  return {
    copyText: vi.fn(),
    toastSuccess: vi.fn(),
    detailRefetch: vi.fn(),
    knowledgeArgs: [] as Array<{ language: string; keyword?: string }>,
    detailArgs: [] as Array<{ id: number | string | undefined; language: string }>,
    pending: false,
    fetching: false,
    placeholder: false,
    detailFetching: false,
    detailError: false,
    searchParams: new URLSearchParams(),
    defaultGroups,
    defaultDetailById,
    groups: defaultGroups,
    detailById: { ...defaultDetailById },
  };
});

const labels: Record<string, string> = {
  'common.close_dialog': 'Close',
  'common.loading': 'Loading...',
  'common.error_title': 'Something went wrong',
  'common.retry': 'Retry',
  'dashboard.copy_success': '复制成功',
  'knowledge.last_update': '最后更新: {date}',
  'knowledge.no_results': '没有匹配的文档',
  'knowledge.search_placeholder': '搜索文档',
  'nav.knowledge': '使用文档',
};

vi.mock('react-router', () => ({
  useSearchParams: () => [mocks.searchParams],
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, string>) =>
      (labels[key] ?? key)
        .replace('{{date}}', values?.date ?? '')
        .replace('{date}', values?.date ?? ''),
    i18n: { language: 'zh-CN' },
  }),
}));

vi.mock('@/lib/api', () => ({
  getRequestLocale: () => 'zh-CN',
}));

vi.mock('@/lib/queries', () => ({
  useKnowledge: (language: string, keyword?: string) => {
    mocks.knowledgeArgs.push({ language, keyword });
    return {
      data: mocks.groups,
      error: undefined,
      isPending: mocks.pending,
      isFetching: mocks.fetching,
      isPlaceholderData: mocks.placeholder,
    };
  },
  useKnowledgeDetail: (id: number | string | undefined, language: string) => {
    mocks.detailArgs.push({ id, language });
    return {
      // The detail query has no placeholderData, so a pending fetch resolves to
      // no data yet; the page keeps the previous article visible on its own.
      data:
        id === undefined || mocks.detailFetching
          ? undefined
          : mocks.detailById[String(id)],
      isFetching: mocks.detailFetching,
      isError: mocks.detailError,
      refetch: mocks.detailRefetch,
    };
  },
}));

vi.mock('@/lib/legacy-settings', () => ({
  copyText: mocks.copyText,
}));

vi.mock('@/lib/toast', () => ({
  toast: {
    success: mocks.toastSuccess,
  },
}));

beforeEach(() => {
  mocks.pending = false;
  mocks.fetching = false;
  mocks.placeholder = false;
  mocks.detailFetching = false;
  mocks.detailError = false;
  mocks.searchParams = new URLSearchParams();
  mocks.groups = mocks.defaultGroups;
  mocks.detailById = { ...mocks.defaultDetailById };
  mocks.knowledgeArgs = [];
  mocks.detailArgs = [];
  mocks.copyText.mockReset();
  mocks.copyText.mockResolvedValue(true);
  mocks.toastSuccess.mockClear();
  mocks.detailRefetch.mockClear();
});

describe('KnowledgePage shadcn library surface', () => {
  it('renders the search card, category groups, article rows, and dates', () => {
    renderWithProviders(<KnowledgePage />);

    expect(screen.getByTestId('knowledge-surface')).toBeInTheDocument();
    expect(screen.getByText('使用文档')).toBeInTheDocument();

    const search = within(screen.getByTestId('knowledge-search-bar')).getByRole('textbox', {
      name: '搜索文档',
    });
    expect(search).toHaveAttribute('placeholder', '搜索文档');
    expect(search).toHaveValue('');

    expect(
      screen.getAllByTestId('knowledge-category-title').map((title) => title.textContent),
    ).toEqual(['General', 'Router']);
    expect(screen.getAllByTestId('knowledge-item')).toHaveLength(2);
    expect(screen.getByRole('button', { name: /Copy Article/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Router Guide/ })).toBeInTheDocument();
    expect(screen.getByText('最后更新: 2023/11/14')).toBeInTheDocument();
    expect(screen.getByText('最后更新: 2023/11/15')).toBeInTheDocument();
  });

  it('renders the locale empty description for an empty knowledge payload', () => {
    mocks.groups = {};

    renderWithProviders(<KnowledgePage />);

    // Empty knowledge base shows the locale empty description, not the
    // no-search-results copy.
    expect(screen.getByTestId('knowledge-empty')).toHaveTextContent('暂无数据');
    expect(screen.queryByText('没有匹配的文档')).not.toBeInTheDocument();
    expect(screen.queryAllByTestId('knowledge-item')).toHaveLength(0);
  });
});

describe('KnowledgePage redesigned interactions', () => {
  it('defers searches to the query through the controlled input and keeps the request locale', async () => {
    const { user } = renderWithProviders(<KnowledgePage />);

    expect(mocks.knowledgeArgs).toContainEqual({ language: 'zh-CN', keyword: undefined });
    expect(mocks.knowledgeArgs.some((call) => call.keyword === 'router')).toBe(false);

    const input = screen.getByRole('textbox', { name: '搜索文档' });
    await user.type(input, 'router');

    // The controlled input reflects the typed value and the (deferred) keyword
    // reaches the knowledge query with the request locale.
    expect(input).toHaveValue('router');
    await waitFor(() =>
      expect(mocks.knowledgeArgs).toContainEqual({ language: 'zh-CN', keyword: 'router' }),
    );
  });

  it('shows the full spinner card only on the initial load (isPending) and keeps the search bar', () => {
    // Initial load: no cached list yet, so the full spinner card replaces it.
    mocks.pending = true;
    mocks.fetching = true;

    renderWithProviders(<KnowledgePage />);

    expect(screen.getByTestId('knowledge-search-bar')).toBeInTheDocument();
    expect(
      within(screen.getByTestId('knowledge-loading')).getByRole('status'),
    ).toHaveTextContent('Loading...');
    expect(screen.queryByText('General')).not.toBeInTheDocument();
    expect(screen.queryAllByTestId('knowledge-item')).toHaveLength(0);
  });

  it('keeps the previous list rendered (dimmed) while a debounced search refetch resolves', () => {
    // keepPreviousData supplies the prior list during a search refetch, so
    // isPending is false while isFetching/isPlaceholderData are true.
    mocks.pending = false;
    mocks.fetching = true;
    mocks.placeholder = true;

    renderWithProviders(<KnowledgePage />);

    // No blanking spinner card: the previous categories keep rendering, dimmed.
    expect(screen.queryByTestId('knowledge-loading')).not.toBeInTheDocument();
    expect(screen.getAllByTestId('knowledge-item')).toHaveLength(2);
    expect(screen.getByTestId('knowledge-list-grid').className).toContain('opacity-80');
  });

  it('opens the article in a dialog sheet from the URL id with sanitized markdown actions', async () => {
    mocks.searchParams = new URLSearchParams('id=2');

    renderWithProviders(<KnowledgePage />);

    const sheet = await screen.findByTestId('knowledge-sheet');
    // The sheet is real accessible dialog composition, not a hand-rolled drawer.
    expect(screen.getByRole('dialog')).toBe(sheet);
    expect(mocks.detailArgs).toContainEqual({ id: 2, language: 'zh-CN' });
    await waitFor(() =>
      expect(within(sheet).getByTestId('knowledge-sheet-title')).toHaveTextContent('Router Guide'),
    );

    // Legacy inline onclick hooks are sanitized into delegated data attributes.
    const article = within(sheet).getByTestId('knowledge-article');
    const jump = within(article).getByRole('button', { name: 'jump' });
    expect(jump).toHaveAttribute('data-v2board-markdown-action', 'jump');
    expect(jump).toHaveAttribute('data-v2board-markdown-value', '1');
    expect(article.querySelector('[onclick]')).toBeNull();
  });

  it('opens an article by URL id even when it is absent from the current list', async () => {
    // A link to a cross-language / search-excluded article: not in the loaded
    // list, but the detail endpoint fetches it by id, so it must still open.
    mocks.searchParams = new URLSearchParams('id=99');
    mocks.detailById = {
      '99': { id: 99, title: 'Cross-language Doc', body: 'body', updated_at: 1_700_000_000 },
    } as Record<string, unknown>;

    renderWithProviders(<KnowledgePage />);

    const sheet = await screen.findByTestId('knowledge-sheet');
    expect(mocks.detailArgs).toContainEqual({ id: 99, language: 'zh-CN' });
    await within(sheet).findByText('Cross-language Doc');
  });

  it('shows a retryable error state when the article detail fetch fails', async () => {
    mocks.detailById = {} as Record<string, unknown>;
    mocks.detailError = true;

    const { user } = renderWithProviders(<KnowledgePage />);

    await user.click(screen.getByRole('button', { name: /Copy Article/ }));

    // A failed fetch surfaces the error + retry, not a blank article body.
    const sheet = await screen.findByTestId('knowledge-sheet');
    const error = await within(sheet).findByRole('alert');
    expect(within(sheet).getByTestId('knowledge-sheet-title')).toHaveTextContent(
      'Something went wrong',
    );
    expect(within(sheet).queryByTestId('knowledge-article')).not.toBeInTheDocument();

    await user.click(within(error).getByRole('button', { name: 'Retry' }));

    expect(mocks.detailRefetch).toHaveBeenCalled();
  });

  it('distinguishes no-search-matches from an empty knowledge base', async () => {
    const { user } = renderWithProviders(<KnowledgePage />);

    expect(screen.getByRole('button', { name: /Copy Article/ })).toBeInTheDocument();

    // Searching with no matches shows the purpose-built no-results copy...
    mocks.groups = {};
    await user.type(screen.getByRole('textbox', { name: '搜索文档' }), 'zzz');

    await screen.findByText('没有匹配的文档');
    // ...not the empty-knowledge-base description.
    expect(screen.queryByText('暂无数据')).not.toBeInTheDocument();
  });

  it('shows sheet loading content while fetching an article and closes from the sheet', async () => {
    mocks.detailFetching = true;

    const { user } = renderWithProviders(<KnowledgePage />);

    await user.click(screen.getByRole('button', { name: /Copy Article/ }));

    const sheet = await screen.findByTestId('knowledge-sheet');
    expect(within(sheet).getByTestId('knowledge-sheet-title')).toHaveTextContent('Loading...');
    expect(within(sheet).getByRole('status')).toHaveTextContent('Loading...');

    await user.click(within(sheet).getByRole('button', { name: 'Close' }));

    await waitFor(() =>
      expect(screen.queryByTestId('knowledge-sheet')).not.toBeInTheDocument(),
    );
  });

  it('renders an empty article body through the markdown fallback without crashing', async () => {
    mocks.detailById = {
      ...mocks.defaultDetailById,
      1: {
        ...(mocks.defaultDetailById[1] as Record<string, unknown>),
        body: '',
      },
    };

    const { user } = renderWithProviders(<KnowledgePage />);

    await user.click(screen.getByRole('button', { name: /Copy Article/ }));

    const sheet = await screen.findByTestId('knowledge-sheet');
    await waitFor(() =>
      expect(within(sheet).getByTestId('knowledge-sheet-title')).toHaveTextContent('Copy Article'),
    );
    expect(within(sheet).getByTestId('knowledge-article')).toBeEmptyDOMElement();
  });

  it('copies from sanitized markdown actions and shows the success message once', async () => {
    const { user } = renderWithProviders(<KnowledgePage />);

    await user.click(screen.getByRole('button', { name: /Copy Article/ }));

    const sheet = await screen.findByTestId('knowledge-sheet');
    const copyAction = await within(sheet).findByRole('button', { name: 'copy' });
    expect(copyAction).toHaveAttribute('data-v2board-markdown-value', 'token');

    await user.click(copyAction);

    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalledTimes(1));
    expect(mocks.copyText).toHaveBeenCalledWith('token');
    expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功');
  });

  it('runs sanitized markdown jump hooks from keyboard activation', async () => {
    const { user } = renderWithProviders(<KnowledgePage />);

    await user.click(screen.getByRole('button', { name: /Router Guide/ }));

    const sheet = await screen.findByTestId('knowledge-sheet');
    const jump = await within(sheet).findByRole('button', { name: 'jump' });
    expect(jump).toHaveAttribute('data-v2board-markdown-action', 'jump');

    jump.focus();
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(mocks.detailArgs).toContainEqual({ id: '1', language: 'zh-CN' }),
    );
    await waitFor(() =>
      expect(within(sheet).getByTestId('knowledge-sheet-title')).toHaveTextContent('Copy Article'),
    );
  });

  it('jumps between articles and refetches when jumping to the currently visible one', async () => {
    mocks.detailById = {
      ...mocks.defaultDetailById,
      1: {
        ...(mocks.defaultDetailById[1] as Record<string, unknown>),
        body: '<a onclick="jump(2)">jump</a>',
      },
      2: {
        ...(mocks.defaultDetailById[2] as Record<string, unknown>),
        body: '<a onclick="jump(2)">refresh</a>',
      },
    };

    const { user } = renderWithProviders(<KnowledgePage />);

    await user.click(screen.getByRole('button', { name: /Copy Article/ }));

    const sheet = await screen.findByTestId('knowledge-sheet');
    await user.click(await within(sheet).findByRole('button', { name: 'jump' }));

    expect(mocks.detailArgs).toContainEqual({ id: '2', language: 'zh-CN' });
    await waitFor(() =>
      expect(within(sheet).getByTestId('knowledge-sheet-title')).toHaveTextContent('Router Guide'),
    );

    await user.click(within(sheet).getByRole('button', { name: 'refresh' }));

    expect(mocks.detailRefetch).toHaveBeenCalledTimes(1);
  });

  it('keeps the previous article title while a jump fetch is loading', async () => {
    mocks.detailById = {
      ...mocks.defaultDetailById,
      1: {
        ...(mocks.defaultDetailById[1] as Record<string, unknown>),
        body: '<a onclick="jump(2)">jump</a>',
      },
    };

    const { user } = renderWithProviders(<KnowledgePage />);

    await user.click(screen.getByRole('button', { name: /Copy Article/ }));

    const sheet = await screen.findByTestId('knowledge-sheet');
    await waitFor(() =>
      expect(within(sheet).getByTestId('knowledge-sheet-title')).toHaveTextContent('Copy Article'),
    );

    mocks.detailFetching = true;
    await user.click(within(sheet).getByRole('button', { name: 'jump' }));

    expect(within(sheet).getByTestId('knowledge-sheet-title')).toHaveTextContent('Copy Article');
    expect(within(sheet).queryByText('Router Guide')).not.toBeInTheDocument();
    expect(within(sheet).getByRole('status')).toHaveTextContent('Loading...');
  });
});
