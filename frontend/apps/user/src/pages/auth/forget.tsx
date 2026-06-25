import { useRef, useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { KeyRound, Mail } from 'lucide-react';
import { AuthLanguageMenu } from '@/components/layout/auth-language-menu';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardFooter } from '@/components/ui/card';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import { useLegacyRecaptcha } from '@/components/legacy-recaptcha';
import { useForgetMutation, useGuestConfig, useSendEmailVerifyMutation } from '@/lib/guest';
import { authToast } from '@/lib/auth-toast';
import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';
import { PasswordField } from './password-field';

function readFormValue(form: HTMLFormElement | null, name: string) {
  if (!form) return '';
  return String(new FormData(form).get(name) ?? '');
}

export default function ForgetPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data: config } = useGuestConfig();
  const { mutateAsync: forget, isPending } = useForgetMutation();
  const { mutateAsync: sendCode, isPending: isSendingCode } = useSendEmailVerifyMutation();
  const { run: runRecaptcha, recaptchaModal } = useLegacyRecaptcha(
    Boolean(config?.is_recaptcha),
    config?.recaptcha_site_key,
  );

  const formRef = useRef<HTMLFormElement | null>(null);
  const cooldownRef = useRef(60);
  const [cooldown, setCooldown] = useState(60);
  const logo = getLegacyLogo();
  const title = getLegacyTitle();
  const description = getLegacyDescription();

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

  const onSendCode = async (recaptchaData?: string) => {
    try {
      const sent = await sendCode({
        email: readFormValue(formRef.current, 'email'),
        isforget: 1,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (!sent) return;
      authToast.success('发送成功', { description: '如果没有收到验证码请检查垃圾箱。' });
      startSendEmailVerifyCountdown();
    } catch {}
  };

  const onForget = async () => {
    const password = readFormValue(formRef.current, 'password');
    if (password !== readFormValue(formRef.current, 'confirm_password')) {
      authToast.error('请求失败', { description: '两次密码输入不同' });
      return;
    }
    try {
      await forget({
        email: readFormValue(formRef.current, 'email'),
        password,
        email_code: readFormValue(formRef.current, 'email_code'),
      });
      navigate('/login');
    } catch {}
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void onForget();
  };

  return (
    <>
      <Card className="v2board-auth-card">
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
                <h1 className="v2board-auth-title tw:text-2xl tw:font-semibold tw:tracking-tight">
                  {title || 'V2Board'}
                </h1>
              )}
              {description ? (
                <p className="tw:mt-2 tw:text-sm tw:text-foreground-muted">{description}</p>
              ) : null}
            </div>

            <div className="tw:space-y-5">
              <FormField id="forget-email" label={t('auth.email')}>
                <Input type="email" name="email" autoComplete="username" />
              </FormField>

              <div className="tw:flex tw:items-end tw:gap-2">
                <FormField id="forget-email-code" label={t('auth.email_code')} className="tw:flex-1">
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

              <FormField id="forget-password" label={t('auth.password')}>
                <PasswordField name="password" autoComplete="new-password" />
              </FormField>
              <FormField id="forget-confirm-password" label={t('auth.password')}>
                <PasswordField name="confirm_password" autoComplete="new-password" />
              </FormField>

              <Button
                type="submit"
                block
                loading={isPending}
                disabled={isPending}
                className="tw:ring-offset-surface"
              >
                <KeyRound aria-hidden="true" className="tw:size-4" />
                {t('auth.submit_reset')}
              </Button>
            </div>
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
            <AuthLanguageMenu />
          </div>
        </CardFooter>
      </Card>
      {recaptchaModal}
    </>
  );
}
