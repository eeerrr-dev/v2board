import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { KeyboardEvent, MouseEvent } from 'react';
import { useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { getLocaleAntdMessages } from '@v2board/i18n';
import type { Knowledge, KnowledgeSummary } from '@v2board/types';
import { ChevronRight, Search } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { PageShell } from '@/components/ui/page';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { getRequestLocale } from '@/lib/api';
import { formatUserLegacyDateSlash } from '@/lib/legacy-date';
import { copyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/toast';
import {
  LEGACY_MARKDOWN_ACTION_ATTRIBUTE,
  LEGACY_MARKDOWN_VALUE_ATTRIBUTE,
  renderLegacyMarkdown,
} from '@/lib/markdown';
import { useKnowledge, useKnowledgeDetail } from '@/lib/queries';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';

export default function KnowledgePage() {
  const { t, i18n } = useTranslation();
  const [searchValue, setSearchValue] = useState('');
  const [keyword, setKeyword] = useState('');
  const [searchParams] = useSearchParams();
  const language = getRequestLocale();
  const [selectedId, setSelectedId] = useState<number | string | undefined>(undefined);
  const [visibleDetail, setVisibleDetail] = useState<Knowledge | undefined>();
  const knowledgeQuery = useKnowledge(language, keyword || undefined);
  const { data, isFetching } = knowledgeQuery;
  const loading = useLegacyFetchLoading(isFetching, knowledgeQuery.error);
  const knowledgeGroups = data ?? {};
  const categories = Object.entries(knowledgeGroups).filter(
    ([, items]) => (items?.length ?? 0) > 0,
  );
  const articleCount = categories.reduce((total, [, items]) => total + (items?.length ?? 0), 0);
  const detail = useKnowledgeDetail(selectedId, language);
  const refetchDetail = detail.refetch;
  const detailVisible = selectedId !== undefined;
  const urlIdAppliedRef = useRef(false);
  const emptyDescription = getLocaleAntdMessages(i18n.language).emptyDescription;

  useEffect(() => {
    if (urlIdAppliedRef.current) return;
    if (!data) return;
    const raw = searchParams.get('id');
    if (raw == null) {
      urlIdAppliedRef.current = true;
      return;
    }
    const urlId = parseInt(raw);
    if (Number.isNaN(urlId)) {
      urlIdAppliedRef.current = true;
      return;
    }
    const matchedItem = Object.values(data)
      .flatMap((items) => items ?? [])
      .find((item) => parseInt(String(item.id)) === urlId);
    if (matchedItem) {
      urlIdAppliedRef.current = true;
      setSelectedId(matchedItem.id);
    }
  }, [data, searchParams]);

  const renderedBody = useMemo(
    () => renderLegacyMarkdown(visibleDetail?.body || ''),
    [visibleDetail?.body],
  );

  const closeDetail = () => {
    setSelectedId(undefined);
    setVisibleDetail(undefined);
  };

  const openDetail = (item: KnowledgeSummary) => {
    setVisibleDetail(undefined);
    setSelectedId(item.id);
  };

  useEffect(() => {
    const id = window.setTimeout(() => setKeyword(searchValue || ''), 300);
    return () => window.clearTimeout(id);
  }, [searchValue]);

  useEffect(() => {
    if (detail.data && !detail.isFetching) setVisibleDetail(detail.data);
  }, [detail.data, detail.isFetching]);

  const copyMarkdownText = useCallback(
    async (text: string) => {
      if (await copyText(text)) toast.success(t('dashboard.copy_success'));
    },
    [t],
  );

  const jumpToArticle = useCallback(
    (id: number | string) => {
      if (selectedId !== undefined && String(id) === String(selectedId)) void refetchDetail();
      else setSelectedId(id);
    },
    [refetchDetail, selectedId],
  );

  const runMarkdownAction = (element: HTMLElement) => {
    const action = element.getAttribute(LEGACY_MARKDOWN_ACTION_ATTRIBUTE);
    const value = element.getAttribute(LEGACY_MARKDOWN_VALUE_ATTRIBUTE) ?? '';
    if (action === 'copy') void copyMarkdownText(value);
    if (action === 'jump') jumpToArticle(value);
  };

  const handleMarkdownAction = (event: MouseEvent<HTMLDivElement>) => {
    if (!(event.target instanceof Element)) return;
    const element = event.target.closest<HTMLElement>(`[${LEGACY_MARKDOWN_ACTION_ATTRIBUTE}]`);
    if (!element || !event.currentTarget.contains(element)) return;
    event.preventDefault();
    runMarkdownAction(element);
  };

  const handleMarkdownActionKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    if (!(event.target instanceof Element)) return;
    const element = event.target.closest<HTMLElement>(`[${LEGACY_MARKDOWN_ACTION_ATTRIBUTE}]`);
    if (!element || !event.currentTarget.contains(element)) return;
    event.preventDefault();
    runMarkdownAction(element);
  };

  return (
    <PageShell className="gap-4" data-testid="knowledge-surface">
      <Card className="overflow-hidden" data-testid="knowledge-card">
        <CardHeader className="gap-4 sm:flex sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 items-center gap-2">
            <CardTitle className="truncate text-xl">{t('nav.knowledge')}</CardTitle>
            {articleCount > 0 ? (
              <Badge variant="secondary">{articleCount}</Badge>
            ) : null}
          </div>
          <div className="relative w-full sm:max-w-sm" data-testid="knowledge-search-bar">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              aria-label={t('knowledge.search_placeholder')}
              className="pl-9"
              placeholder={t('knowledge.search_placeholder')}
              value={searchValue}
              onChange={(event) => setSearchValue(event.target.value)}
            />
          </div>
        </CardHeader>
      </Card>

      {loading ? (
        <Card data-testid="knowledge-loading">
          <CardContent className="flex items-center justify-center gap-2 py-14 text-sm text-muted-foreground">
            <span role="status" className="inline-flex items-center gap-2">
              <Spinner />
              <span>Loading...</span>
            </span>
          </CardContent>
        </Card>
      ) : categories.length ? (
        <div className="grid gap-4">
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
                      className="flex w-full items-center gap-4 px-6 py-4 text-left transition-colors hover:bg-muted/50 focus-visible:bg-muted/50 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                      data-testid="knowledge-item"
                      onClick={() => openDetail(item)}
                    >
                      <span className="min-w-0 flex-1 space-y-1">
                        <span
                          className="flex overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-foreground"
                          data-testid="knowledge-item-title"
                        >
                          {item.title}
                        </span>
                        <span
                          className="flex text-xs text-muted-foreground"
                          data-testid="knowledge-item-date"
                        >
                          {t('knowledge.last_update', {
                            date: formatUserLegacyDateSlash(item.updated_at),
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
            {emptyDescription}
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
              {visibleDetail?.title || 'Loading...'}
            </SheetTitle>
            <SheetDescription className={visibleDetail?.updated_at ? undefined : 'sr-only'}>
              {visibleDetail?.updated_at
                ? t('knowledge.last_update', {
                    date: formatUserLegacyDateSlash(visibleDetail.updated_at),
                  })
                : 'Loading...'}
            </SheetDescription>
          </SheetHeader>
          <div
            className="min-h-0 flex-1 overflow-y-auto px-6 py-6"
            data-testid="knowledge-sheet-body"
          >
            {detail.isFetching ? (
              <div
                role="status"
                className="flex items-center gap-2 text-sm text-muted-foreground"
              >
                <Spinner />
                <span>Loading...</span>
              </div>
            ) : (
              <div
                className="custom-html-style min-w-0"
                data-testid="knowledge-article"
                onClick={handleMarkdownAction}
                onKeyDown={handleMarkdownActionKeyDown}
                dangerouslySetInnerHTML={{ __html: renderedBody }}
              />
            )}
          </div>
        </SheetContent>
      </Sheet>
    </PageShell>
  );
}
