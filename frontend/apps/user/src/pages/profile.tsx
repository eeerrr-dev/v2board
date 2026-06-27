import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  AlertCircle,
  Bell,
  Gift,
  KeyRound,
  MessageCircle,
  RefreshCcw,
  Send,
  WalletCards,
} from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { PageShell } from '@/components/ui/page';
import {
  PreferenceRow,
  ProfileConfirmDialog,
  ProfileDepositDialog,
  ProfileField,
  ProfileSwitch,
  ProfileTelegramBindDialog,
  type ProfileConfirmAction,
  type ProfilePreferenceKey,
} from './profile-components';
import {
  useChangePasswordMutation,
  useCommConfig,
  useRedeemGiftCardMutation,
  useResetSubscribeMutation,
  useSaveOrderMutation,
  useSubscribe,
  useTelegramBotInfo,
  useUnbindTelegramMutation,
  useUpdateProfileMutation,
  useUserInfo,
} from '@/lib/queries';
import { toast } from '@/lib/toast';

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
  const saveOrder = useSaveOrderMutation();

  const [giftCard, setGiftCard] = useState('');
  const [passwordForm, setPasswordForm] = useState({
    oldPassword: '',
    newPassword: '',
    confirmPassword: '',
  });
  const [depositInput, setDepositInput] = useState('');
  const [depositAmount, setDepositAmount] = useState<number | undefined>(undefined);
  const [redeemTimeoutStuck, setRedeemTimeoutStuck] = useState(false);
  const [depositOpen, setDepositOpen] = useState(false);
  const [telegramOpen, setTelegramOpen] = useState(false);
  const [confirmAction, setConfirmAction] = useState<ProfileConfirmAction>(null);
  const [updatingPref, setUpdatingPref] = useState<Record<ProfilePreferenceKey, boolean>>({
    auto_renewal: false,
    remind_expire: false,
    remind_traffic: false,
  });

  const botInfo = useTelegramBotInfo(telegramOpen);
  const data = info.data;
  const currency = comm?.currency;
  const depositPlaceholder = t('profile.deposit_placeholder', { currency });
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
    if (passwordForm.newPassword !== passwordForm.confirmPassword) {
      toast.error(t('profile.password_mismatch'));
      return;
    }
    try {
      await changePassword.mutateAsync({
        oldPassword: passwordForm.oldPassword,
        newPassword: passwordForm.newPassword,
      });
      toast.success(t('profile.change_password_success'));
      navigate('/login');
    } catch {}
  };

  const onRedeem = async () => {
    if (giftCard.length === 0) {
      toast.error(t('profile.redeem_placeholder'));
      return;
    }
    setRedeemTimeoutStuck(false);
    try {
      const result = await redeem.mutateAsync(giftCard);
      void info.refetch();
      toast.success(
        t('profile.redeem_success', {
          detail: redeemGiftcardText(result.type, result.value, t),
        }),
      );
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
    setDepositInput('');
    setDepositOpen(true);
  };

  const closeDeposit = () => {
    setDepositOpen(false);
    setDepositInput('');
  };

  const onDeposit = () => {
    // The legacy page keeps the last typed amount on the page instance and only
    // destroys the input DOM. Preserve that small behavior quirk.
    void saveOrder
      .mutateAsync({
        plan_id: 0,
        period: 'deposit',
        deposit_amount: depositAmount,
      })
      .then((tradeNo) => navigate(`/order/${tradeNo}`))
      .catch(() => {});
    closeDeposit();
  };

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
                  value={giftCard}
                  onChange={(event) => setGiftCard(event.target.value)}
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
                  value={passwordForm.oldPassword}
                  onChange={(value) =>
                    setPasswordForm((current) => ({ ...current, oldPassword: value }))
                  }
                />
                <ProfileField
                  id="profile-new-password"
                  label={t('profile.new_password')}
                  placeholder={t('profile.new_password_placeholder')}
                  value={passwordForm.newPassword}
                  onChange={(value) =>
                    setPasswordForm((current) => ({ ...current, newPassword: value }))
                  }
                />
                <ProfileField
                  id="profile-confirm-password"
                  label={t('profile.new_password')}
                  placeholder={t('profile.new_password_placeholder')}
                  value={passwordForm.confirmPassword}
                  onChange={(value) =>
                    setPasswordForm((current) => ({ ...current, confirmPassword: value }))
                  }
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

      <ProfileDepositDialog
        open={depositOpen}
        input={depositInput}
        placeholder={depositPlaceholder}
        onClose={closeDeposit}
        onInputChange={(value) => {
          setDepositInput(value);
          setDepositAmount(Number(value) * 100);
        }}
        onConfirm={onDeposit}
      />
      <ProfileTelegramBindDialog
        open={telegramOpen}
        botUsername={botInfo.data?.username}
        subscribeUrl={subscribe?.subscribe_url}
        onClose={() => setTelegramOpen(false)}
      />
      <ProfileConfirmDialog
        action={confirmAction}
        onCancel={() => setConfirmAction(null)}
        onConfirm={onConfirmAction}
      />
    </>
  );
}

function redeemGiftcardText(
  type: number,
  value: number,
  t: ReturnType<typeof useTranslation>['t'],
) {
  switch (type) {
    case 1:
      return t('profile.redeem_balance', { amount: (value / 100).toFixed(2) });
    case 2:
      return t('profile.redeem_days', { days: value });
    case 3:
      return t('profile.redeem_traffic', { traffic: value });
    case 4:
      return t('profile.redeem_reset');
    case 5:
      return t('profile.redeem_plan_days', { days: value });
    default:
      return t('profile.redeem_unknown');
  }
}

function isLegacyTimeoutError(error: unknown) {
  return error instanceof Error && /timeout/i.test(error.message);
}

function formatCentsPlain(cents: number) {
  return (parseInt(String(cents)) / 100).toFixed(2);
}
