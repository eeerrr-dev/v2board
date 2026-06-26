import { useTranslation } from 'react-i18next';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import {
  AuthFormStack,
  AuthInlineError,
  AuthSubmitButton,
} from './auth-fields';
import {
  AuthFooterDivider,
  AuthFooterLink,
  AuthPanel,
} from './auth-panel';
import { PasswordField } from './password-field';
import { useLoginController } from './use-login-controller';

export default function LoginPage() {
  const { t } = useTranslation();
  const { submit, clearError, isPending, error } = useLoginController();

  return (
    <AuthPanel
      onSubmit={(event) => void submit(event)}
      onInput={clearError}
      footer={
        <>
          <AuthFooterLink href="#/register">{t('auth.sign_up')}</AuthFooterLink>
          <AuthFooterDivider />
          <AuthFooterLink href="#/forgetpassword">{t('auth.forget_password')}</AuthFooterLink>
        </>
      }
    >
      <AuthFormStack>
        {error ? <AuthInlineError id="login-error">{error}</AuthInlineError> : null}

        <FormField id="login-email" label={t('auth.email')}>
          <Input
            type="email"
            name="email"
            autoComplete="username"
            invalid={!!error}
            aria-describedby={error ? 'login-error' : undefined}
          />
        </FormField>
        <FormField id="login-password" label={t('auth.password')}>
          <PasswordField
            name="password"
            autoComplete="current-password"
            invalid={!!error}
            aria-describedby={error ? 'login-error' : undefined}
          />
        </FormField>

        <AuthSubmitButton loading={isPending}>{t('auth.submit_login')}</AuthSubmitButton>
      </AuthFormStack>
    </AuthPanel>
  );
}
