import { useState } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
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

  const onSubmit = async () => {
    try {
      await withdraw.mutateAsync({
        withdraw_method: method,
        withdraw_account: account,
      });
      navigate('/ticket');
      onOpenChange(false);
    } catch {}
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        {children ?? (
          <Button type="button" variant="outline">
            {t('invite.withdraw_button')}
          </Button>
        )}
      </DialogTrigger>
      <DialogContent className="sm:max-w-md" data-testid="invite-dialog">
        <DialogHeader>
          <DialogTitle data-testid="invite-dialog-title">
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
                data-testid="invite-select-trigger"
              >
                <SelectValue placeholder={t('invite.withdraw_method_placeholder')} />
              </SelectTrigger>
              <SelectContent data-testid="invite-select-content">
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

        <DialogFooter data-testid="invite-dialog-footer">
          <DialogClose asChild>
            <Button type="button" variant="outline">
              {t('common.cancel')}
            </Button>
          </DialogClose>
          <Button type="button" loading={withdraw.isPending} onClick={() => void onSubmit()}>
            {t('profile.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
