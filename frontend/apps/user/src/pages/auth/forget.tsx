import { useTranslation } from 'react-i18next';
import { AuthBrandHeader } from '@/components/layout/auth-brand-header';
import { AuthLanguageMenu } from '@/components/layout/auth-language-menu';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardFooter } from '@/components/ui/card';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { PasswordField } from './password-field';
import { useForgetController } from './use-forget-controller';

export default function ForgetPage() {
  const { t } = useTranslation();
  const {
    configLoading,
    formRef,
    submit,
    sendCode,
    isPending,
    isSendingCode,
    cooldown,
    recaptchaModal,
  } = useForgetController();

  return (
    <>
      <Card className="v2board-auth-card">
        <form ref={formRef} noValidate onSubmit={submit}>
          <CardBody>
            <AuthBrandHeader />

            {configLoading ? (
              <div className="tw:flex tw:min-h-64 tw:items-center tw:justify-center" role="status">
                <Spinner className="tw:size-6 tw:text-primary" />
              </div>
            ) : (
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
                    onClick={sendCode}
                    className="tw:min-w-20"
                  >
                    {cooldown === 60 ? t('auth.send_code') : cooldown}
                  </Button>
                </div>

                <FormField id="forget-password" label={t('auth.password')}>
                  <PasswordField name="password" autoComplete="new-password" />
                </FormField>
                <FormField id="forget-confirm-password" label={t('auth.confirm_password')}>
                  <PasswordField name="confirm_password" autoComplete="new-password" />
                </FormField>

                <Button
                  type="submit"
                  block
                  loading={isPending}
                  disabled={isPending}
                  className="tw:ring-offset-surface"
                >
                  {t('auth.submit_reset')}
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
            <AuthLanguageMenu />
          </div>
        </CardFooter>
      </Card>
      {recaptchaModal}
    </>
  );
}
