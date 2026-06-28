import { useTranslation } from 'react-i18next';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  AuthFormStack,
  AuthInlineError,
  AuthSubmitButton,
} from './auth-fields';
import {
  AuthAuxiliaryLink,
  AuthFooterLink,
  AuthPanel,
} from './auth-panel';
import { PasswordField } from './password-field';
import { useLoginController } from './use-login-controller';

export default function LoginPage() {
  const { t } = useTranslation();
  const { registerInput, submit, clearError, isPending, error } = useLoginController();

  return (
    <AuthPanel
      title={t('auth.login_title')}
      description={t('auth.login_description')}
      onSubmit={(event) => void submit(event)}
      onInput={clearError}
      footer={
        <>
          {t('auth.no_account')}{' '}
          <AuthFooterLink href="#/register">{t('auth.sign_up')}</AuthFooterLink>
        </>
      }
    >
      <AuthFormStack>
        {error ? <AuthInlineError id="login-error">{error}</AuthInlineError> : null}

        <FormField id="login-email" label={t('auth.email')}>
          <Input
            type="email"
            autoComplete="username"
            placeholder="m@example.com"
            invalid={!!error}
            aria-describedby={error ? 'login-error' : undefined}
            {...registerInput('email')}
          />
        </FormField>
        <div className="grid gap-3">
          <div className="flex items-center">
            <Label htmlFor="login-password">{t('auth.password')}</Label>
            <AuthAuxiliaryLink
              href="#/forgetpassword"
              className="ml-auto"
            >
              {t('auth.forget_password')}
            </AuthAuxiliaryLink>
          </div>
          <PasswordField
            id="login-password"
            autoComplete="current-password"
            invalid={!!error}
            aria-describedby={error ? 'login-error' : undefined}
            {...registerInput('password')}
          />
        </div>

        <AuthSubmitButton loading={isPending}>{t('auth.submit_login')}</AuthSubmitButton>
      </AuthFormStack>
    </AuthPanel>
  );
}
