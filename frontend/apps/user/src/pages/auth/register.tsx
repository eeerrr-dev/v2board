import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import {
  AuthEmailCodeField,
  AuthEmailWithSuffixField,
  AuthField,
  AuthFormStack,
  AuthLoadingState,
  AuthPasswordConfirmationFields,
  AuthSubmitButton,
} from './auth-fields';
import { AuthFooterLink, AuthPanel } from './auth-panel';
import { AuthTosField } from './auth-tos-field';
import { useRegisterController } from './use-register-controller';

export default function RegisterPage() {
  const { t } = useTranslation();
  const {
    config,
    configLoading,
    registerInput,
    submit,
    sendCode,
    passwordMismatch,
    isPending,
    isSendingCode,
    cooldownActive,
    cooldownRemaining,
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
      <AuthPanel
        title={t('auth.register_title')}
        description={t('auth.register_description')}
        onSubmit={submit}
        footer={
          <>
            {t('auth.have_account')}{' '}
            <AuthFooterLink href="#/login">{t('auth.sign_in')}</AuthFooterLink>
          </>
        }
      >
        {configLoading ? (
          <AuthLoadingState />
        ) : (
          <AuthFormStack>
            {hasEmailWhitelist ? (
              <AuthEmailWithSuffixField
                id="register-email"
                label={t('auth.email')}
                selectLabel={t('auth.email_domain')}
                suffixes={emailSuffixes}
                value={selectedEmailSuffix}
                onChange={setEmailSuffix}
                inputProps={registerInput('email')}
              />
            ) : (
              <AuthField id="register-email" label={t('auth.email')}>
                <Input
                  id="register-email"
                  type="email"
                  autoComplete="username"
                  placeholder="m@example.com"
                  {...registerInput('email')}
                />
              </AuthField>
            )}

            {config?.is_email_verify ? (
              <AuthEmailCodeField
                id="register-email-code"
                label={t('auth.email_code')}
                buttonLabel={cooldownActive ? cooldownRemaining : t('auth.send_code')}
                buttonAriaLabel={
                  cooldownActive ? t('auth.code_sent', { seconds: cooldownRemaining }) : undefined
                }
                disabled={cooldownActive || isSendingCode}
                loading={isSendingCode}
                onSendCode={sendCode}
                inputProps={registerInput('email_code')}
              />
            ) : null}

            <AuthPasswordConfirmationFields
              passwordId="register-password"
              passwordLabel={t('auth.password')}
              passwordInputProps={registerInput('password')}
              confirmId="register-confirm-password"
              confirmLabel={t('auth.confirm_password')}
              confirmInputProps={registerInput('confirm_password')}
              confirmError={passwordMismatch ? t('auth.password_mismatch') : undefined}
            />
            <AuthField
              id="register-invite-code"
              label={
                config?.is_invite_force
                  ? t('auth.invite_code')
                  : t('auth.invite_code_optional')
              }
            >
              <Input
                id="register-invite-code"
                type="text"
                disabled={Boolean(initialInviteCode)}
                defaultValue={initialInviteCode ?? undefined}
                autoComplete="off"
                {...registerInput('invite_code')}
              />
            </AuthField>

            {config?.tos_url ? (
              <AuthTosField
                id="register-tos"
                checked={tosChecked}
                template={t('auth.tos_html')}
                url={config.tos_url}
                onToggle={() => setTosChecked((value) => !value)}
              />
            ) : null}

            <AuthSubmitButton
              loading={isPending}
              disabled={isPending || Boolean(config?.tos_url && !tosChecked)}
            >
              {t('auth.submit_register')}
            </AuthSubmitButton>
          </AuthFormStack>
        )}
      </AuthPanel>
      {recaptchaModal}
    </>
  );
}
