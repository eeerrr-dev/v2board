import { useTranslation } from 'react-i18next';
import { ErrorState } from '@v2board/ui/error-state';
import { Input } from '@v2board/ui/input';
import {
  AuthEmailCodeField,
  AuthField,
  AuthFormStack,
  AuthLoadingState,
  AuthPasswordConfirmationFields,
  AuthSubmitButton,
} from './auth-fields';
import { AuthFooterLink, AuthPanel } from './auth-panel';
import { useForgetController } from './use-forget-controller';

export default function ForgetPage() {
  const { t } = useTranslation();
  const {
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
    recaptchaModal,
  } = useForgetController();

  return (
    <>
      <AuthPanel
        title={t(($) => $.auth.reset_title)}
        description={t(($) => $.auth.reset_description)}
        onSubmit={submit}
        footer={<AuthFooterLink to="/login">{t(($) => $.auth.return_to_login)}</AuthFooterLink>}
      >
        {configLoading ? (
          <AuthLoadingState />
        ) : configError ? (
          <ErrorState data-testid="forget-config-error" onRetry={retryConfig} />
        ) : (
          <AuthFormStack>
            <AuthField id="forget-email" label={t(($) => $.auth.email)} error={errors.email}>
              <Input
                id="forget-email"
                type="email"
                autoComplete="username"
                placeholder="m@example.com"
                aria-invalid={errors.email ? true : undefined}
                aria-describedby={errors.email ? 'forget-email-error' : undefined}
                {...registerInput('email')}
              />
            </AuthField>

            <AuthEmailCodeField
              id="forget-email-code"
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

            <AuthPasswordConfirmationFields
              passwordId="forget-password"
              passwordLabel={t(($) => $.auth.password)}
              passwordInputProps={registerInput('password')}
              confirmId="forget-confirm-password"
              confirmLabel={t(($) => $.auth.confirm_password)}
              confirmInputProps={registerInput('confirm_password')}
              passwordError={errors.password}
              confirmError={errors.confirmPassword}
            />

            <AuthSubmitButton loading={isPending} disabled={isPending}>
              {t(($) => $.auth.submit_reset)}
            </AuthSubmitButton>
          </AuthFormStack>
        )}
      </AuthPanel>
      {recaptchaModal}
    </>
  );
}
