import { cloneElement, isValidElement, useState } from 'react';
import type { MouseEvent, ReactElement, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { AntBtn } from '@/components/ant-btn';
import { useTransferMutation } from '@/lib/queries';
import { getLegacySettings } from '@/lib/legacy-settings';

interface TransferDialogProps {
  max?: number;
  children?: ReactNode;
}

export function TransferDialog({ max, children }: TransferDialogProps) {
  const { t } = useTranslation();
  const { mutateAsync } = useTransferMutation();
  const [yuan, setYuan] = useState<string | undefined>();
  const [open, setOpen] = useState(false);

  const onOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) setYuan(undefined);
  };

  const onSubmit = async () => {
    try {
      await mutateAsync(Number(yuan) * 100);
      setOpen(false);
      setYuan(undefined);
    } catch {}
  };

  const trigger = isValidElement(children)
    ? cloneElement(children as ReactElement<{ onClick?: (event: MouseEvent) => void }>, {
        onClick: (event: MouseEvent) => {
          (children as ReactElement<{ onClick?: (event: MouseEvent) => void }>).props.onClick?.(event);
          setOpen(true);
        },
      })
    : (
      <button type="button" className="btn btn-primary mr-2" onClick={() => setOpen(true)}>
        {t('invite.transfer')}
      </button>
    );

  return (
    <>
      {trigger}
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="v2board-ant-modal">
          <div>
            <div className="ant-modal-header">
              <div className="ant-modal-title">{t('dashboard.transfer_to_balance')}</div>
            </div>
            <div className="ant-modal-body">
              <div className="alert alert-danger d-flex align-items-center" role="alert">
                <div className="flex-00-auto">
                  <i className="fa fa-fw fa-info-circle" />
                </div>
                <div className="flex-fill ml-3">
                  <p className="mb-0">{t('invite.transfer_notice', { title: getLegacySettings().title })}</p>
                </div>
              </div>
              <div className="form-group">
                <label htmlFor="transfer-balance">{t('invite.current_commission_balance')}</label>
                <input
                  id="transfer-balance"
                  disabled
                  type="text"
                  className="ant-input form-control"
                  value={Number(max) / 100}
                  readOnly
                />
              </div>
              <div className="form-group">
                <label htmlFor="transfer-amount">{t('invite.transfer_amount')}</label>
                <input
                  id="transfer-amount"
                  type="text"
                  className="ant-input form-control"
                  placeholder={t('invite.transfer_placeholder')}
                  value={yuan ?? ''}
                  onChange={(e2) => setYuan(e2.target.value)}
                />
              </div>
            </div>
            <div className="ant-modal-footer">
              <AntBtn type="button" className="ant-btn" onClick={() => onOpenChange(false)}>
                {t('common.cancel')}
              </AntBtn>
              <AntBtn type="button" className="ant-btn ant-btn-primary" onClick={() => void onSubmit()}>
                {t('invite.withdraw_submit')}
              </AntBtn>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
