import { useState } from 'react';
import type { ParseKeys } from 'i18next';
import { ApiError } from '@v2board/api-client';
import { formatCentsPlain, formatLegacyDateTime } from '@v2board/config/format';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import {
  AlertCircle,
  Bell,
  CircleUser,
  Copy,
  Gift,
  KeyRound,
  MessageCircle,
  MonitorSmartphone,
  RefreshCcw,
  Send,
  WalletCards,
} from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { ErrorState } from '@/components/ui/error-state';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { PageShell } from '@/components/ui/page';
import { Spinner } from '@/components/ui/spinner';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  TableScroll,
} from '@/components/ui/table';
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
  useActiveSessions,
  useChangePasswordMutation,
  useCommConfig,
  useRedeemGiftCardMutation,
  useRemoveSessionMutation,
  useResetSubscribeMutation,
  useSaveOrderMutation,
  useSubscribe,
  useTelegramBotInfo,
  useUnbindTelegramMutation,
  useUpdateProfileMutation,
  useUserInfo,
} from '@/lib/queries';
import { toast } from '@/lib/toast';
import { getAuthData } from '@/lib/auth';
import { copyText } from '@/lib/legacy-settings';
import { makeConfirmPasswordRefinement } from './auth/refine-confirm-password';
import type { ReactNode } from 'react';

const passwordSchema = z
  .object({
    oldPassword: z.string(),
    newPassword: z.string(),
    confirmPassword: z.string(),
  })
  .superRefine(
    makeConfirmPasswordRefinement({ passwordKey: 'newPassword', confirmKey: 'confirmPassword' }),
  );

const giftCardSchema = z.object({
  code: z.string().min(1),
});

const depositSchema = z.object({
  amount: z.coerce.number().positive(),
});

type PasswordFormValues = z.infer<typeof passwordSchema>;
type GiftCardFormValues = z.infer<typeof giftCardSchema>;
type DepositFormValues = z.input<typeof depositSchema>;
type DepositPayload = z.output<typeof depositSchema>;

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
  const passwordForm = useForm<PasswordFormValues>({
    resolver: zodResolver(passwordSchema),
    defaultValues: { oldPassword: '', newPassword: '', confirmPassword: '' },
  });
  const giftCardForm = useForm<GiftCardFormValues>({
    resolver: zodResolver(giftCardSchema),
    defaultValues: { code: '' },
  });
  const depositForm = useForm<DepositFormValues, unknown, DepositPayload>({
    resolver: zodResolver(depositSchema),
    defaultValues: { amount: '' },
  });

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

  const copyUuid = async () => {
    if (data?.uuid && (await copyText(data.uuid))) toast.success(t('dashboard.copy_success'));
  };

  const togglePref = async (key: ProfilePreferenceKey, value: 0 | 1) => {
    setUpdatingPref((current) => ({ ...current, [key]: true }));
    try {
      // The mutation invalidates the user record on success, so consumers
      // refresh without a manual refetch here.
      await updateProfile.mutateAsync({ [key]: value } as Parameters<
        typeof updateProfile.mutateAsync
      >[0]);
    } catch {
    } finally {
      setUpdatingPref((current) => ({ ...current, [key]: false }));
    }
  };

  const onChangePwd = passwordForm.handleSubmit(async (values) => {
    try {
      await changePassword.mutateAsync({
        oldPassword: values.oldPassword,
        newPassword: values.newPassword,
      });
      toast.success(t('profile.change_password_success'));
      navigate('/login');
    } catch {}
  }, () => toast.error(t('profile.password_mismatch')));

  const onRedeem = giftCardForm.handleSubmit(async ({ code }) => {
    setRedeemTimeoutStuck(false);
    try {
      const result = await redeem.mutateAsync(code);
      toast.success(
        t('profile.redeem_success', {
          detail: redeemGiftcardText(result.type, result.value, t),
        }),
      );
    } catch (error) {
      if (isTransportError(error)) setRedeemTimeoutStuck(true);
    }
  }, () => toast.error(t('profile.redeem_placeholder')));

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
          // The mutation invalidates the user record; the subscribe query is
          // disabled, so it still needs an explicit refetch here.
          void subscribeQuery.refetch();
        })
        .catch(() => {});
    }
  };

  const openDeposit = () => {
    depositForm.reset({ amount: '' });
    setDepositOpen(true);
  };

  const closeDeposit = () => {
    setDepositOpen(false);
    depositForm.reset({ amount: '' });
  };

  const onDeposit = depositForm.handleSubmit(
    ({ amount }) => {
      void saveOrder
        .mutateAsync({
          plan_id: 0,
          period: 'deposit',
          // Cents must be an integer: the backend stores total_amount in an int
          // column that truncates, so a raw float (19.99 * 100 = 1998.9999…)
          // would under-credit the user by a cent. Round to the nearest cent.
          deposit_amount: Math.round(amount * 100),
        })
        .then((tradeNo) => navigate(`/order/${tradeNo}`))
        .catch(() => {});
      closeDeposit();
    },
    // A non-positive or non-numeric amount mirrors the legacy silent close.
    () => closeDeposit(),
  );

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
                  ariaLabel={t('profile.auto_renewal')}
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
            <CardContent>
              <form className="space-y-4" onSubmit={onRedeem} noValidate>
                <div className="space-y-2.5">
                  <Label htmlFor="profile-gift-card">{t('profile.redeem_giftcard')}</Label>
                  <Input
                    id="profile-gift-card"
                    data-testid="profile-giftcard-input"
                    placeholder={t('profile.redeem_placeholder')}
                    autoComplete="one-time-code"
                    invalid={giftCardForm.formState.errors.code ? true : undefined}
                    {...giftCardForm.register('code')}
                  />
                </div>
                <Button
                  type="submit"
                  className="w-full sm:w-fit"
                  data-testid="profile-redeem-button"
                  loading={redeemLoading}
                >
                  {t('profile.redeem_submit')}
                </Button>
              </form>
            </CardContent>
          </Card>
        </div>

        <Card data-testid="profile-account-card">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="rounded-md border border-border bg-muted p-2 text-muted-foreground">
                <CircleUser className="size-4" />
              </div>
              <CardTitle className="text-lg" data-testid="profile-card-title">
                {t('profile.account')}
              </CardTitle>
            </div>
          </CardHeader>
          <CardContent>
            <div className="grid gap-x-6 gap-y-4 sm:grid-cols-2">
              <IdentityField
                label={t('profile.email')}
                value={data?.email}
                testId="profile-account-email"
              />
              <IdentityField
                label={t('profile.uuid')}
                value={data?.uuid}
                testId="profile-account-uuid"
                copyLabel={data?.uuid ? t('common.copy') : undefined}
                onCopy={data?.uuid ? copyUuid : undefined}
              />
              <IdentityField
                label={t('profile.created_at')}
                value={data?.created_at ? formatLegacyDateTime(data.created_at) : undefined}
                testId="profile-account-created"
              />
              <IdentityField
                label={t('profile.last_login')}
                value={
                  data?.last_login_at ? formatLegacyDateTime(data.last_login_at) : undefined
                }
                testId="profile-account-last-login"
              />
            </div>
          </CardContent>
        </Card>

        <ProfileSessionsCard />

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
            <CardContent>
              <form className="space-y-5" onSubmit={onChangePwd} noValidate>
                <div className="grid gap-4">
                  <ProfileField
                    id="profile-old-password"
                    label={t('profile.old_password')}
                    placeholder={t('profile.old_password_placeholder')}
                    inputProps={passwordForm.register('oldPassword')}
                  />
                  <ProfileField
                    id="profile-new-password"
                    label={t('profile.new_password')}
                    placeholder={t('profile.new_password_placeholder')}
                    inputProps={passwordForm.register('newPassword')}
                  />
                  <ProfileField
                    id="profile-confirm-password"
                    label={t('profile.new_password')}
                    placeholder={t('profile.new_password_placeholder')}
                    inputProps={passwordForm.register('confirmPassword')}
                    error={
                      passwordForm.formState.errors.confirmPassword
                        ? t('profile.password_mismatch')
                        : undefined
                    }
                  />
                </div>
                <Button
                  type="submit"
                  className="w-full sm:w-fit"
                  data-testid="profile-password-save"
                  loading={changePassword.isPending}
                >
                  {t('profile.save')}
                </Button>
              </form>
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
        placeholder={depositPlaceholder}
        inputProps={depositForm.register('amount')}
        onClose={closeDeposit}
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

function IdentityField({
  label,
  value,
  testId,
  onCopy,
  copyLabel,
}: {
  label: string;
  value: ReactNode;
  testId: string;
  onCopy?: () => void;
  copyLabel?: string;
}) {
  return (
    <div className="space-y-1">
      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div className="flex items-center gap-2 text-sm font-medium text-foreground">
        <span className="min-w-0 truncate" data-testid={testId}>
          {value == null || value === '' ? '—' : value}
        </span>
        {onCopy ? (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="size-6 shrink-0 text-muted-foreground"
            aria-label={copyLabel}
            data-testid={`${testId}-copy`}
            onClick={() => void onCopy()}
          >
            <Copy className="size-3.5" />
          </Button>
        ) : null}
      </div>
    </div>
  );
}

// Active-session management. The backend keeps a USER_SESSIONS map keyed by an
// opaque guid; each entry carries the device UA, source IP, login time, and the
// JWT that session authenticates with. The current device is the entry whose
// auth_data equals the stored token, so it is badged and cannot revoke itself
// (self sign-out is the header logout). Revoking any other entry drops it from
// the map and the query invalidation refetches the shortened list.
function ProfileSessionsCard() {
  const { t } = useTranslation();
  const sessions = useActiveSessions({ refetchOnMount: 'always' });
  const removeSession = useRemoveSessionMutation();
  const currentToken = getAuthData();

  const entries = sessions.data
    ? Object.entries(sessions.data).sort(([, a], [, b]) => b.login_at - a.login_at)
    : [];

  const onRevoke = (sessionId: string) => {
    void confirmDialog({
      title: t('common.attention'),
      description: t('profile.session_revoke_confirm'),
      confirmText: t('profile.session_revoke'),
      onConfirm: () =>
        removeSession.mutateAsync(sessionId).then(() => {
          toast.success(t('profile.session_revoke_success'));
        }),
    });
  };

  return (
    <Card data-testid="profile-sessions-card">
      <CardHeader>
        <div className="flex items-center gap-3">
          <div className="rounded-md border border-border bg-muted p-2 text-muted-foreground">
            <MonitorSmartphone className="size-4" />
          </div>
          <div className="space-y-1">
            <CardTitle className="text-lg" data-testid="profile-card-title">
              {t('profile.active_sessions')}
            </CardTitle>
            <CardDescription>{t('profile.active_sessions_desc')}</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className="p-0">
        {sessions.isLoading ? (
          <div className="flex items-center gap-2 px-6 pb-6 text-sm text-muted-foreground">
            <Spinner className="size-4" />
            <span>{t('common.loading')}</span>
          </div>
        ) : sessions.isError ? (
          <div className="px-6 pb-6">
            <ErrorState
              onRetry={() => void sessions.refetch()}
              data-testid="profile-sessions-error"
            />
          </div>
        ) : entries.length === 0 ? (
          <div
            className="px-6 pb-6 text-sm text-muted-foreground"
            data-testid="profile-sessions-empty"
          >
            {t('profile.no_sessions')}
          </div>
        ) : (
          <TableScroll className="pb-2">
            <Table className="min-w-[560px]">
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead>{t('profile.session_device')}</TableHead>
                  <TableHead>{t('profile.session_ip')}</TableHead>
                  <TableHead>{t('profile.session_login_at')}</TableHead>
                  <TableHead className="text-right">
                    <span className="sr-only">{t('profile.session_revoke')}</span>
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {entries.map(([sessionId, session]) => {
                  const isCurrent = Boolean(currentToken) && session.auth_data === currentToken;
                  return (
                    <TableRow key={sessionId} data-testid="profile-session-row">
                      <TableCell className="max-w-[18rem]">
                        <div className="flex items-center gap-2">
                          <span className="min-w-0 truncate" title={session.ua}>
                            {session.ua || '—'}
                          </span>
                          {isCurrent ? (
                            <Badge variant="secondary" data-testid="profile-session-current">
                              {t('profile.session_current')}
                            </Badge>
                          ) : null}
                        </div>
                      </TableCell>
                      <TableCell className="font-mono text-xs">{session.ip || '—'}</TableCell>
                      <TableCell className="whitespace-nowrap text-muted-foreground">
                        {formatLegacyDateTime(session.login_at)}
                      </TableCell>
                      <TableCell className="text-right">
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="text-destructive hover:text-destructive"
                          disabled={isCurrent}
                          data-testid="profile-session-revoke"
                          onClick={() => onRevoke(sessionId)}
                        >
                          {t('profile.session_revoke')}
                        </Button>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </TableScroll>
        )}
      </CardContent>
    </Card>
  );
}

function redeemGiftcardText(
  type: number,
  value: number,
  // A minimal callable instead of the full TFunction: passing the heavy i18next
  // t type into this helper and calling it with interpolation overflows the TS
  // instantiation depth (TS2589). Keys are still checked against ParseKeys.
  t: (key: ParseKeys, options?: Record<string, string | number>) => string,
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

// A gift-card redeem that never gets a backend response (timeout / network
// drop) leaves the button in the legacy "stuck loading" state. The api-client
// already models every transport-level failure as ApiError.status === 0, so key
// off that structured signal instead of string-sniffing the message (the same
// anti-pattern api.test.ts forbids in the api layer).
function isTransportError(error: unknown) {
  return error instanceof ApiError && error.status === 0;
}
