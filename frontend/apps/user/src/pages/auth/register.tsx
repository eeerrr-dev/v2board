import { useEffect, useRef, useState, type FormEvent } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Mail, UserPlus } from 'lucide-react';
import { LanguageMenu } from '@/components/layout/language-menu';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardFooter } from '@/components/ui/card';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Spinner } from '@/components/ui/spinner';
import { useLegacyRecaptcha } from '@/components/legacy-recaptcha';
import {
  useGuestConfig,
  useRegisterMutation,
  useSendEmailVerifyMutation,
} from '@/lib/guest';
import { i18nGet } from '@/lib/errors';
import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { PasswordField } from './password-field';

function readFormValue(form: HTMLFormElement | null, name: string) {
  if (!form) return '';
  return String(new FormData(form).get(name) ?? '');
}

export default function RegisterPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const guestConfig = useGuestConfig();
  const { data: config } = guestConfig;
  const configLoading = useLegacyFetchLoading(guestConfig.isFetching);
  const { mutateAsync: register, isPending } = useRegisterMutation();
  const { mutateAsync: sendCode, isPending: isSendingCode } = useSendEmailVerifyMutation();
  const { run: runRecaptcha, recaptchaModal } = useLegacyRecaptcha(
    Boolean(config?.is_recaptcha),
    config?.recaptcha_site_key,
  );

  const initialInviteCode = params.get('code');
  const formRef = useRef<HTMLFormElement | null>(null);
  const cooldownRef = useRef(60);
  const [emailSuffix, setEmailSuffix] = useState<string | undefined>(undefined);
  const [tosChecked, setTosChecked] = useState(false);
  const [cooldown, setCooldown] = useState(60);
  const logo = getLegacyLogo();
  const title = getLegacyTitle();
  const description = getLegacyDescription();
  const emailWhitelistSuffix = config?.email_whitelist_suffix;
  const emailSuffixes = Array.isArray(emailWhitelistSuffix) ? emailWhitelistSuffix : [];
  const hasEmailWhitelist = Boolean(emailWhitelistSuffix);
  const selectedEmailSuffix = hasEmailWhitelist
    ? emailSuffixes.includes(emailSuffix ?? '')
      ? emailSuffix
      : (emailSuffixes[0] ?? '')
    : '';
  const getEmail = () => {
    const email = readFormValue(formRef.current, 'email');
    return hasEmailWhitelist ? `${email}@${selectedEmailSuffix}` : email;
  };

  const startSendEmailVerifyCountdown = () => {
    window.setTimeout(() => {
      if (cooldownRef.current !== 0) {
        cooldownRef.current -= 1;
        setCooldown(cooldownRef.current);
        startSendEmailVerifyCountdown();
      } else {
        cooldownRef.current = 60;
        setCooldown(60);
      }
    }, 1000);
  };

  useEffect(() => {
    if (!hasEmailWhitelist) {
      if (emailSuffix !== '') setEmailSuffix('');
      return;
    }
    if (emailSuffix !== selectedEmailSuffix) setEmailSuffix(selectedEmailSuffix);
  }, [emailSuffix, hasEmailWhitelist, selectedEmailSuffix]);

  const onSendCode = async (recaptchaData?: string) => {
    try {
      const sent = await sendCode({
        email: getEmail(),
        isforget: 0,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (!sent) return;
      toast.success('发送成功', { description: '如果没有收到验证码请检查垃圾箱。' });
      startSendEmailVerifyCountdown();
    } catch {}
  };

  const onRegister = async (recaptchaData?: string) => {
    if (config?.tos_url && !tosChecked) {
      toast.error(i18nGet('请求失败'), { description: t('auth.tos_required') });
      return;
    }
    const password = readFormValue(formRef.current, 'password');
    if (password !== readFormValue(formRef.current, 'confirm_password')) {
      toast.error(i18nGet('请求失败'), { description: t('auth.password_mismatch') });
      return;
    }
    try {
      await register({
        email: getEmail(),
        password,
        invite_code:
          readFormValue(formRef.current, 'invite_code') || initialInviteCode || '',
        email_code: config?.is_email_verify ? readFormValue(formRef.current, 'email_code') : '',
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      navigate('/login');
    } catch {}
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    runRecaptcha(onRegister);
  };

  return (
    <>
      <Card className="v2board-register-card">
        <form ref={formRef} noValidate onSubmit={submit}>
          <CardBody>
            <div className="tw:mb-7 tw:text-center">
              {logo ? (
                <h1 className="tw:m-0">
                  <img
                    className="v2board-logo tw:mx-auto tw:h-11 tw:w-auto"
                    src={logo}
                    alt={title || 'V2Board'}
                  />
                </h1>
              ) : (
                <h1 className="v2board-login-title tw:text-2xl tw:font-semibold tw:tracking-tight">
                  {title || 'V2Board'}
                </h1>
              )}
              {description ? (
                <p className="tw:mt-2 tw:text-sm tw:text-foreground-muted">{description}</p>
              ) : null}
            </div>

            {configLoading ? (
              <div className="tw:flex tw:min-h-64 tw:items-center tw:justify-center" role="status">
                <Spinner className="tw:size-6 tw:text-primary" />
              </div>
            ) : (
              <div className="tw:space-y-5">
                {hasEmailWhitelist ? (
                  <div className="tw:space-y-1.5">
                    <Label htmlFor="register-email">{t('auth.email')}</Label>
                    <div className="tw:flex tw:gap-2">
                      <Input
                        id="register-email"
                        type="text"
                        name="email"
                        autoComplete="username"
                        className="tw:flex-1"
                      />
                      <select
                        aria-label={t('auth.email')}
                        className="tw:h-10 tw:rounded-field tw:border tw:border-input tw:bg-surface tw:px-3 tw:text-sm tw:text-foreground tw:shadow-sm tw:outline-none tw:transition tw:focus-visible:border-primary tw:focus-visible:ring-2 tw:focus-visible:ring-ring/25"
                        value={selectedEmailSuffix}
                        onChange={(event) => setEmailSuffix(event.target.value)}
                      >
                        {emailSuffixes.map((suffix) => (
                          <option key={suffix} value={suffix}>
                            @{suffix}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>
                ) : (
                  <FormField id="register-email" label={t('auth.email')}>
                    <Input type="email" name="email" autoComplete="username" />
                  </FormField>
                )}

                {config?.is_email_verify ? (
                  <div className="tw:flex tw:items-end tw:gap-2">
                    <FormField
                      id="register-email-code"
                      label={t('auth.email_code')}
                      className="tw:flex-1"
                    >
                      <Input type="text" name="email_code" inputMode="numeric" />
                    </FormField>
                    <Button
                      type="button"
                      disabled={cooldown !== 60 || isSendingCode}
                      loading={isSendingCode}
                      onClick={() => runRecaptcha(onSendCode)}
                      className="tw:min-w-20"
                    >
                      {cooldown === 60 ? (
                        <>
                          <Mail aria-hidden="true" className="tw:size-4" />
                          {t('auth.send_code')}
                        </>
                      ) : (
                        cooldown
                      )}
                    </Button>
                  </div>
                ) : null}

                <FormField id="register-password" label={t('auth.password')}>
                  <PasswordField name="password" autoComplete="new-password" />
                </FormField>
                <FormField id="register-confirm-password" label={t('auth.password')}>
                  <PasswordField name="confirm_password" autoComplete="new-password" />
                </FormField>
                <FormField
                  id="register-invite-code"
                  label={
                    config?.is_invite_force
                      ? t('auth.invite_code')
                      : t('auth.invite_code_optional')
                  }
                >
                  <Input
                    type="text"
                    name="invite_code"
                    disabled={Boolean(initialInviteCode)}
                    defaultValue={initialInviteCode ?? undefined}
                    autoComplete="off"
                  />
                </FormField>

                {config?.tos_url ? (
                  <label className="tw:flex tw:items-start tw:gap-2 tw:text-sm tw:text-foreground-muted">
                    <input
                      type="checkbox"
                      checked={tosChecked}
                      onChange={() => setTosChecked((value) => !value)}
                      className="tw:mt-1 tw:size-4 tw:rounded tw:border-input tw:accent-primary"
                    />
                    <span
                      dangerouslySetInnerHTML={{
                        __html: t('auth.tos_html').replace('{url}', config.tos_url),
                      }}
                    />
                  </label>
                ) : null}

                <Button
                  type="submit"
                  block
                  loading={isPending}
                  disabled={isPending || Boolean(config?.tos_url && !tosChecked)}
                  className="tw:ring-offset-surface"
                >
                  <UserPlus aria-hidden="true" className="tw:size-4" />
                  {t('auth.submit_register')}
                </Button>
              </div>
            )}
          </CardBody>
        </form>

        <CardFooter>
          <a
            className="tw:rounded tw:text-foreground-muted tw:transition tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40 tw:focus-visible:ring-offset-2 tw:ring-offset-surface"
            href="#/login"
          >
            {t('auth.return_to_login')}
          </a>
          <div className="tw:ml-auto">
            <LanguageMenu reskin showLabel triggerClassName="v2board-login-i18n-btn" />
          </div>
        </CardFooter>
      </Card>
      {recaptchaModal}
    </>
  );
}
