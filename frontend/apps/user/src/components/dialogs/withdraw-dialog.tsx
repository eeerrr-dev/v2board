import { cloneElement, isValidElement, useState } from 'react';
import type { MouseEvent, ReactElement, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { LegacySelect } from '@/components/legacy-select';
import { useWithdrawCommissionMutation } from '@/lib/queries';

interface WithdrawDialogProps {
  methods: string[];
  children?: ReactNode;
}

export function WithdrawDialog({ methods, children }: WithdrawDialogProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const withdraw = useWithdrawCommissionMutation();
  const [open, setOpen] = useState(false);
  const [method, setMethod] = useState<string | undefined>();
  const [account, setAccount] = useState<string | undefined>();

  const onOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    setMethod(undefined);
    setAccount(undefined);
  };

  const show = () => {
    setOpen((currentOpen) => !currentOpen);
    setMethod(undefined);
    setAccount(undefined);
  };

  const onSubmit = async () => {
    try {
      await withdraw.mutateAsync({
        withdraw_method: method,
        withdraw_account: account,
      });
      navigate('/ticket');
      show();
    } catch {}
  };

  const trigger = isValidElement(children)
    ? cloneElement(children as ReactElement<{ onClick?: (event: MouseEvent) => void }>, {
        onClick: (_event: MouseEvent) => show(),
      })
    : (
      <button
        type="button"
        className="btn"
        onClick={() => show()}
      >
        {t('invite.withdraw_button')}
      </button>
    );

  return (
    <>
      {trigger}
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent
          title={t('invite.withdraw')}
          okText={t('invite.withdraw_submit')}
          cancelText={t('common.cancel')}
          onOk={() => void onSubmit()}
        >
          <div className="form-group">
            <label>{t('invite.withdraw_method')}</label>
            {/* Original wraps the antd Select in a class-less <div> (umi.js @1140053). */}
            <div>
              <LegacySelect
                style={{ width: '100%' }}
                value={method}
                placeholder={t('invite.withdraw_method_placeholder')}
                options={methods.map((item) => ({ value: item, label: item }))}
                onChange={(nextMethod) => setMethod(String(nextMethod))}
              />
            </div>
          </div>
          <div className="form-group">
            <label>{t('invite.withdraw_account')}</label>
            <input
              type="text"
              className="ant-input form-control"
              onChange={(event) => setAccount(event.target.value)}
              placeholder={t('invite.withdraw_account_placeholder')}
            />
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
