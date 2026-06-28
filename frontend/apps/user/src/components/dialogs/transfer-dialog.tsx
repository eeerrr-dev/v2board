import { useState } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { AlertCircle } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/shadcn-dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { userKeys, useTransferMutation } from '@/lib/queries';
import { getLegacySettings } from '@/lib/legacy-settings';

interface TransferDialogProps {
  max?: number;
  children?: ReactNode;
}

export function TransferDialog({ max, children }: TransferDialogProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const transfer = useTransferMutation();
  const [yuan, setYuan] = useState<string | undefined>();
  const [open, setOpen] = useState(false);

  const onOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    setYuan(undefined);
  };

  const onSubmit = async () => {
    try {
      await transfer.mutateAsync(yuan);
      onOpenChange(false);
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    } catch {}
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        {children ?? (
          <Button type="button">{t('invite.transfer')}</Button>
        )}
      </DialogTrigger>
      <DialogContent className="sm:max-w-md" data-testid="invite-dialog">
        <DialogHeader>
          <DialogTitle data-testid="invite-dialog-title">
            {t('dashboard.transfer_to_balance')}
          </DialogTitle>
          <DialogDescription>
            {t('invite.current_commission_balance')}: {Number(max) / 100}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <Alert variant="destructive" className="bg-card">
            <AlertCircle className="size-4" />
            <AlertDescription>
              {t('invite.transfer_notice', { title: getLegacySettings().title })}
            </AlertDescription>
          </Alert>
          <div className="space-y-2">
            <Label htmlFor="invite-transfer-current">
              {t('invite.current_commission_balance')}
            </Label>
            <Input id="invite-transfer-current" disabled value={Number(max) / 100} readOnly />
          </div>
          <div className="space-y-2">
            <Label htmlFor="invite-transfer-amount">{t('invite.transfer_amount')}</Label>
            <Input
              id="invite-transfer-amount"
              placeholder={t('invite.transfer_placeholder')}
              value={yuan ?? ''}
              onChange={(event) => setYuan(event.target.value)}
            />
          </div>
        </div>

        <DialogFooter data-testid="invite-dialog-footer">
          <DialogClose asChild>
            <Button type="button" variant="outline">
              {t('common.cancel')}
            </Button>
          </DialogClose>
          <Button type="button" loading={transfer.isPending} onClick={() => void onSubmit()}>
            {t('profile.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
