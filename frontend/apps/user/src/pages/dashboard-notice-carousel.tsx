import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Notice } from '@v2board/types';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Badge } from '@/components/ui/badge';
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  type CarouselApi,
} from '@/components/ui/carousel';
import { formatBackendDate } from '@v2board/config/format';
import { sanitizeBackendHtml } from '@/lib/sanitize-html';
import { cn } from '@/lib/cn';

interface DashboardNoticeCarouselProps {
  notices: Notice[];
}

export function DashboardNoticeCarousel({ notices }: DashboardNoticeCarouselProps) {
  const { t } = useTranslation();
  const [api, setApi] = useState<CarouselApi>();
  const [selectedIndex, setSelectedIndex] = useState(0);
  const initialPopup = notices.find((notice) => notice.tags?.includes('弹窗')) ?? null;
  const [activeNotice, setActiveNotice] = useState<Notice | null>(initialPopup);
  const [noticeOpen, setNoticeOpen] = useState(initialPopup !== null);

  useEffect(() => {
    if (!api) return;
    const onSelect = () => setSelectedIndex(api.selectedScrollSnap());
    api.on('select', onSelect);
    api.on('reInit', onSelect);
    return () => {
      api.off('select', onSelect);
      api.off('reInit', onSelect);
    };
  }, [api]);

  const openNotice = (notice: Notice) => {
    setActiveNotice(notice);
    setNoticeOpen(true);
  };

  if (!notices.length) return null;

  return (
    <section data-testid="dashboard-notices" className="space-y-3">
      <Carousel
        data-testid="dashboard-notice-carousel"
        setApi={setApi}
        aria-label={t(($) => $.notice.title)}
      >
        <CarouselContent>
          {notices.map((notice, index) => (
            <CarouselItem
              key={notice.id}
              data-testid="dashboard-notice-slide"
              data-active={index === selectedIndex ? 'true' : 'false'}
              aria-label={`${t(($) => $.notice.title)} ${index + 1} / ${notices.length}`}
            >
              <NoticeCard notice={notice} active={index === selectedIndex} onOpen={openNotice} />
            </CarouselItem>
          ))}
        </CarouselContent>
        {notices.length > 1 ? (
          <div
            data-testid="dashboard-notice-dots"
            role="group"
            aria-label={t(($) => $.notice.title)}
            className="mt-3 flex h-auto justify-center gap-1"
          >
            {notices.map((notice, index) => (
              <button
                key={notice.id}
                type="button"
                data-testid="dashboard-notice-dot"
                data-active={index === selectedIndex ? 'true' : 'false'}
                aria-label={`${t(($) => $.notice.title)} ${index + 1}`}
                aria-current={index === selectedIndex ? 'true' : undefined}
                onClick={() => api?.scrollTo(index)}
                className={cn(
                  'h-1.5 w-6 rounded-full bg-border transition-colors hover:bg-muted-foreground/40 focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
                  index === selectedIndex && 'bg-primary',
                )}
              />
            ))}
          </div>
        ) : null}
      </Carousel>

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
              // eslint-disable-next-line @eslint-react/dom-no-dangerously-set-innerhtml -- backend HTML sanitized by sanitizeBackendHtml
              dangerouslySetInnerHTML={{ __html: sanitizeBackendHtml(activeNotice.content) }}
            />
          ) : null}
        </DialogContent>
      </Dialog>
    </section>
  );
}

function NoticeCard({
  notice,
  active,
  onOpen,
}: {
  notice: Notice;
  active: boolean;
  onOpen: (notice: Notice) => void;
}) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      data-testid="dashboard-notice-card"
      className="flex w-full flex-col overflow-hidden rounded-xl border border-border bg-card text-left text-card-foreground shadow-sm transition-colors hover:bg-accent/40 focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
      onClick={() => onOpen(notice)}
    >
      <div
        className={cn(
          'relative isolate min-h-36 overflow-hidden p-5 sm:min-h-40',
          !notice.img_url && 'bg-muted/30',
        )}
      >
        {notice.img_url ? (
          <>
            <img
              data-testid="dashboard-notice-image"
              src={notice.img_url}
              alt=""
              loading={active ? 'eager' : 'lazy'}
              decoding="async"
              fetchPriority={active ? 'high' : 'low'}
              className="absolute inset-0 -z-20 size-full object-cover"
            />
            <div aria-hidden="true" className="absolute inset-0 -z-10 bg-black/50" />
          </>
        ) : null}
        <div className="relative">
          <Badge>{t(($) => $.notice.title)}</Badge>
          <div className={cn('mt-10 space-y-1', notice.img_url && 'text-white')}>
            <div className="line-clamp-2 text-lg font-semibold">{notice.title}</div>
            <div className={cn('text-sm text-muted-foreground', notice.img_url && 'text-white/75')}>
              {formatBackendDate(notice.created_at)}
            </div>
          </div>
        </div>
      </div>
    </button>
  );
}
