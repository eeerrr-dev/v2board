import { useTranslation } from 'react-i18next';
import type { ComponentPropsWithRef, ReactNode } from 'react';
import { Copy, Link2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { copyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/toast';

export type ProfilePreferenceKey = 'auto_renewal' | 'remind_expire' | 'remind_traffic';
export type ProfileConfirmAction = 'reset-subscribe' | 'unbind-telegram' | null;

interface ProfileFieldProps {
  id: string;
  inputProps?: ComponentPropsWithRef<typeof Input>;
  label: string;
  placeholder: string;
  error?: ReactNode;
}

export function ProfileField({
  id,
  inputProps,
  label,
  placeholder,
  error,
}: ProfileFieldProps) {
  return (
    <FormField id={id} label={label} error={error} className="gap-2.5">
      <Input type="password" placeholder={placeholder} {...inputProps} />
    </FormField>
  );
}

export function PreferenceRow({
  label,
  checked,
  loading,
  onChange,
}: {
  label: string;
  checked?: unknown;
  loading?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-lg border border-border p-4">
      <div className="text-sm font-medium leading-5">{label}</div>
      <ProfileSwitch
        ariaLabel={label}
        checked={checked}
        loading={loading}
        onChange={onChange}
      />
    </div>
  );
}

export function ProfileSwitch({
  ariaLabel,
  checked,
  loading,
  onChange,
}: {
  ariaLabel?: string;
  checked?: unknown;
  loading?: boolean;
  onChange: (checked: boolean) => void;
}) {
  const normalizedChecked = !!checked;
  return (
    <Switch
      checked={normalizedChecked}
      disabled={loading}
      data-loading={loading ? 'true' : undefined}
      data-testid="profile-switch"
      aria-label={ariaLabel}
      aria-busy={!!loading}
      onCheckedChange={(nextChecked) => onChange(nextChecked)}
      onKeyDown={(event) => {
        if (event.key === 'ArrowLeft') onChange(false);
        else if (event.key === 'ArrowRight') onChange(true);
      }}
    />
  );
}

export function ProfileDepositDialog({
  inputProps,
  onClose,
  onConfirm,
  open,
  placeholder,
}: {
  inputProps?: ComponentPropsWithRef<typeof Input>;
  onClose: () => void;
  onConfirm: () => void;
  open: boolean;
  placeholder: string;
}) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => (nextOpen ? undefined : onClose())}>
      <DialogContent
        className="sm:max-w-md"
        data-testid="profile-deposit-dialog"
        showCloseButton={false}
      >
        <DialogHeader>
          <DialogTitle>{t('profile.recharge')}</DialogTitle>
          <DialogDescription>{placeholder}</DialogDescription>
        </DialogHeader>
        <Input
          data-testid="profile-deposit-input"
          autoComplete="one-time-code"
          aria-label={placeholder}
          placeholder={placeholder}
          {...inputProps}
        />
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button data-testid="profile-deposit-confirm" onClick={onConfirm}>
            {t('profile.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function ProfileTelegramBindDialog({
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
      <DialogContent data-testid="profile-telegram-bind-dialog">
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

export function ProfileConfirmDialog({
  action,
  onCancel,
  onConfirm,
}: {
  action: ProfileConfirmAction;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const { t } = useTranslation();
  const isTelegram = action === 'unbind-telegram';
  const title = isTelegram
    ? t('profile.telegram_unbind_confirm')
    : t('profile.reset_subscribe_confirm');
  const description = isTelegram ? t('profile.telegram_unbind_tip') : t('profile.reset_subscribe_tip');

  return (
    <Dialog open={action !== null} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent
        className="sm:max-w-md"
        data-testid="profile-confirm-dialog"
        showCloseButton={false}
      >
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>
            {t('common.cancel')}
          </Button>
          <Button data-testid="profile-confirm-primary" onClick={onConfirm}>
            {t('profile.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
