import { useTranslation } from 'react-i18next';
import { LanguageMenu } from '@/components/layout/language-menu';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardFooter } from '@/components/ui/card';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';
import { useLoginController } from './use-login-controller';

export default function LoginPage() {
  const { t } = useTranslation();
  const { submit, clearError, isPending, error } = useLoginController();
  const logo = getLegacyLogo();
  const title = getLegacyTitle();
  const description = getLegacyDescription();

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
    <Card>
      <form noValidate onSubmit={(event) => void submit(event)} onInput={clearError}>
        <CardBody>
          <div className="tw:mb-7 tw:text-center">
            {/* The page always exposes exactly one top-level heading: the operator logo is wrapped
                in the <h1> (its alt names the heading) when set, otherwise the title text is the h1. */}
            {logo ? (
              <h1 className="tw:m-0">
                <img
                  className="v2board-logo tw:mx-auto tw:h-11 tw:w-auto"
                  src={logo}
                  alt={title || 'V2Board'}
                />
              </h1>
            ) : (
              <h1 className="tw:text-2xl tw:font-semibold tw:tracking-tight tw:text-foreground">
                {title || 'V2Board'}
              </h1>
            )}
            {description ? (
              <p className="tw:mt-2 tw:text-sm tw:text-foreground-muted">{description}</p>
            ) : null}
          </div>

          <div className="tw:space-y-5">
            {error ? (
              <div
                id="login-error"
                role="alert"
                className="tw:flex tw:items-start tw:gap-2 tw:rounded-field tw:border tw:border-destructive/30 tw:bg-destructive-subtle tw:px-3.5 tw:py-2.5 tw:text-sm tw:text-destructive"
              >
                <svg
                  aria-hidden="true"
                  className="tw:mt-0.5 tw:h-4 tw:w-4 tw:shrink-0"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <circle cx="12" cy="12" r="9" />
                  <path d="M12 8v5" />
                  <path d="M12 16h.01" />
                </svg>
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
            <FormField id="login-password" label={t('auth.password')}>
              <Input
                type="password"
                name="password"
                autoComplete="current-password"
                invalid={!!error}
                aria-describedby={error ? 'login-error' : undefined}
              />
            </FormField>

            <Button type="submit" block loading={isPending}>
              {t('auth.submit_login')}
            </Button>
          </div>
        </CardBody>
      </form>

      <CardFooter>
        {/* HashRouter — native `#/route` anchors navigate without JS handlers. */}
        <a
          className="tw:rounded tw:text-foreground-muted tw:transition tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40 tw:focus-visible:ring-offset-2"
          href="#/register"
        >
          {t('auth.sign_up')}
        </a>
        <span aria-hidden="true" className="tw:text-border">
          ·
        </span>
        <a
          className="tw:rounded tw:text-foreground-muted tw:transition tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40 tw:focus-visible:ring-offset-2"
          href="#/forgetpassword"
        >
          {t('auth.forget_password')}
        </a>
        <div className="tw:ml-auto">
          <LanguageMenu reskin showLabel triggerClassName="v2board-login-i18n-btn" />
        </div>
      </CardFooter>
    </Card>
  );
}
