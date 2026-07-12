import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  AuthField,
  AuthFieldError,
  AuthFormStack,
  AuthInlineError,
  AuthSubmitButton,
} from './auth-fields';
import { AuthAuxiliaryLink, AuthFooterLink, AuthPanel } from './auth-panel';
import { useLoginController } from './use-login-controller';

export default function LoginPage() {
  const { t } = useTranslation();
  const { registerInput, submit, clearError, isPending, error, emailError, passwordError } =
    useLoginController();

  return (
    <AuthPanel
      title={t($ => $.auth.login_title)}
      description={t($ => $.auth.login_description)}
      onSubmit={(event) => void submit(event)}
      onInput={clearError}
      footer={
        <>
          {t($ => $.auth.no_account)} <AuthFooterLink to="/register">{t($ => $.auth.sign_up)}</AuthFooterLink>
        </>
      }
    >
      <AuthFormStack>
        {error ? <AuthInlineError id="login-error">{error}</AuthInlineError> : null}

        <AuthField id="login-email" label={t($ => $.auth.email)} error={emailError}>
          <Input
            id="login-email"
            type="email"
            autoComplete="username"
            placeholder="m@example.com"
            aria-invalid={error || emailError ? true : undefined}
            aria-describedby={error ? 'login-error' : emailError ? 'login-email-error' : undefined}
            {...registerInput('email')}
          />
        </AuthField>
        <div className="grid gap-3">
          <div className="flex items-center">
            <Label htmlFor="login-password">{t($ => $.auth.password)}</Label>
            <AuthAuxiliaryLink to="/forgetpassword" className="ml-auto">
              {t($ => $.auth.forget_password)}
            </AuthAuxiliaryLink>
          </div>
          <Input
            id="login-password"
            type="password"
            autoComplete="current-password"
            aria-invalid={error || passwordError ? true : undefined}
            aria-describedby={
              error ? 'login-error' : passwordError ? 'login-password-error' : undefined
            }
            {...registerInput('password')}
          />
          <AuthFieldError id="login-password-error" message={passwordError} />
        </div>

        <AuthSubmitButton loading={isPending}>{t($ => $.auth.submit_login)}</AuthSubmitButton>
      </AuthFormStack>
    </AuthPanel>
  );
}
