import { useTranslation } from 'react-i18next';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import {
  AuthEmailCodeField,
  AuthEmailWithSuffixField,
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
                suffixes={emailSuffixes}
                value={selectedEmailSuffix}
                onChange={setEmailSuffix}
                inputProps={registerInput('email')}
              />
            ) : (
              <FormField id="register-email" label={t('auth.email')}>
                <Input
                  type="email"
                  autoComplete="username"
                  placeholder="m@example.com"
                  {...registerInput('email')}
                />
              </FormField>
            )}

            {config?.is_email_verify ? (
              <AuthEmailCodeField
                id="register-email-code"
                label={t('auth.email_code')}
                buttonLabel={cooldown === 60 ? t('auth.send_code') : cooldown}
                disabled={cooldown !== 60 || isSendingCode}
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
            />
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
                disabled={Boolean(initialInviteCode)}
                defaultValue={initialInviteCode ?? undefined}
                autoComplete="off"
                {...registerInput('invite_code')}
              />
            </FormField>

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
