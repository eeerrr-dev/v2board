import { useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { getLocaleAntdMessages } from '@v2board/i18n';
import type { Knowledge, KnowledgeSummary } from '@v2board/types';
import { ChevronRight, Search } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
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
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';
import { renderLegacyMarkdown } from '@/lib/markdown';
import { useKnowledge, useKnowledgeDetail } from '@/lib/queries';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';

declare global {
  interface Window {
    copy?: (text: string) => void;
    jump?: (id: number | string) => void;
  }
}

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
    if (selectedId === undefined) return;
    window.copy = (text: string) => {
      legacyCopyText(text);
      toast.success(t('dashboard.copy_success'));
    };
    window.jump = (id: number | string) => {
      if (Object.is(id, selectedId)) void refetchDetail();
      else setSelectedId(id);
    };
    return () => {
      window.copy = undefined;
      window.jump = undefined;
    };
  }, [refetchDetail, selectedId, t]);

  useEffect(() => {
    if (detail.data && !detail.isFetching) setVisibleDetail(detail.data);
  }, [detail.data, detail.isFetching]);

  return (
    <div className="v2board-knowledge-surface space-y-4">
      <Card className="v2board-knowledge-card overflow-hidden">
        <CardHeader className="gap-4 sm:flex sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 items-center gap-2">
            <CardTitle className="truncate text-xl">{t('nav.knowledge')}</CardTitle>
            {articleCount > 0 ? (
              <Badge variant="secondary">{articleCount}</Badge>
            ) : null}
          </div>
          <div className="v2board-knowledge-search-bar relative w-full sm:max-w-sm">
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
        <Card className="v2board-knowledge-loading">
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
            <Card key={category} className="v2board-knowledge-category overflow-hidden py-0">
              <CardHeader className="border-b border-border py-4">
                <div className="flex min-w-0 items-center justify-between gap-3">
                  <CardTitle className="v2board-knowledge-category-title truncate text-base">
                    {category}
                  </CardTitle>
                  <Badge variant="secondary">{items?.length ?? 0}</Badge>
                </div>
              </CardHeader>
              <CardContent className="p-0">
                <div className="v2board-knowledge-list divide-y divide-border">
                  {items?.map((item) => (
                    <button
                      key={item.id}
                      type="button"
                      className="v2board-knowledge-item flex w-full items-center gap-4 px-6 py-4 text-left transition-colors hover:bg-muted/50 focus-visible:bg-muted/50 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                      onClick={() => openDetail(item)}
                    >
                      <span className="min-w-0 flex-1 space-y-1">
                        <span className="v2board-knowledge-item-title block truncate text-sm font-medium text-foreground">
                          {item.title}
                        </span>
                        <span className="v2board-knowledge-item-date block text-xs text-muted-foreground">
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
          <CardContent className="v2board-knowledge-empty py-14 text-center text-sm text-muted-foreground">
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
          className="v2board-knowledge-sheet w-full p-0 sm:max-w-2xl"
        >
          <SheetHeader className="border-b border-border px-6 py-5 pr-12">
            <SheetTitle className="v2board-knowledge-sheet-title leading-6">
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
          <div className="v2board-knowledge-sheet-body min-h-0 flex-1 overflow-y-auto px-6 py-6">
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
                className="v2board-knowledge-article custom-html-style min-w-0"
                dangerouslySetInnerHTML={{ __html: renderedBody }}
              />
            )}
          </div>
        </SheetContent>
      </Sheet>
    </div>
  );
}
