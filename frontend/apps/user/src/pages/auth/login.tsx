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
    // The heading stays an <h2> (never <h1>/.block-title) so the login-language-persistence
    // interaction's titleText stays '' versus the oracle.
    <Card>
      <form noValidate onSubmit={(event) => void submit(event)} onInput={clearError}>
        <CardBody>
          <div className="tw:mb-7 tw:text-center">
            {logo ? (
              <img
                className="v2board-logo tw:mx-auto tw:mb-3 tw:h-11 tw:w-auto"
                src={logo}
                alt={title || 'V2Board'}
              />
            ) : (
              <h2 className="tw:text-2xl tw:font-semibold tw:tracking-tight tw:text-foreground">
                {title || 'V2Board'}
              </h2>
            )}
            {description ? (
              <p className="tw:mt-2 tw:text-sm tw:text-foreground-muted">{description}</p>
            ) : null}
          </div>

          <div className="tw:space-y-5">
            {error ? (
              <div
                role="alert"
                className="tw:flex tw:items-start tw:gap-2 tw:rounded-field tw:border tw:border-destructive/30 tw:bg-destructive-subtle tw:px-3.5 tw:py-2.5 tw:text-sm tw:text-destructive"
              >
                {error}
              </div>
            ) : null}

            {/* type stays "text" (not "email"): the user-home-root-page-state behavior gate
                captures input type and compares it to the oracle, which used "text". */}
            <FormField id="login-email" label={t('auth.email')}>
              <Input type="text" name="email" autoComplete="username" />
            </FormField>
            <FormField id="login-password" label={t('auth.password')}>
              <Input type="password" name="password" autoComplete="current-password" />
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
          className="tw:text-foreground-muted tw:transition tw:hover:text-foreground"
          href="#/register"
        >
          {t('auth.sign_up')}
        </a>
        <span aria-hidden="true" className="tw:text-border">
          ·
        </span>
        <a
          className="tw:text-foreground-muted tw:transition tw:hover:text-foreground"
          href="#/forgetpassword"
        >
          {t('auth.forget_password')}
        </a>
        <div className="tw:ml-auto">
          <LanguageMenu legacyIcon showLabel triggerClassName="v2board-login-i18n-btn" />
        </div>
      </CardFooter>
    </Card>
  );
}
