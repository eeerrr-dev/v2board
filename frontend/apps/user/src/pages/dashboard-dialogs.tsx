import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { QRCodeSVG } from 'qrcode.react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { Button } from '@/components/ui/button';
import { DashboardSubscribeMenu } from './dashboard-subscribe-menu';
import {
  useNewPeriodMutation,
  useSaveOrderMutation,
  useSubscribe,
} from '@/lib/queries';
import { toast } from '@/lib/toast';

export type DashboardConfirmAction = 'reset-package' | 'new-period' | null;

interface DashboardConfirmDialogProps {
  action: DashboardConfirmAction;
  onClose: () => void;
}

export function DashboardConfirmDialog({ action, onClose }: DashboardConfirmDialogProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const subscribe = useSubscribe();
  const newPeriod = useNewPeriodMutation();
  const saveOrder = useSaveOrderMutation();
  const sub = subscribe.data;

  const confirmResetPackage = async () => {
    if (!sub) return;
    const tradeNo = await saveOrder.mutateAsync({
      period: 'reset_price',
      plan_id: sub.plan_id as number,
    });
    onClose();
    navigate(`/order/${tradeNo}`);
  };

  const confirmNewPeriod = async () => {
    await newPeriod.mutateAsync();
    await subscribe.refetch();
    toast.success(t('dashboard.new_period_success'));
    onClose();
    navigate('/dashboard');
  };

  const confirmLoading = saveOrder.isPending || newPeriod.isPending;
  const confirmTitle =
    action === 'reset-package'
      ? t('dashboard.reset_package_confirm_title')
      : t('dashboard.new_period_confirm_title');
  const confirmContent =
    action === 'reset-package'
      ? t('dashboard.reset_package_confirm_content')
      : t('dashboard.new_period_confirm_content');

  return (
    <Dialog
      open={action !== null}
      onOpenChange={(open) => {
        if (!open && !confirmLoading) onClose();
      }}
    >
      <DialogContent data-testid="dashboard-dialog" className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{confirmTitle}</DialogTitle>
          <DialogDescription>{confirmContent}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            disabled={confirmLoading}
            onClick={onClose}
          >
            {t('common.cancel')}
          </Button>
          <Button
            type="button"
            data-testid="dashboard-confirm-primary"
            loading={confirmLoading}
            onClick={() => {
              void (action === 'reset-package'
                ? confirmResetPackage()
                : confirmNewPeriod());
            }}
          >
            {t('common.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface DashboardSubscribeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  subscribeUrl: string;
}

export function DashboardSubscribeDialog({
  open,
  onOpenChange,
  subscribeUrl,
}: DashboardSubscribeDialogProps) {
  const { t } = useTranslation();
  const [qrOpen, setQrOpen] = useState(false);

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent data-testid="dashboard-dialog" className="p-0 sm:max-w-sm">
          <DialogHeader className="px-5 pt-5">
            <DialogTitle>{t('dashboard.shortcut_one_click')}</DialogTitle>
          </DialogHeader>
          <DashboardSubscribeMenu subscribeUrl={subscribeUrl} onOpenQr={() => setQrOpen(true)} />
        </DialogContent>
      </Dialog>

      <Dialog open={qrOpen} onOpenChange={setQrOpen}>
        <DialogContent data-testid="dashboard-dialog" className="sm:max-w-xs">
          <DialogHeader>
            <DialogTitle>{t('dashboard.scan_qrcode_subscribe')}</DialogTitle>
            <DialogDescription>{t('dashboard.qrcode_client_tip')}</DialogDescription>
          </DialogHeader>
          <div className="flex justify-center" data-testid="dashboard-subscribe-qrcode-image">
            <QRCodeSVG value={subscribeUrl} />
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
