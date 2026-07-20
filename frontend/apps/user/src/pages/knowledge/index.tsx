import { useDeferredValue, useState } from 'react';
import type { KeyboardEvent, MouseEvent } from 'react';
import { useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useEmptyDescription } from '@/lib/use-empty-description';
import type { Knowledge, KnowledgeSummary } from '@v2board/types';
import { ChevronRight, Search } from 'lucide-react';
import { Badge } from '@v2board/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@v2board/ui/card';
import { ErrorState } from '@v2board/ui/error-state';
import { Input } from '@v2board/ui/input';
import { PageShell } from '@v2board/ui/page';
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@v2board/ui/sheet';
import { LoadingState, SkeletonLines, SkeletonRows } from '@v2board/ui/loading-state';
import { cn } from '@v2board/ui/cn';
import { getRequestLocale } from '@/lib/api';
import { formatBackendDateSlash } from '@v2board/config/format';
import { copyText } from '@v2board/config/clipboard';
import { toast } from '@/lib/toast';
import {
  BACKEND_MARKDOWN_ACTION_ATTRIBUTE,
  BACKEND_MARKDOWN_VALUE_ATTRIBUTE,
  renderBackendMarkdown,
} from '@/lib/markdown';
import { useKnowledge, useKnowledgeDetail } from '@/lib/queries';

export default function KnowledgePage() {
  const { t } = useTranslation();
  const [searchValue, setSearchValue] = useState('');
  const deferredKeyword = useDeferredValue(searchValue);
  const [searchParams] = useSearchParams();
  const language = getRequestLocale();
  // The URL is the initial navigation input, not state to mirror after every
  // render. Capture it once so the detail request can start alongside the list
  // request, and closing or manually selecting an article is never undone by a
  // later list/search update.
  const [selectedId, setSelectedId] = useState<number | string | undefined>(() => {
    const raw = searchParams.get('id');
    if (raw == null) return undefined;
    const urlId = Number.parseInt(raw, 10);
    return Number.isNaN(urlId) ? undefined : urlId;
  });
  // Remembers the article that is on screen while a jump to another article
  // loads, so the previous one stays visible instead of blanking to a skeleton.
  const [previousArticle, setPreviousArticle] = useState<Knowledge | undefined>(undefined);
  const knowledgeQuery = useKnowledge(language, deferredKeyword || undefined);
  const { data, isError, isPending, isFetching, isPlaceholderData } = knowledgeQuery;
  // The full loading card is only for the initial load (no cached list yet).
  // While a debounced search resolves, keepPreviousData keeps the prior list as
  // placeholder data, so keep rendering it dimmed instead of blanking it —
  // matching the opacity-80 refetch pattern invite/tickets already use.
  const showListLoading = isPending;
  const listRefreshing = isFetching && isPlaceholderData;
  const knowledgeGroups = data ?? {};
  const categories = Object.entries(knowledgeGroups).filter(
    ([, items]) => (items?.length ?? 0) > 0,
  );
  const articleCount = categories.reduce((total, [, items]) => total + (items?.length ?? 0), 0);
  const detail = useKnowledgeDetail(selectedId, language);
  const refetchDetail = detail.refetch;
  // Render the detail straight from the query. While jumping to another article
  // the new query has no data yet, so fall back to the last-shown article
  // (captured in jumpToArticle) to keep it visible; a fresh open clears the ref
  // and shows the loading state instead.
  const visibleDetail = detail.data ?? (detail.isFetching ? previousArticle : undefined);
  const detailVisible = selectedId !== undefined;
  const detailTitle =
    visibleDetail?.title ||
    (detail.isError ? t(($) => $.common.error_title) : t(($) => $.common.loading));
  const emptyDescription = useEmptyDescription();

  // React Compiler memoizes this render (one markdown render per visible article).
  const renderedBody = renderBackendMarkdown(visibleDetail?.body || '');

  const closeDetail = () => {
    setPreviousArticle(undefined);
    setSelectedId(undefined);
  };

  const openDetail = (item: KnowledgeSummary) => {
    // A fresh open from the list shows the loading state, not the last article.
    setPreviousArticle(undefined);
    setSelectedId(item.id);
  };

  const copyMarkdownText = async (text: string) => {
    if (await copyText(text)) toast.success(t(($) => $.dashboard.copy_success));
  };

  const jumpToArticle = (id: number | string) => {
    if (selectedId !== undefined && String(id) === String(selectedId)) {
      void refetchDetail();
      return;
    }
    // Keep the current article visible while the jumped-to article loads.
    setPreviousArticle(visibleDetail);
    setSelectedId(id);
  };

  const runMarkdownAction = (element: HTMLElement) => {
    const action = element.getAttribute(BACKEND_MARKDOWN_ACTION_ATTRIBUTE);
    const value = element.getAttribute(BACKEND_MARKDOWN_VALUE_ATTRIBUTE) ?? '';
    if (action === 'copy') void copyMarkdownText(value);
    if (action === 'jump') jumpToArticle(value);
  };

  const handleMarkdownAction = (event: MouseEvent<HTMLDivElement>) => {
    if (!(event.target instanceof Element)) return;
    const element = event.target.closest<HTMLElement>(`[${BACKEND_MARKDOWN_ACTION_ATTRIBUTE}]`);
    if (!element || !event.currentTarget.contains(element)) return;
    event.preventDefault();
    runMarkdownAction(element);
  };

  const handleMarkdownActionKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    if (!(event.target instanceof Element)) return;
    const element = event.target.closest<HTMLElement>(`[${BACKEND_MARKDOWN_ACTION_ATTRIBUTE}]`);
    if (!element || !event.currentTarget.contains(element)) return;
    event.preventDefault();
    runMarkdownAction(element);
  };

  return (
    <PageShell className="gap-4" data-testid="knowledge-surface">
      <Card className="overflow-hidden" data-testid="knowledge-card">
        <CardHeader className="gap-4 sm:flex sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 items-center gap-2">
            <CardTitle className="truncate text-xl">{t(($) => $.nav.knowledge)}</CardTitle>
            {articleCount > 0 ? <Badge variant="secondary">{articleCount}</Badge> : null}
          </div>
          <div className="relative w-full sm:max-w-sm" data-testid="knowledge-search-bar">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              aria-label={t(($) => $.knowledge.search_placeholder)}
              className="pl-9"
              placeholder={t(($) => $.knowledge.search_placeholder)}
              value={searchValue}
              onChange={(event) => setSearchValue(event.target.value)}
            />
          </div>
        </CardHeader>
      </Card>

      {isError ? (
        <Card>
          <CardContent className="py-8">
            <ErrorState
              onRetry={() => void knowledgeQuery.refetch()}
              data-testid="knowledge-list-error"
            />
          </CardContent>
        </Card>
      ) : showListLoading ? (
        <Card data-testid="knowledge-loading">
          <CardContent className="py-6">
            <LoadingState>
              <SkeletonRows rows={4} />
            </LoadingState>
          </CardContent>
        </Card>
      ) : categories.length ? (
        <div
          className={cn('grid gap-4', listRefreshing && 'opacity-80')}
          data-testid="knowledge-list-grid"
        >
          {categories.map(([category, items]) => (
            <Card key={category} className="overflow-hidden py-0" data-testid="knowledge-category">
              <CardHeader className="border-b border-border py-4">
                <div className="flex min-w-0 items-center justify-between gap-3">
                  <CardTitle className="truncate text-base" data-testid="knowledge-category-title">
                    {category}
                  </CardTitle>
                  <Badge variant="secondary">{items?.length ?? 0}</Badge>
                </div>
              </CardHeader>
              <CardContent className="p-0">
                <div className="divide-y divide-border" data-testid="knowledge-list">
                  {items?.map((item) => (
                    <button
                      key={item.id}
                      type="button"
                      className="flex w-full items-center gap-4 px-6 py-4 text-left transition-colors hover:bg-muted/50 focus-visible:bg-muted/50 focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
                      data-testid="knowledge-item"
                      onClick={() => openDetail(item)}
                    >
                      <span className="min-w-0 flex-1 space-y-1">
                        <span
                          className="flex overflow-hidden text-sm font-medium text-ellipsis whitespace-nowrap text-foreground"
                          data-testid="knowledge-item-title"
                        >
                          {item.title}
                        </span>
                        <span
                          className="flex text-xs text-muted-foreground"
                          data-testid="knowledge-item-date"
                        >
                          {t(($) => $.knowledge.last_update, {
                            date: formatBackendDateSlash(item.updated_at),
                          })}
                        </span>
                      </span>
                      <ChevronRight className="size-4 shrink-0 text-muted-foreground" />
                    </button>
                  ))}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      ) : (
        <Card>
          <CardContent
            className="py-14 text-center text-sm text-muted-foreground"
            data-testid="knowledge-empty"
          >
            {deferredKeyword ? t(($) => $.knowledge.no_results) : emptyDescription}
          </CardContent>
        </Card>
      )}

      <Sheet
        open={detailVisible}
        onOpenChange={(open) => {
          if (!open) closeDetail();
        }}
      >
        <SheetContent
          side="right"
          className="w-full p-0 sm:max-w-2xl"
          data-testid="knowledge-sheet"
        >
          <SheetHeader className="border-b border-border px-6 py-5 pr-12">
            <SheetTitle className="leading-6" data-testid="knowledge-sheet-title">
              {detailTitle}
            </SheetTitle>
            <SheetDescription className={visibleDetail?.updated_at ? undefined : 'sr-only'}>
              {visibleDetail?.updated_at
                ? t(($) => $.knowledge.last_update, {
                    date: formatBackendDateSlash(visibleDetail.updated_at),
                  })
                : t(($) => $.common.loading)}
            </SheetDescription>
          </SheetHeader>
          <div
            className="min-h-0 flex-1 overflow-y-auto px-6 py-6"
            data-testid="knowledge-sheet-body"
          >
            {detail.isFetching ? (
              <LoadingState>
                <SkeletonLines lines={5} />
              </LoadingState>
            ) : detail.isError ? (
              // A failed detail fetch must not sit on a blank body under a stuck
              // "Loading..." title; surface the error with a retry instead.
              <ErrorState onRetry={() => void refetchDetail()} data-testid="knowledge-error" />
            ) : (
              <div
                className="custom-html-style min-w-0"
                data-testid="knowledge-article"
                onClick={handleMarkdownAction}
                onKeyDown={handleMarkdownActionKeyDown}
                // eslint-disable-next-line @eslint-react/dom-no-dangerously-set-innerhtml -- rendered and sanitized in renderBackendMarkdown
                dangerouslySetInnerHTML={{ __html: renderedBody }}
              />
            )}
          </div>
        </SheetContent>
      </Sheet>
    </PageShell>
  );
}
