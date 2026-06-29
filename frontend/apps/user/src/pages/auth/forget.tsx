import { useTranslation } from 'react-i18next';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import {
  AuthEmailCodeField,
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
    registerInput,
    submit,
    sendCode,
    passwordMismatch,
    isPending,
    isSendingCode,
    cooldownActive,
    cooldownRemaining,
    recaptchaModal,
  } = useForgetController();

  return (
    <>
      <AuthPanel
        title={t('auth.reset_title')}
        description={t('auth.reset_description')}
        onSubmit={submit}
        footer={<AuthFooterLink href="#/login">{t('auth.return_to_login')}</AuthFooterLink>}
      >
        {configLoading ? (
          <AuthLoadingState />
        ) : (
          <AuthFormStack>
            <FormField id="forget-email" label={t('auth.email')}>
              <Input
                type="email"
                autoComplete="username"
                placeholder="m@example.com"
                {...registerInput('email')}
              />
            </FormField>

            <AuthEmailCodeField
              id="forget-email-code"
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

            <AuthPasswordConfirmationFields
              passwordId="forget-password"
              passwordLabel={t('auth.password')}
              passwordInputProps={registerInput('password')}
              confirmId="forget-confirm-password"
              confirmLabel={t('auth.confirm_password')}
              confirmInputProps={registerInput('confirm_password')}
              confirmError={passwordMismatch ? t('auth.password_mismatch') : undefined}
            />

            <AuthSubmitButton loading={isPending} disabled={isPending}>
              {t('auth.submit_reset')}
            </AuthSubmitButton>
          </AuthFormStack>
        )}
      </AuthPanel>
      {recaptchaModal}
    </>
  );
}
