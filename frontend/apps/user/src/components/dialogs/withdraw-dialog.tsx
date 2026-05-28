import { cloneElement, isValidElement, useState } from 'react';
import type { MouseEvent, ReactElement, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { AntBtn } from '@/components/ant-btn';
import { LegacySelect } from '@/components/legacy-select';
import { useWithdrawCommissionMutation } from '@/lib/queries';

interface WithdrawDialogProps {
  methods: string[];
  children?: ReactNode;
}

export function WithdrawDialog({ methods, children }: WithdrawDialogProps) {
  const { t } = useTranslation();
  const withdraw = useWithdrawCommissionMutation();
  const [open, setOpen] = useState(false);
  const [method, setMethod] = useState<string | undefined>();
  const [account, setAccount] = useState<string | undefined>();

  const onOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) {
      setMethod(undefined);
      setAccount(undefined);
    }
  };

  const onSubmit = async () => {
    try {
      await withdraw.mutateAsync({
        withdraw_method: method,
        withdraw_account: account,
      });
      setOpen(false);
      setMethod(undefined);
      setAccount(undefined);
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
      <button type="button" className="btn" onClick={() => setOpen(true)}>
        {t('invite.withdraw_button')}
      </button>
    );

  return (
    <>
      {trigger}
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="v2board-ant-modal">
          <div>
            <div className="ant-modal-header">
              <div className="ant-modal-title">{t('invite.withdraw')}</div>
            </div>
            <div className="ant-modal-body">
              <div className="form-group">
                <label htmlFor="withdraw-method">{t('invite.withdraw_method')}</label>
                <LegacySelect
                  id="withdraw-method"
                  style={{ width: '100%' }}
                  value={method ?? ''}
                  placeholder={t('invite.withdraw_method_placeholder')}
                  options={methods.map((item) => ({ value: item, label: item }))}
                  onChange={(nextMethod) => setMethod(nextMethod || undefined)}
                />
              </div>
              <div className="form-group">
                <label htmlFor="withdraw-account">{t('invite.withdraw_account')}</label>
                <input
                  id="withdraw-account"
                  type="text"
                  className="ant-input form-control"
                  value={account ?? ''}
                  onChange={(event) => setAccount(event.target.value)}
                  placeholder={t('invite.withdraw_account_placeholder')}
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
