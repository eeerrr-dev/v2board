import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { CloseIcon, SearchIcon } from '@/components/ant-icon';
import { AntBtn } from '@/components/ant-btn';
import { useKnowledge, useKnowledgeDetail } from '@/lib/queries';
import { renderLegacyMarkdown } from '@/lib/markdown';
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';
import { useTransitionStatus } from '@/lib/use-transition-status';
import { lockLegacyDrawerBodyScroll } from '@/lib/legacy-body-scroll';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { getRequestLocale } from '@/lib/api';
import { formatUserLegacyDateSlash } from '@/lib/legacy-date';
import type { Knowledge } from '@v2board/types';

declare global {
  interface Window {
    copy?: (text: string) => void;
    jump?: (id: number | string) => void;
  }
}

export default function KnowledgePage() {
  const { t } = useTranslation();
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
  const detail = useKnowledgeDetail(selectedId, language);
  const refetchDetail = detail.refetch;
  const detailVisible = selectedId !== undefined;
  const detailDrawerStatus = useTransitionStatus(detailVisible, 300);
  const urlIdAppliedRef = useRef(false);
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

  const drawerRef = useRef<HTMLDivElement | null>(null);

  const closeDetail = () => {
    // The original hide() dispatches knowledge/setState {knowledge:{}}, clearing
    // the panel content during the slide-out: the title falls back to "Loading..."
    // and the body renders empty markdown while the drawer animates closed.
    setSelectedId(undefined);
    setVisibleDetail(undefined);
  };

  useEffect(() => {
    const id = window.setTimeout(() => setKeyword(searchValue || ''), 300);
    return () => window.clearTimeout(id);
  }, [searchValue]);

  useEffect(() => {
    if (selectedId === undefined) return;
    window.copy = (text: string) => {
      // Legacy `window.copy` calls the copy helper, then shows one success message.
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

  useEffect(() => {
    // rc-drawer autofocuses its tabIndex=-1 wrapper on open (domFocus in
    // componentDidMount/Update) so the node-scoped onKeyDown can catch Escape.
    if (detailDrawerStatus === 'entered') drawerRef.current?.focus();
  }, [detailDrawerStatus]);

  useEffect(() => {
    if (selectedId === undefined) return;
    return lockLegacyDrawerBodyScroll();
  }, [selectedId]);

  return (
    <>
      <div key="knowledge-search" className="v2board-knowledge-search-bar">
        <span className="ant-input-search mb-3 ant-input-search-enter-button ant-input-search-large ant-input-group-wrapper ant-input-group-wrapper-lg">
          <span className="ant-input-wrapper ant-input-group">
            <input
              placeholder={t('knowledge.search_placeholder')}
              className="ant-input ant-input-lg"
              type="text"
              defaultValue=""
              onChange={(event) => setSearchValue(event.target.value)}
            />
            <span className="ant-input-group-addon">
              <AntBtn
                type="button"
                className="ant-btn ant-input-search-button ant-btn-primary ant-btn-lg"
              >
                <SearchIcon />
              </AntBtn>
            </span>
          </span>
        </span>
      </div>

      {loading ? (
        <div className="spinner-grow text-primary" role="status">
          <span className="sr-only">Loading...</span>
        </div>
      ) : (
        Object.keys(knowledgeGroups).map((category) => (
          <div key={category} className="row mb-3 mb-md-0">
            <div className="col-md-12">
              {/* Original class string has a trailing space: "block block-rounded " (umi.js). */}
              <div className="block block-rounded ">
                <div className="block-header block-header-default">
                  <h3 className="block-title">{category}</h3>
                </div>
                <div className="list-group">
                  {knowledgeGroups[category]?.map((item) => (
                    <a
                      key={item.id}
                      className="list-group-item list-group-item-action"
                      style={{
                        borderRadius: 'unset',
                        border: 'unset',
                        borderBottom: '1px solid #e2e8f2',
                      }}
                      onClick={() => {
                        setVisibleDetail(undefined);
                        setSelectedId(item.id);
                      }}
                    >
                      <h5 className="font-size-base mb-1">{item.title}</h5>
                      <small>
                        {t('knowledge.last_update', {
                          date: formatUserLegacyDateSlash(item.updated_at),
                        })}
                      </small>
                    </a>
                  ))}
                </div>
              </div>
            </div>
          </div>
        ))
      )}

      {detailDrawerStatus !== 'exited' && createPortal(
        <div
          ref={drawerRef}
          tabIndex={-1}
          className={`ant-drawer ant-drawer-right${
            detailDrawerStatus === 'entered' ? ' ant-drawer-open' : ''
          }`}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.stopPropagation();
              closeDetail();
            }
          }}
        >
          <div className="ant-drawer-mask" onClick={closeDetail} />
          <div className="ant-drawer-content-wrapper" style={{ width: '80%' }}>
            <div className="ant-drawer-content">
              <div className="ant-drawer-wrapper-body">
                <div className="ant-drawer-header">
                  <div className="ant-drawer-title">{visibleDetail?.title || 'Loading...'}</div>
                  <button aria-label="Close" className="ant-drawer-close" onClick={closeDetail}>
                    <CloseIcon />
                  </button>
                </div>
                <div className="ant-drawer-body">
                  {detail.isFetching ? (
                    <LegacyLoadingIcon />
                  ) : (
                    <div
                      className="custom-html-style"
                      dangerouslySetInnerHTML={{ __html: renderedBody }}
                    />
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>,
        document.body,
      )}
    </>
  );
}
