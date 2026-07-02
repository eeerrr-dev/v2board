import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Switch } from '@/components/ui/switch';

// Shared leaf primitives for the profile cards. The god-page split moved each
// card's own queries/mutations/form/dialog into its own file; the pieces here
// are the ones genuinely shared across cards: the section icon tile (was an
// identical class string repeated across every card header), the 0/1 preference
// switch, and the destructive confirm dialog that both the telegram-unbind and
// reset-subscribe cards drive. The confirm dialog keeps the
// profile-confirm-dialog / profile-confirm-primary test hooks the interaction
// parity harness selects, so each card renders its own instance (only one is
// ever open at a time) instead of funnelling through one page-level state.

export function SectionIcon({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-md border border-border bg-muted p-2 text-muted-foreground">
      {children}
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

export function ProfileConfirmDialog({
  open,
  title,
  description,
  onCancel,
  onConfirm,
}: {
  open: boolean;
  title: ReactNode;
  description: ReactNode;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const { t } = useTranslation();

  return (
    <AlertDialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
      <AlertDialogContent className="sm:max-w-md" data-testid="profile-confirm-dialog">
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <Button variant="outline" onClick={onCancel}>
            {t('common.cancel')}
          </Button>
          <Button data-testid="profile-confirm-primary" onClick={onConfirm}>
            {t('profile.confirm')}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
