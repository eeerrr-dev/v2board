import { useTranslation } from 'react-i18next';
import { ErrorState } from '@v2board/ui/error-state';
import { Input } from '@v2board/ui/input';
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
    configError,
    retryConfig,
    registerInput,
    submit,
    sendCode,
    errors,
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
        title={t(($) => $.auth.register_title)}
        description={t(($) => $.auth.register_description)}
        onSubmit={submit}
        footer={
          <>
            {t(($) => $.auth.have_account)}{' '}
            <AuthFooterLink to="/login">{t(($) => $.auth.sign_in)}</AuthFooterLink>
          </>
        }
      >
        {configLoading ? (
          <AuthLoadingState />
        ) : configError ? (
          <ErrorState data-testid="register-config-error" onRetry={retryConfig} />
        ) : (
          <AuthFormStack>
            {hasEmailWhitelist ? (
              <AuthEmailWithSuffixField
                id="register-email"
                label={t(($) => $.auth.email)}
                selectLabel={t(($) => $.auth.email_domain)}
                suffixes={emailSuffixes}
                value={selectedEmailSuffix}
                onChange={setEmailSuffix}
                inputProps={registerInput('email')}
                error={errors.email}
              />
            ) : (
              <AuthField id="register-email" label={t(($) => $.auth.email)} error={errors.email}>
                <Input
                  id="register-email"
                  type="email"
                  autoComplete="username"
                  placeholder="m@example.com"
                  aria-invalid={errors.email ? true : undefined}
                  aria-describedby={errors.email ? 'register-email-error' : undefined}
                  {...registerInput('email')}
                />
              </AuthField>
            )}

            {config?.is_email_verify ? (
              <AuthEmailCodeField
                id="register-email-code"
                label={t(($) => $.auth.email_code)}
                buttonLabel={cooldownActive ? cooldownRemaining : t(($) => $.auth.send_code)}
                buttonAriaLabel={
                  cooldownActive
                    ? t(($) => $.auth.code_sent, { seconds: cooldownRemaining })
                    : undefined
                }
                disabled={cooldownActive || isSendingCode}
                loading={isSendingCode}
                onSendCode={sendCode}
                inputProps={registerInput('email_code')}
                error={errors.emailCode}
              />
            ) : null}

            <AuthPasswordConfirmationFields
              passwordId="register-password"
              passwordLabel={t(($) => $.auth.password)}
              passwordInputProps={registerInput('password')}
              confirmId="register-confirm-password"
              confirmLabel={t(($) => $.auth.confirm_password)}
              confirmInputProps={registerInput('confirm_password')}
              passwordError={errors.password}
              confirmError={errors.confirmPassword}
            />
            <AuthField
              id="register-invite-code"
              error={errors.inviteCode}
              label={
                config?.is_invite_force
                  ? t(($) => $.auth.invite_code)
                  : t(($) => $.auth.invite_code_optional)
              }
            >
              <Input
                id="register-invite-code"
                type="text"
                disabled={Boolean(initialInviteCode)}
                defaultValue={initialInviteCode ?? undefined}
                autoComplete="off"
                aria-invalid={errors.inviteCode ? true : undefined}
                aria-describedby={errors.inviteCode ? 'register-invite-code-error' : undefined}
                {...registerInput('invite_code')}
              />
            </AuthField>

            {config?.tos_url ? (
              <AuthTosField
                id="register-tos"
                checked={tosChecked}
                url={config.tos_url}
                onToggle={() => setTosChecked((value) => !value)}
              />
            ) : null}

            <AuthSubmitButton
              loading={isPending}
              disabled={isPending || Boolean(config?.tos_url && !tosChecked)}
            >
              {t(($) => $.auth.submit_register)}
            </AuthSubmitButton>
          </AuthFormStack>
        )}
      </AuthPanel>
      {recaptchaModal}
    </>
  );
}
