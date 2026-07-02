import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, Link2, MessageCircle, Send } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { Spinner } from '@/components/ui/spinner';
import {
  useCommConfig,
  useSubscribe,
  useTelegramBotInfo,
  useUnbindTelegramMutation,
  useUserInfo,
} from '@/lib/queries';
import { copyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/toast';
import { ProfileConfirmDialog, SectionIcon } from './profile-ui';

export function TelegramBindCard() {
  const { t } = useTranslation();
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const info = useUserInfo({ refetchOnMount: 'always' });
  // The original /profile never dispatches user/getSubscribe on mount; it only
  // reads whatever subscribe data already sits in the store and re-fetches
  // solely after unbinding Telegram. Keep that contract (enabled: false).
  const subscribeQuery = useSubscribe({ enabled: false });
  const unbindTelegram = useUnbindTelegramMutation();
  const [telegramOpen, setTelegramOpen] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const botInfo = useTelegramBotInfo(telegramOpen);

  const data = info.data;

  const onConfirmUnbind = () => {
    setConfirmOpen(false);
    void unbindTelegram
      .mutateAsync()
      .then(() => {
        toast.success(t('profile.reset_success'));
        // The mutation invalidates the user record; the subscribe query is
        // disabled, so it still needs an explicit refetch here.
        void subscribeQuery.refetch();
      })
      .catch(() => {});
  };

  if (!comm?.is_telegram) return null;

  return (
    <>
      {!data?.telegram_id ? (
        <Card data-testid="profile-telegram-bind">
          <CardHeader>
            <div className="flex items-center justify-between gap-4">
              <div className="flex items-center gap-3">
                <SectionIcon>
                  <Send className="size-4" />
                </SectionIcon>
                <CardTitle className="text-lg" data-testid="profile-card-title">
                  {t('profile.telegram_bind')}
                </CardTitle>
              </div>
              <Button
                data-testid="profile-telegram-start"
                size="sm"
                onClick={() => setTelegramOpen(true)}
              >
                {t('profile.start_now')}
              </Button>
            </div>
          </CardHeader>
        </Card>
      ) : (
        <Card data-testid="profile-telegram-unbind">
          <CardHeader>
            <div className="flex items-start justify-between gap-4">
              <div className="space-y-1.5">
                <CardTitle className="text-lg" data-testid="profile-card-title">
                  {t('profile.telegram_bind')}
                </CardTitle>
                <CardDescription data-testid="profile-telegram-id">
                  Telegram ID: {String(data.telegram_id)}
                </CardDescription>
              </div>
              <Button
                data-testid="profile-telegram-unbind-button"
                variant="destructive"
                size="sm"
                onClick={() => setConfirmOpen(true)}
              >
                {t('profile.telegram_unbind')}
              </Button>
            </div>
          </CardHeader>
        </Card>
      )}

      <TelegramBindDialog
        open={telegramOpen}
        botUsername={botInfo.data?.username}
        subscribeUrl={subscribeQuery.data?.subscribe_url}
        onClose={() => setTelegramOpen(false)}
      />
      <ProfileConfirmDialog
        open={confirmOpen}
        title={t('profile.telegram_unbind_confirm')}
        description={t('profile.telegram_unbind_tip')}
        onCancel={() => setConfirmOpen(false)}
        onConfirm={onConfirmUnbind}
      />
    </>
  );
}

export function TelegramDiscussCard() {
  const { t } = useTranslation();
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });

  if (!comm?.telegram_discuss_link) return null;

  return (
    <Card data-testid="profile-telegram-discuss">
      <CardHeader>
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <SectionIcon>
              <MessageCircle className="size-4" />
            </SectionIcon>
            <CardTitle className="text-lg" data-testid="profile-card-title">
              {t('profile.telegram_discuss')}
            </CardTitle>
          </div>
          <Button asChild size="sm">
            <a href={comm.telegram_discuss_link} target="_blank" rel="noreferrer">
              {t('profile.join_now')}
            </a>
          </Button>
        </div>
      </CardHeader>
    </Card>
  );
}

function TelegramBindDialog({
  botUsername,
  onClose,
  open,
  subscribeUrl,
}: {
  botUsername?: string;
  onClose: () => void;
  open: boolean;
  subscribeUrl?: string;
}) {
  const { t } = useTranslation();
  const bindCommand = subscribeUrl ? `/bind ${subscribeUrl}` : '/bind';

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
      <DialogContent data-testid="profile-telegram-bind-dialog" aria-describedby={undefined}>
        <DialogHeader>
          <DialogTitle>{t('profile.telegram_bind')}</DialogTitle>
        </DialogHeader>
        {botUsername ? (
          <div className="space-y-6">
            <div className="space-y-2">
              <div className="flex items-center gap-2 text-sm font-medium">
                <Link2 className="size-4 text-muted-foreground" />
                {t('profile.telegram_step1')}
              </div>
              <div className="text-sm text-muted-foreground">
                {t('profile.telegram_search')}
                <a
                  href={`https://t.me/${botUsername}`}
                  className="ml-1 font-medium text-foreground underline underline-offset-4"
                >
                  @{botUsername}
                </a>
              </div>
            </div>
            <div className="space-y-2">
              <div className="flex items-center gap-2 text-sm font-medium">
                <Copy className="size-4 text-muted-foreground" />
                {t('profile.telegram_step2')}
              </div>
              <div className="text-sm text-muted-foreground">{t('profile.telegram_send')}</div>
              <button
                type="button"
                className="flex w-full cursor-pointer rounded-md border border-border bg-muted px-3 py-2 text-left font-mono text-sm text-foreground"
                data-testid="profile-copy-code"
                onClick={async () => {
                  if (await copyText(bindCommand)) toast.success(t('dashboard.copy_success'));
                }}
              >
                {bindCommand}
              </button>
            </div>
          </div>
        ) : (
          <div className="flex min-h-24 items-center justify-center">
            <Spinner />
          </div>
        )}
        <DialogFooter>
          <Button data-testid="profile-telegram-bind-confirm" onClick={onClose}>
            {t('profile.i_know')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
