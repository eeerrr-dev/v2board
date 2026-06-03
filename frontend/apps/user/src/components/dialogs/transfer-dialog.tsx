import { cloneElement, isValidElement, useState } from 'react';
import type { MouseEvent, ReactElement, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { userKeys, useTransferMutation } from '@/lib/queries';
import { getLegacySettings } from '@/lib/legacy-settings';

interface TransferDialogProps {
  max?: number;
  children?: ReactNode;
}

export function TransferDialog({ max, children }: TransferDialogProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { mutateAsync } = useTransferMutation();
  const [yuan, setYuan] = useState<string | undefined>();
  const [open, setOpen] = useState(false);

  const onOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    setYuan(undefined);
  };

  const show = () => {
    setOpen((currentOpen) => !currentOpen);
    setYuan(undefined);
  };

  const onSubmit = async () => {
    try {
      await mutateAsync(yuan);
      show();
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    } catch {}
  };

  const trigger = isValidElement(children)
    ? cloneElement(children as ReactElement<{ onClick?: (event: MouseEvent) => void }>, {
        onClick: (_event: MouseEvent) => show(),
      })
    : (
      <button
        type="button"
        className="btn btn-primary mr-2"
        onClick={() => show()}
      >
        {t('invite.transfer')}
      </button>
    );

  return (
    <>
      {trigger}
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent
          title={t('dashboard.transfer_to_balance')}
          okText={t('invite.withdraw_submit')}
          cancelText={t('common.cancel')}
          onOk={() => void onSubmit()}
        >
          <div className="alert alert-danger d-flex align-items-center" role="alert">
            <div className="flex-00-auto">
              <i className="fa fa-fw fa-info-circle" />
            </div>
            <div className="flex-fill ml-3">
              <p className="mb-0">{t('invite.transfer_notice', { title: getLegacySettings().title })}</p>
            </div>
          </div>
          <div className="form-group">
            <label>{t('invite.current_commission_balance')}</label>
            <input
              disabled
              type="text"
              className="ant-input form-control"
              value={Number(max) / 100}
            />
          </div>
          <div className="form-group">
            <label>{t('invite.transfer_amount')}</label>
            <input
              type="text"
              className="ant-input form-control"
              placeholder={t('invite.transfer_placeholder')}
              onChange={(e2) => setYuan(e2.target.value)}
            />
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
