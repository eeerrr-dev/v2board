import { forwardRef, useImperativeHandle, useState } from 'react';
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

type ConfirmAction = 'reset-package' | 'new-period' | null;

export interface DashboardConfirmDialogHandle {
  openReset: () => void;
  openNewPeriod: () => void;
}

export const DashboardConfirmDialog = forwardRef<DashboardConfirmDialogHandle>(
  function DashboardConfirmDialog(_props, ref) {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const subscribe = useSubscribe();
    const newPeriod = useNewPeriodMutation();
    const saveOrder = useSaveOrderMutation();
    const [confirmAction, setConfirmAction] = useState<ConfirmAction>(null);
    const sub = subscribe.data;

    useImperativeHandle(ref, () => ({
      openReset: () => {
        if (!sub) return;
        setConfirmAction('reset-package');
      },
      openNewPeriod: () => {
        setConfirmAction('new-period');
      },
    }));

    const confirmResetPackage = async () => {
      if (!sub) return;
      const tradeNo = await saveOrder.mutateAsync({
        period: 'reset_price',
        plan_id: sub.plan_id as number,
      });
      setConfirmAction(null);
      navigate(`/order/${tradeNo}`);
    };

    const confirmNewPeriod = async () => {
      await newPeriod.mutateAsync();
      await subscribe.refetch();
      toast.success(t('dashboard.new_period_success'));
      setConfirmAction(null);
      navigate('/dashboard');
    };

    const confirmLoading = saveOrder.isPending || newPeriod.isPending;
    const confirmTitle =
      confirmAction === 'reset-package'
        ? t('dashboard.reset_package_confirm_title')
        : t('dashboard.new_period_confirm_title');
    const confirmContent =
      confirmAction === 'reset-package'
        ? t('dashboard.reset_package_confirm_content')
        : t('dashboard.new_period_confirm_content');

    return (
      <Dialog
        open={confirmAction !== null}
        onOpenChange={(open) => {
          if (!open && !confirmLoading) setConfirmAction(null);
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
              onClick={() => setConfirmAction(null)}
            >
              {t('common.cancel')}
            </Button>
            <Button
              type="button"
              data-testid="dashboard-confirm-primary"
              loading={confirmLoading}
              onClick={() => {
                void (confirmAction === 'reset-package'
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
  },
);

export interface DashboardSubscribeDialogHandle {
  open: () => void;
}

interface DashboardSubscribeDialogProps {
  subscribeUrl: string;
}

export const DashboardSubscribeDialog = forwardRef<
  DashboardSubscribeDialogHandle,
  DashboardSubscribeDialogProps
>(function DashboardSubscribeDialog({ subscribeUrl }, ref) {
  const { t } = useTranslation();
  const [subscribeOpen, setSubscribeOpen] = useState(false);
  const [qrOpen, setQrOpen] = useState(false);

  useImperativeHandle(ref, () => ({
    open: () => setSubscribeOpen(true),
  }));

  return (
    <>
      <Dialog open={subscribeOpen} onOpenChange={setSubscribeOpen}>
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
          <div className="flex justify-center">
            <QRCodeSVG value={subscribeUrl} />
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
});
