import { cloneElement, isValidElement, useState } from 'react';
import type { MouseEvent, ReactElement, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
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
  const [account, setAccount] = useState('');

  const onOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    setMethod(undefined);
    setAccount('');
  };

  const show = () => {
    setOpen((currentOpen) => !currentOpen);
    setMethod(undefined);
    setAccount('');
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

  const child = isValidElement(children)
    ? (children as ReactElement<{ onClick?: (event: MouseEvent) => void }>)
    : null;
  const trigger = child ? (
    cloneElement(child, {
      onClick: (event: MouseEvent) => {
        child.props.onClick?.(event);
        show();
      },
    })
  ) : (
    <Button type="button" variant="outline" onClick={() => show()}>
      {t('invite.withdraw_button')}
    </Button>
  );

  return (
    <>
      {trigger}
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="v2board-invite-dialog sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="v2board-invite-dialog-title">
              {t('invite.withdraw')}
            </DialogTitle>
            <DialogDescription>{t('invite.withdraw_button')}</DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="invite-withdraw-method">{t('invite.withdraw_method')}</Label>
              <Select value={method} onValueChange={setMethod}>
                <SelectTrigger
                  id="invite-withdraw-method"
                  className="v2board-invite-select-trigger"
                >
                  <SelectValue placeholder={t('invite.withdraw_method_placeholder')} />
                </SelectTrigger>
                <SelectContent className="v2board-invite-select-content">
                  {methods.map((item) => (
                    <SelectItem key={item} value={item}>
                      {item}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="invite-withdraw-account">{t('invite.withdraw_account')}</Label>
              <Input
                id="invite-withdraw-account"
                placeholder={t('invite.withdraw_account_placeholder')}
                value={account}
                onChange={(event) => setAccount(event.target.value)}
              />
            </div>
          </div>

          <DialogFooter className="v2board-invite-dialog-footer">
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            <Button type="button" loading={withdraw.isPending} onClick={() => void onSubmit()}>
              {t('profile.confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
