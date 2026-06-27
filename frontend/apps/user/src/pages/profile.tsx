import { useRef, useState, type Ref } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { user } from '@v2board/api-client';
import {
  AlertCircle,
  Bell,
  Copy,
  Gift,
  KeyRound,
  Link2,
  MessageCircle,
  RefreshCcw,
  Send,
  WalletCards,
} from 'lucide-react';
import { apiClient } from '@/lib/api';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
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
import { PageShell } from '@/components/ui/page';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import {
  useChangePasswordMutation,
  useCommConfig,
  useRedeemGiftCardMutation,
  useResetSubscribeMutation,
  useSubscribe,
  useTelegramBotInfo,
  useUnbindTelegramMutation,
  useUpdateProfileMutation,
  useUserInfo,
} from '@/lib/queries';
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';

type ProfilePreferenceKey = 'auto_renewal' | 'remind_expire' | 'remind_traffic';
type ConfirmAction = 'reset-subscribe' | 'unbind-telegram' | null;

export default function ProfilePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const info = useUserInfo({ refetchOnMount: 'always' });
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  // The original /profile never dispatches user/getSubscribe on mount; it only reads
  // whatever subscribe data already sits in the dva store and re-fetches solely after
  // unbinding Telegram. Keep that contract while replacing the presentation layer.
  const subscribeQuery = useSubscribe({ enabled: false });
  const subscribe = subscribeQuery.data;
  const updateProfile = useUpdateProfileMutation();
  const changePassword = useChangePasswordMutation();
  const redeem = useRedeemGiftCardMutation();
  const resetSub = useResetSubscribeMutation();
  const unbindTelegram = useUnbindTelegramMutation();

  const giftCardRef = useRef<HTMLInputElement>(null);
  const oldPasswordRef = useRef<HTMLInputElement>(null);
  const newPasswordRef = useRef<HTMLInputElement>(null);
  const confirmPasswordRef = useRef<HTMLInputElement>(null);
  const depositInputRef = useRef<HTMLInputElement>(null);
  const depositAmountRef = useRef<number | undefined>(undefined);

  const [redeemTimeoutStuck, setRedeemTimeoutStuck] = useState(false);
  const [depositOpen, setDepositOpen] = useState(false);
  const [telegramOpen, setTelegramOpen] = useState(false);
  const [confirmAction, setConfirmAction] = useState<ConfirmAction>(null);
  const [updatingPref, setUpdatingPref] = useState<Record<ProfilePreferenceKey, boolean>>({
    auto_renewal: false,
    remind_expire: false,
    remind_traffic: false,
  });

  const botInfo = useTelegramBotInfo(telegramOpen);
  const data = info.data;
  const currency = comm?.currency;
  const depositPlaceholder = t(`请输入充值金额${currency}`);
  const redeemLoading = redeem.isPending || redeemTimeoutStuck;

  const togglePref = async (key: ProfilePreferenceKey, value: 0 | 1) => {
    let succeeded = false;
    setUpdatingPref((current) => ({ ...current, [key]: true }));
    try {
      await updateProfile.mutateAsync({ [key]: value } as Parameters<
        typeof updateProfile.mutateAsync
      >[0]);
      succeeded = true;
    } catch {
    } finally {
      setUpdatingPref((current) => ({ ...current, [key]: false }));
    }
    if (succeeded) void info.refetch();
  };

  const onChangePwd = async () => {
    const oldPassword = oldPasswordRef.current!.value;
    const newPassword = newPasswordRef.current!.value;
    const confirmPassword = confirmPasswordRef.current!.value;
    if (newPassword !== confirmPassword) {
      toast.error(t('profile.password_mismatch'));
      return;
    }
    try {
      await changePassword.mutateAsync({ oldPassword, newPassword });
      toast.success('修改成功，请重新登陆');
      navigate('/login');
    } catch {}
  };

  const onRedeem = async () => {
    const giftcard = giftCardRef.current!.value;
    if (giftcard.length === 0) {
      toast.error(t('profile.redeem_placeholder'));
      return;
    }
    setRedeemTimeoutStuck(false);
    try {
      const result = await redeem.mutateAsync(giftcard);
      void info.refetch();
      toast.success(`兑换成功: ${redeemGiftcardText(result.type, result.value)}`);
    } catch (error) {
      if (isLegacyTimeoutError(error)) setRedeemTimeoutStuck(true);
    }
  };

  const onReset = () => {
    setConfirmAction('reset-subscribe');
  };

  const onUnbindTelegram = () => {
    setConfirmAction('unbind-telegram');
  };

  const onConfirmAction = () => {
    const action = confirmAction;
    setConfirmAction(null);
    if (action === 'reset-subscribe') {
      void resetSub
        .mutateAsync()
        .then(() => {
          toast.success(t('profile.reset_success'));
        })
        .catch(() => {});
      return;
    }
    if (action === 'unbind-telegram') {
      void unbindTelegram
        .mutateAsync()
        .then(() => {
          toast.success(t('profile.reset_success'));
          void info.refetch();
          void subscribeQuery.refetch();
        })
        .catch(() => {});
    }
  };

  const openDeposit = () => {
    if (depositInputRef.current) depositInputRef.current.value = '';
    setDepositOpen(true);
  };

  const closeDeposit = () => {
    setDepositOpen(false);
    if (depositInputRef.current) depositInputRef.current.value = '';
  };

  const onDeposit = () => {
    // The legacy page keeps the last typed amount on the page instance and only
    // destroys the input DOM. Preserve that small behavior quirk.
    const depositAmountValue = depositAmountRef.current;
    void user
      .saveOrder(apiClient, {
        plan_id: 0,
        period: 'deposit',
        deposit_amount: depositAmountValue,
      })
      .then((tradeNo) => navigate(`/order/${tradeNo}`))
      .catch(() => {});
    closeDeposit();
  };

  const copyBindCommand = () => {
    legacyCopyText(`/bind ${subscribe?.subscribe_url}`);
  };

  const confirmTitle =
    confirmAction === 'unbind-telegram'
      ? t('profile.telegram_unbind_confirm')
      : t('profile.reset_subscribe_confirm');
  const confirmDescription =
    confirmAction === 'unbind-telegram'
      ? t('profile.telegram_unbind_tip')
      : t('profile.reset_subscribe_tip');

  return (
    <>
      <PageShell data-testid="profile-page">
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(360px,0.85fr)]">
          <Card className="overflow-hidden" data-testid="profile-wallet-card">
            <CardHeader className="gap-4">
              <div className="flex items-start justify-between gap-4">
                <div className="space-y-2">
                  <CardDescription>{t('profile.wallet')}</CardDescription>
                  <CardTitle
                    className="text-4xl font-semibold tracking-normal text-foreground sm:text-5xl"
                    data-testid="profile-card-title"
                  >
                    {data?.balance !== undefined ? formatCentsPlain(data.balance) : '--.--'}
                    <span className="ml-3 align-baseline text-base font-medium text-muted-foreground">
                      {currency}
                    </span>
                  </CardTitle>
                </div>
                <div className="rounded-md border border-border bg-muted p-2.5 text-muted-foreground">
                  <WalletCards className="size-5" />
                </div>
              </div>
            </CardHeader>
            <CardContent className="flex flex-col gap-5">
              <div className="flex flex-col gap-4 rounded-lg border border-border bg-muted/40 p-4 sm:flex-row sm:items-center sm:justify-between">
                <div className="space-y-1">
                  <div className="text-sm font-medium leading-5">
                    {t('profile.auto_renewal')}
                  </div>
                </div>
                <ProfileSwitch
                  checked={data?.auto_renewal}
                  loading={updatingPref.auto_renewal}
                  onChange={(checked) => void togglePref('auto_renewal', checked ? 1 : 0)}
                />
              </div>
              <Button
                className="w-full sm:w-fit"
                data-testid="profile-recharge"
                size="lg"
                onClick={openDeposit}
              >
                {t('profile.recharge')}
              </Button>
            </CardContent>
          </Card>

          <Card data-testid="profile-gift-card">
            <CardHeader>
              <div className="flex items-center gap-3">
                <div className="rounded-md border border-border bg-muted p-2 text-muted-foreground">
                  <Gift className="size-4" />
                </div>
                <CardTitle className="text-lg" data-testid="profile-card-title">
                  {t('profile.redeem_giftcard')}
                </CardTitle>
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2.5">
                <Label htmlFor="profile-gift-card">{t('profile.redeem_giftcard')}</Label>
                <Input
                  id="profile-gift-card"
                  data-testid="profile-giftcard-input"
                  placeholder={t('profile.redeem_placeholder')}
                  autoComplete="one-time-code"
                  ref={giftCardRef}
                />
              </div>
              <Button
                className="w-full sm:w-fit"
                data-testid="profile-redeem-button"
                loading={redeemLoading}
                onClick={() => {
                  if (!redeemLoading) void onRedeem();
                }}
              >
                {t('profile.redeem_submit')}
              </Button>
            </CardContent>
          </Card>
        </div>

        <div className="grid gap-6 lg:grid-cols-2">
          <Card data-testid="profile-password-card">
            <CardHeader>
              <div className="flex items-center gap-3">
                <div className="rounded-md border border-border bg-muted p-2 text-muted-foreground">
                  <KeyRound className="size-4" />
                </div>
                <CardTitle className="text-lg" data-testid="profile-card-title">
                  {t('profile.change_password')}
                </CardTitle>
              </div>
            </CardHeader>
            <CardContent className="space-y-5">
              <div className="grid gap-4">
                <ProfileField
                  id="profile-old-password"
                  label={t('profile.old_password')}
                  placeholder={t('profile.old_password_placeholder')}
                  inputRef={oldPasswordRef}
                />
                <ProfileField
                  id="profile-new-password"
                  label={t('profile.new_password')}
                  placeholder={t('profile.new_password_placeholder')}
                  inputRef={newPasswordRef}
                />
                <ProfileField
                  id="profile-confirm-password"
                  label={t('profile.new_password')}
                  placeholder={t('profile.new_password_placeholder')}
                  inputRef={confirmPasswordRef}
                />
              </div>
              <Button
                className="w-full sm:w-fit"
                data-testid="profile-password-save"
                loading={changePassword.isPending}
                onClick={() => {
                  if (!changePassword.isPending) void onChangePwd();
                }}
              >
                {t('profile.save')}
              </Button>
            </CardContent>
          </Card>

          <Card data-testid="profile-notifications-card">
            <CardHeader>
              <div className="flex items-center gap-3">
                <div className="rounded-md border border-border bg-muted p-2 text-muted-foreground">
                  <Bell className="size-4" />
                </div>
                <CardTitle className="text-lg" data-testid="profile-card-title">
                  {t('profile.notifications')}
                </CardTitle>
              </div>
            </CardHeader>
            <CardContent className="space-y-3">
              <PreferenceRow
                label={t('profile.remind_expire')}
                checked={data?.remind_expire}
                loading={updatingPref.remind_expire}
                onChange={(checked) => void togglePref('remind_expire', checked ? 1 : 0)}
              />
              <PreferenceRow
                label={t('profile.remind_traffic')}
                checked={data?.remind_traffic}
                loading={updatingPref.remind_traffic}
                onChange={(checked) => void togglePref('remind_traffic', checked ? 1 : 0)}
              />
            </CardContent>
          </Card>
        </div>

        <div className="grid gap-6 lg:grid-cols-2">
          {comm?.is_telegram ? (
            !data?.telegram_id ? (
              <Card data-testid="profile-telegram-bind">
                <CardHeader>
                  <div className="flex items-center justify-between gap-4">
                    <div className="flex items-center gap-3">
                      <div className="rounded-md border border-border bg-muted p-2 text-muted-foreground">
                        <Send className="size-4" />
                      </div>
                      <CardTitle className="text-lg" data-testid="profile-card-title">
                        {t('profile.telegram_bind')}
                      </CardTitle>
                    </div>
                    <Button
                      data-testid="profile-telegram-start"
                      size="sm"
                      onClick={() => setTelegramOpen(true)}
                    >
                      {t('profile.start_now')}
                    </Button>
                  </div>
                </CardHeader>
              </Card>
            ) : (
              <Card data-testid="profile-telegram-unbind">
                <CardHeader>
                  <div className="flex items-start justify-between gap-4">
                    <div className="space-y-1.5">
                      <CardTitle className="text-lg" data-testid="profile-card-title">
                        {t('profile.telegram_bind')}
                      </CardTitle>
                      <CardDescription data-testid="profile-telegram-id">
                        Telegram ID: {String(data.telegram_id)}
                      </CardDescription>
                    </div>
                    <Button
                      data-testid="profile-telegram-unbind-button"
                      variant="destructive"
                      size="sm"
                      onClick={onUnbindTelegram}
                    >
                      {t('profile.telegram_unbind')}
                    </Button>
                  </div>
                </CardHeader>
              </Card>
            )
          ) : null}

          {comm?.telegram_discuss_link ? (
            <Card data-testid="profile-telegram-discuss">
              <CardHeader>
                <div className="flex items-center justify-between gap-4">
                  <div className="flex items-center gap-3">
                    <div className="rounded-md border border-border bg-muted p-2 text-muted-foreground">
                      <MessageCircle className="size-4" />
                    </div>
                    <CardTitle className="text-lg" data-testid="profile-card-title">
                      {t('profile.telegram_discuss')}
                    </CardTitle>
                  </div>
                  <Button asChild size="sm">
                    <a href={comm.telegram_discuss_link} target="_blank" rel="noreferrer">
                      {t('profile.join_now')}
                    </a>
                  </Button>
                </div>
              </CardHeader>
            </Card>
          ) : null}

          <Card className="lg:col-span-2" data-testid="profile-reset-card">
            <CardHeader>
              <div className="flex items-center gap-3">
                <div className="rounded-md border border-border bg-muted p-2 text-muted-foreground">
                  <RefreshCcw className="size-4" />
                </div>
                <CardTitle className="text-lg" data-testid="profile-card-title">
                  {t('profile.reset_subscribe')}
                </CardTitle>
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
              <Alert data-testid="profile-reset-warning">
                <AlertCircle className="size-4" />
                <AlertDescription>{t('profile.reset_subscribe_warning')}</AlertDescription>
              </Alert>
              <Button
                className="w-full sm:w-fit"
                data-testid="profile-reset-button"
                variant="destructive"
                onClick={onReset}
              >
                {t('profile.reset')}
              </Button>
            </CardContent>
          </Card>
        </div>
      </PageShell>

      <Dialog open={depositOpen} onOpenChange={(open) => (open ? setDepositOpen(true) : closeDeposit())}>
        <DialogContent
          className="sm:max-w-md"
          data-testid="profile-deposit-dialog"
          showCloseButton={false}
        >
          <DialogHeader>
            <DialogTitle>{t('profile.recharge')}</DialogTitle>
            <DialogDescription>{depositPlaceholder}</DialogDescription>
          </DialogHeader>
          <Input
            data-testid="profile-deposit-input"
            autoComplete="one-time-code"
            placeholder={depositPlaceholder}
            ref={depositInputRef}
            onChange={(event) => {
              depositAmountRef.current = Number(event.target.value) * 100;
            }}
          />
          <DialogFooter>
            <Button variant="outline" onClick={closeDeposit}>
              {t('common.cancel')}
            </Button>
            <Button data-testid="profile-deposit-confirm" onClick={() => onDeposit()}>
              {t('profile.confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={telegramOpen} onOpenChange={setTelegramOpen}>
        <DialogContent data-testid="profile-telegram-bind-dialog">
          <DialogHeader>
            <DialogTitle>{t('profile.telegram_bind')}</DialogTitle>
          </DialogHeader>
          {botInfo.data?.username ? (
            <div className="space-y-6">
              <div className="space-y-2">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Link2 className="size-4 text-muted-foreground" />
                  {t('profile.telegram_step1')}
                </div>
                <div className="text-sm text-muted-foreground">
                  {t('profile.telegram_search')}
                  <a
                    href={`https://t.me/${botInfo.data.username}`}
                    className="ml-1 font-medium text-foreground underline underline-offset-4"
                  >
                    @{botInfo.data.username}
                  </a>
                </div>
              </div>
              <div className="space-y-2">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Copy className="size-4 text-muted-foreground" />
                  {t('profile.telegram_step2')}
                </div>
                <div className="text-sm text-muted-foreground">{t('profile.telegram_send')}</div>
                <code
                  className="flex cursor-pointer rounded-md border border-border bg-muted px-3 py-2 text-sm text-foreground"
                  data-testid="profile-copy-code"
                  onClick={() => copyBindCommand()}
                >
                  /bind {subscribe?.subscribe_url}
                </code>
              </div>
            </div>
          ) : (
            <div className="flex min-h-24 items-center justify-center">
              <Spinner />
            </div>
          )}
          <DialogFooter>
            <Button
              data-testid="profile-telegram-bind-confirm"
              onClick={() => setTelegramOpen(false)}
            >
              {t('profile.i_know')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={confirmAction !== null} onOpenChange={(open) => !open && setConfirmAction(null)}>
        <DialogContent
          className="sm:max-w-md"
          data-testid="profile-confirm-dialog"
          showCloseButton={false}
        >
          <DialogHeader>
            <DialogTitle>{confirmTitle}</DialogTitle>
            <DialogDescription>{confirmDescription}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirmAction(null)}>
              {t('common.cancel')}
            </Button>
            <Button data-testid="profile-confirm-primary" onClick={onConfirmAction}>
              {t('profile.confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

const ProfileField = ({
  id,
  inputRef,
  label,
  placeholder,
}: {
  id: string;
  inputRef: Ref<HTMLInputElement>;
  label: string;
  placeholder: string;
}) => (
  <div className="space-y-2.5">
    <Label htmlFor={id}>{label}</Label>
    <Input id={id} ref={inputRef} type="password" placeholder={placeholder} />
  </div>
);

function PreferenceRow({
  label,
  checked,
  loading,
  onChange,
}: {
  label: string;
  checked?: unknown;
  loading?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-lg border border-border p-4">
      <div className="text-sm font-medium leading-5">{label}</div>
      <ProfileSwitch checked={checked} loading={loading} onChange={onChange} />
    </div>
  );
}

function ProfileSwitch({
  checked,
  loading,
  onChange,
}: {
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
      aria-busy={!!loading}
      onCheckedChange={(nextChecked) => onChange(nextChecked)}
      onKeyDown={(event) => {
        if (event.key === 'ArrowLeft') onChange(false);
        else if (event.key === 'ArrowRight') onChange(true);
      }}
    />
  );
}

function redeemGiftcardText(type: number, value: number) {
  switch (type) {
    case 1:
      return `账户余额 ${(value / 100).toFixed(2)}`;
    case 2:
      return `订阅时长 ${value} 天`;
    case 3:
      return `套餐流量 ${value} GB`;
    case 4:
      return '流量已重置';
    case 5:
      return `订阅套餐 ${value} 天`;
    default:
      return '未知类型';
  }
}

function isLegacyTimeoutError(error: unknown) {
  return error instanceof Error && /timeout/i.test(error.message);
}

function formatCentsPlain(cents: number) {
  return (parseInt(String(cents)) / 100).toFixed(2);
}
