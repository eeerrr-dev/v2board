import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Notice } from '@v2board/types';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { formatUserLegacyDate } from '@/lib/legacy-date';
import { sanitizeLegacyHtml } from '@/lib/sanitize-html';
import { cn } from '@/lib/cn';

interface DashboardNoticeCarouselProps {
  notices: Notice[];
}

export function DashboardNoticeCarousel({ notices }: DashboardNoticeCarouselProps) {
  const { t } = useTranslation();
  const [activeNoticeIndex, setActiveNoticeIndex] = useState(0);
  const [activeNotice, setActiveNotice] = useState<Notice | null>(null);
  const [noticeOpen, setNoticeOpen] = useState(false);

  useEffect(() => {
    setActiveNoticeIndex(0);
  }, [notices.length]);

  useEffect(() => {
    if (!notices.length) return;
    const popup = notices.find((notice) => notice.tags?.includes('弹窗'));
    if (popup) {
      setActiveNotice(popup);
      setNoticeOpen(true);
    }
  }, [notices]);

  const openNotice = (notice: Notice) => {
    setActiveNotice(notice);
    setNoticeOpen(true);
  };

  const activeNoticeCard = notices[activeNoticeIndex] ?? notices[0];
  if (!notices.length || !activeNoticeCard) return null;

  return (
    <section data-testid="dashboard-notices" className="space-y-3">
      <div data-testid="dashboard-notice-carousel">
        {notices.map((notice, index) => (
          <div
            key={notice.id}
            data-testid="dashboard-notice-slide"
            data-active={index === activeNoticeIndex ? 'true' : 'false'}
            className={cn('mt-0', index !== activeNoticeIndex && 'hidden')}
          >
            {index === activeNoticeIndex ? <NoticeCard notice={notice} onOpen={openNotice} /> : null}
          </div>
        ))}
        {notices.length > 1 ? (
          <div
            data-testid="dashboard-notice-dots"
            role="group"
            aria-label={t('notice.title')}
            className="mt-3 flex h-auto justify-center gap-1"
          >
            {notices.map((notice, index) => (
              <button
                key={notice.id}
                type="button"
                data-testid="dashboard-notice-dot"
                data-active={index === activeNoticeIndex ? 'true' : 'false'}
                aria-label={`${t('notice.title')} ${index + 1}`}
                aria-current={index === activeNoticeIndex ? 'true' : undefined}
                onClick={() => setActiveNoticeIndex(index)}
                className={cn(
                  'h-1.5 w-6 rounded-full bg-border transition-colors hover:bg-muted-foreground/40 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50',
                  index === activeNoticeIndex && 'bg-primary',
                )}
              />
            ))}
          </div>
        ) : null}
      </div>

      <Dialog
        open={noticeOpen}
        onOpenChange={(open) => {
          setNoticeOpen(open);
          if (!open) setActiveNotice(null);
        }}
      >
        <DialogContent data-testid="dashboard-dialog" aria-describedby={undefined}>
          <DialogHeader>
            <DialogTitle>{activeNotice?.title}</DialogTitle>
          </DialogHeader>
          {activeNotice?.content ? (
            <div
              className="custom-html-style max-h-[60vh] overflow-auto text-sm leading-6"
              dangerouslySetInnerHTML={{ __html: sanitizeLegacyHtml(activeNotice.content) }}
            />
          ) : null}
        </DialogContent>
      </Dialog>
    </section>
  );
}

function NoticeCard({ notice, onOpen }: { notice: Notice; onOpen: (notice: Notice) => void }) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      data-testid="dashboard-notice-card"
      className="flex w-full flex-col overflow-hidden rounded-xl border border-border bg-card text-left text-card-foreground shadow-sm transition-colors hover:bg-accent/40 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
      onClick={() => onOpen(notice)}
    >
      <div
        className={cn('min-h-36 p-5 sm:min-h-40', !notice.img_url && 'bg-muted/30')}
        style={
          notice.img_url
            ? {
                backgroundImage: `linear-gradient(rgba(0,0,0,.52), rgba(0,0,0,.52)), url(${notice.img_url})`,
                backgroundPosition: 'center',
                backgroundSize: 'cover',
              }
            : undefined
        }
      >
        <span className="inline-flex rounded-md bg-primary px-2 py-1 text-xs font-medium text-primary-foreground">
          {t('notice.title')}
        </span>
        <div className={cn('mt-10 space-y-1', notice.img_url && 'text-white')}>
          <div className="line-clamp-2 text-lg font-semibold">{notice.title}</div>
          <div className={cn('text-sm text-muted-foreground', notice.img_url && 'text-white/75')}>
            {formatUserLegacyDate(notice.created_at)}
          </div>
        </div>
      </div>
    </button>
  );
}
