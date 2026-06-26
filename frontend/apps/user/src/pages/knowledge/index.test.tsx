import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import KnowledgePage from './index';

const knowledgeSource = readFileSync(`${process.cwd()}/src/pages/knowledge/index.tsx`, 'utf8');

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

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

  return {
    legacyCopyText: vi.fn(),
    toastSuccess: vi.fn(),
    detailRefetch: vi.fn(),
    knowledgeArgs: [] as Array<{ language: string; keyword?: string }>,
    detailArgs: [] as Array<{ id: number | string | undefined; language: string }>,
    fetching: false,
    detailFetching: false,
    searchParams: new URLSearchParams(),
    defaultGroups,
    groups: defaultGroups,
    detailById: {
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
    } as Record<string, unknown>,
  };
});

const labels: Record<string, string> = {
  'dashboard.copy_success': '复制成功',
  'knowledge.last_update': '最后更新: {date}',
  'knowledge.search_placeholder': '搜索文档',
  'nav.knowledge': '使用文档',
};

vi.mock('react-router-dom', () => ({
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
      isFetching: mocks.fetching,
    };
  },
  useKnowledgeDetail: (id: number | string | undefined, language: string) => {
    mocks.detailArgs.push({ id, language });
    return {
      data: id === undefined ? undefined : mocks.detailById[String(id)],
      isFetching: mocks.detailFetching,
      refetch: mocks.detailRefetch,
    };
  },
}));

vi.mock('@/lib/markdown', () => ({
  renderLegacyMarkdown: (value: string) => value,
}));

vi.mock('@/lib/legacy-settings', () => ({
  legacyCopyText: mocks.legacyCopyText,
}));

vi.mock('@/lib/legacy-toast', () => ({
  toast: {
    success: mocks.toastSuccess,
  },
}));

describe('KnowledgePage shadcn library surface', () => {
  beforeEach(() => {
    mocks.fetching = false;
    mocks.detailFetching = false;
    mocks.searchParams = new URLSearchParams();
    mocks.groups = mocks.defaultGroups;
    mocks.knowledgeArgs = [];
    mocks.detailArgs = [];
  });

  afterEach(() => {
    document.body.innerHTML = '';
    window.copy = undefined;
    window.jump = undefined;
  });

  it('renders the shadcn search card, category groups, article rows, and dates', () => {
    const html = renderToStaticMarkup(<KnowledgePage />);

    expect(html).toContain('v2board-knowledge-surface');
    expect(html).toContain('v2board-knowledge-card');
    expect(html).toContain('bg-card');
    expect(html).toContain('border-border');
    expect(html).toContain('v2board-knowledge-search-bar');
    expect(html).toContain('placeholder="搜索文档"');
    expect(html).toContain('value=""');
    expect(html).toContain('使用文档');
    expect(html).toContain('v2board-knowledge-category');
    expect(html).toContain('v2board-knowledge-category-title');
    expect(html).toContain('General');
    expect(html).toContain('Router');
    expect(html).toContain('v2board-knowledge-item');
    expect(html).toContain('Copy Article');
    expect(html).toContain('Router Guide');
    expect(html).toContain('最后更新: 2023/11/14');
    expect(html).toContain('最后更新: 2023/11/15');
  });

  it('does not show the list spinner before the mount fetch dispatch equivalent', () => {
    mocks.fetching = true;

    const html = renderToStaticMarkup(<KnowledgePage />);

    expect(html).toContain('v2board-knowledge-search-bar');
    expect(html).not.toContain('v2board-knowledge-loading');
  });

  it('renders an explicit shadcn empty state for an empty knowledge payload', () => {
    mocks.groups = {};

    const html = renderToStaticMarkup(<KnowledgePage />);

    expect(html).toContain('v2board-knowledge-empty');
    expect(html).not.toContain('class="ant-empty ant-empty-normal"');
    expect(html).not.toContain('block block-rounded');
  });

  it('uses Radix sheet composition instead of legacy Ant and Bootstrap foundations', () => {
    expect(knowledgeSource).toContain("from '@/components/ui/sheet'");
    expect(knowledgeSource).toContain('<SheetContent');
    expect(knowledgeSource).toContain('v2board-knowledge-item');
    expect(knowledgeSource).not.toContain('AntBtn');
    expect(knowledgeSource).not.toContain('createPortal');
    expect(knowledgeSource).not.toContain('lockLegacyDrawerBodyScroll');
    expect(knowledgeSource).not.toContain('ant-drawer');
    expect(knowledgeSource).not.toContain('list-group-item');
    expect(knowledgeSource).not.toContain('block block-rounded');
  });

  it('keeps the markdown body fallback using logical OR', () => {
    expect(knowledgeSource).toContain("renderLegacyMarkdown(visibleDetail?.body || '')");
    expect(knowledgeSource).not.toContain("renderLegacyMarkdown(visibleDetail?.body ?? '')");
  });

  it('uses a controlled shadcn input while debouncing onChange', () => {
    const searchInputSource = knowledgeSource.slice(
      knowledgeSource.indexOf('<Input'),
      knowledgeSource.indexOf('/>', knowledgeSource.indexOf('<Input')),
    );

    expect(searchInputSource).toContain('onChange={(event) => setSearchValue(event.target.value)}');
    expect(searchInputSource).toContain('value={searchValue}');
    expect(searchInputSource).not.toContain('defaultValue=""');
  });
});

describe('KnowledgePage redesigned interactions', () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.fetching = false;
    mocks.detailFetching = false;
    mocks.searchParams = new URLSearchParams();
    mocks.groups = mocks.defaultGroups;
    mocks.knowledgeArgs = [];
    mocks.detailArgs = [];
    mocks.legacyCopyText.mockClear();
    mocks.toastSuccess.mockClear();
    mocks.detailRefetch.mockClear();
  });

  afterEach(() => {
    if (root) {
      act(() => root?.unmount());
      root = null;
    }
    container.remove();
    document.body.innerHTML = '';
    window.copy = undefined;
    window.jump = undefined;
    vi.useRealTimers();
  });

  it('debounces searches for 300ms and keeps the request locale', async () => {
    await act(async () => {
      root!.render(<KnowledgePage />);
      await Promise.resolve();
    });

    const input = container.querySelector('input[placeholder="搜索文档"]') as HTMLInputElement;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        input,
        'router',
      );
      input.dispatchEvent(new Event('input', { bubbles: true }));
      vi.advanceTimersByTime(299);
      await Promise.resolve();
    });

    expect(mocks.knowledgeArgs.some((item) => item.keyword === 'router')).toBe(false);

    await act(async () => {
      vi.advanceTimersByTime(1);
      await Promise.resolve();
    });

    expect(mocks.knowledgeArgs).toContainEqual({ language: 'zh-CN', keyword: 'router' });
  });

  it('shows the shadcn loading card only after the mount fetch dispatch equivalent', async () => {
    mocks.fetching = true;

    await act(async () => {
      root!.render(<KnowledgePage />);
      await Promise.resolve();
    });

    expect(container.innerHTML).toContain('v2board-knowledge-search-bar');
    expect(container.innerHTML).toContain('v2board-knowledge-loading');
    expect(container.innerHTML).toContain('Loading...');
    expect(container.innerHTML).not.toContain('v2board-knowledge-category');
  });

  it('opens the article sheet from the URL id', async () => {
    mocks.searchParams = new URLSearchParams('id=2');

    await act(async () => {
      root!.render(<KnowledgePage />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.detailArgs).toContainEqual({ id: 2, language: 'zh-CN' });
    expect(document.body.innerHTML).toContain('v2board-knowledge-sheet');
    expect(document.body.innerHTML).toContain('v2board-knowledge-sheet-title');
    expect(document.body.innerHTML).toContain('Router Guide');
    expect(document.body.innerHTML).toContain('custom-html-style');
    expect(document.body.innerHTML).toContain('<a onclick="jump(1)">jump</a>');
  });

  it('shows sheet loading content while fetching an article and clears hooks on close', async () => {
    mocks.detailFetching = true;

    await act(async () => {
      root!.render(<KnowledgePage />);
      await Promise.resolve();
    });

    const item = container.querySelector('.v2board-knowledge-item') as HTMLElement;
    await act(async () => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(document.body.innerHTML).toContain('v2board-knowledge-sheet');
    expect(document.body.innerHTML).toContain('Loading...');
    expect(window.copy).toBeTypeOf('function');
    expect(window.jump).toBeTypeOf('function');

    const closeButton = document.body.querySelector(
      '.v2board-knowledge-sheet button',
    ) as HTMLButtonElement;
    await act(async () => {
      closeButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(document.body.innerHTML).not.toContain('v2board-knowledge-sheet');
    expect(window.copy).toBeUndefined();
    expect(window.jump).toBeUndefined();
  });

  it('copies once and shows the success message once', async () => {
    await act(async () => {
      root!.render(<KnowledgePage />);
      await Promise.resolve();
    });

    const item = container.querySelector('.v2board-knowledge-item') as HTMLElement;
    await act(async () => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(window.copy).toBeTypeOf('function');
    window.copy?.('token');

    expect(mocks.legacyCopyText).toHaveBeenCalledWith('token');
    expect(mocks.toastSuccess).toHaveBeenCalledTimes(1);
    expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功');
  });

  it('jumps between articles and refetches when jumping to the currently visible one', async () => {
    await act(async () => {
      root!.render(<KnowledgePage />);
      await Promise.resolve();
    });

    const item = container.querySelector('.v2board-knowledge-item') as HTMLElement;
    await act(async () => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      window.jump?.(2);
      await Promise.resolve();
    });

    expect(mocks.detailArgs).toContainEqual({ id: 2, language: 'zh-CN' });
    expect(document.body.innerHTML).toContain('Router Guide');

    await act(async () => {
      window.jump?.(2);
      await Promise.resolve();
    });

    expect(mocks.detailRefetch).toHaveBeenCalledTimes(1);
  });

  it('keeps the previous article title while a jump fetch is loading', async () => {
    await act(async () => {
      root!.render(<KnowledgePage />);
      await Promise.resolve();
    });

    const item = container.querySelector('.v2board-knowledge-item') as HTMLElement;
    await act(async () => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(document.body.innerHTML).toContain('Copy Article');

    mocks.detailFetching = true;
    await act(async () => {
      window.jump?.(2);
      root!.render(<KnowledgePage />);
      await Promise.resolve();
    });

    const sheetHtml = document.body.querySelector('.v2board-knowledge-sheet')?.innerHTML ?? '';
    expect(sheetHtml).toContain('Copy Article');
    expect(sheetHtml).not.toContain('Router Guide');
    expect(sheetHtml).toContain('Loading...');
  });
});
