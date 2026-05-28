import { useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { useKnowledge, useKnowledgeDetail } from '@/lib/queries';
import { renderLegacyMarkdown } from '@/lib/markdown';
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';
import type { Knowledge } from '@v2board/types';

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
  const language = i18n.resolvedLanguage ?? 'zh-CN';
  const [selectedId, setSelectedId] = useState<number | undefined>(
    () => Number(searchParams.get('id')) || undefined,
  );
  const [visibleDetail, setVisibleDetail] = useState<Knowledge | undefined>();
  const { data, isFetching } = useKnowledge(language, keyword || undefined);
  const knowledgeGroups = data ?? {};
  const detail = useKnowledgeDetail(selectedId, language);
  const renderedBody = useMemo(
    () => renderLegacyMarkdown(visibleDetail?.body ?? ''),
    [visibleDetail?.body],
  );

  const closeDetail = () => {
    setVisibleDetail(undefined);
    setSelectedId(undefined);
  };

  useEffect(() => {
    const id = window.setTimeout(() => setKeyword(searchValue || ''), 300);
    return () => window.clearTimeout(id);
  }, [searchValue]);

  useEffect(() => {
    if (!selectedId) return;
    window.copy = (text: string) => {
      legacyCopyText(text);
      toast.success(t('dashboard.copy_success'));
    };
    window.jump = (id: number | string) => {
      setSelectedId(Number(id));
    };
    return () => {
      window.copy = undefined;
      window.jump = undefined;
    };
  }, [selectedId, t]);

  useEffect(() => {
    if (detail.data) setVisibleDetail(detail.data);
  }, [detail.data]);

  return (
    <>
      <div className="v2board-knowledge-search-bar">
        <form
          className="ant-input-search ant-input-search-enter-button ant-input-search-large ant-input-group-wrapper mb-3"
          onSubmit={(event) => {
            event.preventDefault();
          }}
        >
          <span className="ant-input-wrapper ant-input-group">
            <input
              className="ant-input ant-input-lg"
              placeholder={t('knowledge.search_placeholder')}
              value={searchValue}
              onChange={(event) => setSearchValue(event.target.value)}
            />
            <span className="ant-input-group-addon">
              <button
                type="submit"
                className="ant-btn ant-btn-primary ant-input-search-button ant-btn-lg"
              >
                <i className="anticon anticon-search" />
              </button>
            </span>
          </span>
        </form>
      </div>

      {isFetching ? (
        <div className="spinner-grow text-primary" role="status">
          <span className="sr-only">Loading...</span>
        </div>
      ) : (
        Object.keys(knowledgeGroups).map((category) => (
          <div className="row mb-3 mb-md-0" key={category}>
            <div className="col-md-12">
              <div className="block block-rounded">
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
                      onClick={() => setSelectedId(item.id)}
                    >
                      <h5 className="font-size-base mb-1">{item.title}</h5>
                      <small>
                        {t('knowledge.last_update', {
                          date: formatKnowledgeDate(item.updated_at),
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

      {selectedId && (
        <div className="ant-drawer ant-drawer-right ant-drawer-open">
          <div className="ant-drawer-mask" onClick={closeDetail} />
          <div className="ant-drawer-content-wrapper" style={{ width: '80%' }}>
            <div className="ant-drawer-content">
              <div className="ant-drawer-wrapper-body">
                <div className="ant-drawer-header">
                  <div className="ant-drawer-title">{visibleDetail?.title || 'Loading...'}</div>
                  <button type="button" className="ant-drawer-close" onClick={closeDetail}>
                    <i className="anticon anticon-close" />
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
        </div>
      )}
    </>
  );
}

function formatKnowledgeDate(timestamp: number) {
  const date = new Date(timestamp * 1000);
  const pad = (value: number) => `${value}`.padStart(2, '0');
  return `${date.getFullYear()}/${pad(date.getMonth() + 1)}/${pad(date.getDate())}`;
}
