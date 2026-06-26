import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Mail, UserPlus } from 'lucide-react';
import { AuthBrandHeader } from '@/components/layout/auth-brand-header';
import { AuthLanguageMenu } from '@/components/layout/auth-language-menu';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardFooter } from '@/components/ui/card';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Spinner } from '@/components/ui/spinner';
import { PasswordField } from './password-field';
import { useRegisterController } from './use-register-controller';

// Render the operator's ToS sentence as real JSX rather than dangerouslySetInnerHTML: the template
// (`...<a target="_blank" href="{url}">label</a>...`) is split into its text + link parts, and the
// operator-controlled url is handed to React as a real href prop (so React attribute-escapes it).
// The link also gains rel="noopener noreferrer", and lives outside the checkbox label.
function renderTosSentence(template: string, url: string): ReactNode {
  const match = template.match(/^([\s\S]*?)<a\b[^>]*>([\s\S]*?)<\/a>([\s\S]*)$/);
  if (!match) return template;
  const [, before, linkText, after] = match;
  return (
    <>
      {before}
      <a
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        className="tw:text-primary tw:underline tw:transition tw:hover:text-primary-hover"
      >
        {linkText}
      </a>
      {after}
    </>
  );
}

export default function RegisterPage() {
  const { t } = useTranslation();
  const {
    config,
    configLoading,
    formRef,
    submit,
    sendCode,
    isPending,
    isSendingCode,
    cooldown,
    hasEmailWhitelist,
    emailSuffixes,
    selectedEmailSuffix,
    setEmailSuffix,
    tosChecked,
    setTosChecked,
    initialInviteCode,
    recaptchaModal,
  } = useRegisterController();

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
                      <Select
                        aria-label={t('auth.email')}
                        value={selectedEmailSuffix}
                        onChange={(event) => setEmailSuffix(event.target.value)}
                      >
                        {emailSuffixes.map((suffix) => (
                          <option key={suffix} value={suffix}>
                            @{suffix}
                          </option>
                        ))}
                      </Select>
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
                      onClick={sendCode}
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
                <FormField id="register-confirm-password" label={t('auth.confirm_password')}>
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
                  <div className="tw:flex tw:items-start tw:gap-2 tw:text-sm tw:text-foreground-muted">
                    <input
                      type="checkbox"
                      checked={tosChecked}
                      onChange={() => setTosChecked((value) => !value)}
                      aria-labelledby="register-tos-text"
                      className="tw:mt-1 tw:size-4 tw:rounded tw:border-input tw:accent-primary"
                    />
                    <span id="register-tos-text">
                      {renderTosSentence(t('auth.tos_html'), config.tos_url)}
                    </span>
                  </div>
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
            <AuthLanguageMenu />
          </div>
        </CardFooter>
      </Card>
      {recaptchaModal}
    </>
  );
}
