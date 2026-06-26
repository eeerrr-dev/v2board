import { useTranslation } from 'react-i18next';
import { AlertCircle } from 'lucide-react';
import { AuthBrandHeader } from '@/components/layout/auth-brand-header';
import { AuthLanguageMenu } from '@/components/layout/auth-language-menu';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardFooter } from '@/components/ui/card';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import { PasswordField } from './password-field';
import { useLoginController } from './use-login-controller';

export default function LoginPage() {
  const { t } = useTranslation();
  const { submit, clearError, isPending, error } = useLoginController();

  return (
    // Authored V2Board — reference implementation of the 2026 reskin. Pure presentation over the
    // shared design tokens (@v2board/tokens) and base primitives (Card/FormField/Input/Button);
    // all behavior lives in useLoginController. The submit mechanism is modernized to a native
    // <form> (Enter submits natively; the old global keydown listener and refs are retired) while
    // the request/redirect contract is preserved. Visual parity for this surface is retired (see
    // `user-login` visualRetired in visual-parity.mjs); the behavior/contract gate still holds.
    // The brand title is a semantic <h1> (the page's main heading); the redesign-aware
    // login-language-persistence gate releases its titleText as redesigned presentation instead of
    // pinning the old empty value.
    <Card className="v2board-auth-card">
      <form noValidate onSubmit={(event) => void submit(event)} onInput={clearError}>
        <CardBody>
          <AuthBrandHeader />

          <div className="tw:space-y-5">
            {error ? (
              <div
                id="login-error"
                role="alert"
                className="tw:flex tw:items-start tw:gap-2 tw:rounded-field tw:border tw:border-destructive/30 tw:bg-destructive-subtle tw:px-3.5 tw:py-2.5 tw:text-sm tw:text-destructive"
              >
                <AlertCircle aria-hidden="true" className="tw:mt-0.5 tw:h-4 tw:w-4 tw:shrink-0" />
                <span>{error}</span>
              </div>
            ) : null}

            {/* A proper email field (type="email"): the redesign-aware behavior gate normalizes the
                identifier input's type so this modernization is released while password masking stays
                gated. With noValidate the email type adds no submit-blocking native validation, so the
                request/redirect contract is unchanged. On failure both fields are marked invalid and
                described by the single alert box (#login-error) for assistive tech. */}
            <FormField id="login-email" label={t('auth.email')}>
              <Input
                type="email"
                name="email"
                autoComplete="username"
                invalid={!!error}
                aria-describedby={error ? 'login-error' : undefined}
              />
            </FormField>
            {/* PasswordField adds a 2026 reveal affordance. It defaults to hidden (type="password"),
                while the reveal control is a native button released by the redesigned behavior gate. */}
            <FormField id="login-password" label={t('auth.password')}>
              <PasswordField
                name="password"
                autoComplete="current-password"
                invalid={!!error}
                aria-describedby={error ? 'login-error' : undefined}
              />
            </FormField>

            {/* ring-offset-surface ties the focus-ring offset to the surface token, so on the dark
                login theme the halo blends into the dark card instead of leaving a white gap; in
                light mode the surface token is white, so the offset is unchanged. */}
            <Button type="submit" size="lg" block loading={isPending} className="tw:ring-offset-surface">
              {t('auth.submit_login')}
            </Button>
          </div>
        </CardBody>
      </form>

      <CardFooter>
        {/* HashRouter — native `#/route` anchors navigate without JS handlers. */}
        <a
          className="tw:rounded tw:text-foreground-muted tw:transition tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40 tw:focus-visible:ring-offset-2 tw:ring-offset-surface"
          href="#/register"
        >
          {t('auth.sign_up')}
        </a>
        <span aria-hidden="true" className="tw:text-border">
          ·
        </span>
        <a
          className="tw:rounded tw:text-foreground-muted tw:transition tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40 tw:focus-visible:ring-offset-2 tw:ring-offset-surface"
          href="#/forgetpassword"
        >
          {t('auth.forget_password')}
        </a>
        <div className="tw:ml-auto">
          <AuthLanguageMenu />
        </div>
      </CardFooter>
    </Card>
  );
}
