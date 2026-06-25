import { useCallback, useEffect, useRef } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { user } from '@v2board/api-client';
import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { LanguageMenu } from '@/components/layout/language-menu';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardFooter } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { getAuthData, setAuthData } from '@/lib/auth';
import { useLoginMutation, useTokenLoginMutation } from '@/lib/guest';
import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';
import { apiClient } from '@/lib/api';
import { fetchUserInfo, userKeys } from '@/lib/queries';

function normalizeRedirectTarget(target: string | null): string {
  if (!target) return '/dashboard';
  return target.startsWith('/') ? target : `/${target}`;
}

export default function LoginPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const queryClient = useQueryClient();
  const { mutateAsync, isPending } = useLoginMutation();
  const { mutateAsync: tokenLogin } = useTokenLoginMutation();
  const emailRef = useRef<HTMLInputElement | null>(null);
  const passwordRef = useRef<HTMLInputElement | null>(null);
  const logo = getLegacyLogo();
  const title = getLegacyTitle();
  const description = getLegacyDescription();
  const queryRedirect = params.get('redirect');
  const redirect = normalizeRedirectTarget(queryRedirect);
  const verify = params.get('verify');

  const onLogin = useCallback(async () => {
    try {
      const result = await mutateAsync({
        email: emailRef.current!.value,
        password: passwordRef.current!.value,
      });
      setAuthData(result.auth_data);
      // The saga dispatches user/getUserInfo with `put`, then immediately pushes.
      // It never waits for the user-info request to settle.
      void queryClient
        .fetchQuery({ queryKey: userKeys.info, queryFn: fetchUserInfo })
        .catch(() => undefined);
      navigate(redirect);
    } catch {}
  }, [mutateAsync, navigate, queryClient, redirect]);

  useEffect(() => {
    const finishLogin = (authData: string) => {
      setAuthData(authData);
      navigate(redirect);
    };

    if (verify) {
      tokenLogin({
        verify,
        ...(queryRedirect !== null ? { redirect: queryRedirect } : {}),
      })
        .then((result) => {
          if (result?.auth_data) finishLogin(result.auth_data);
        })
        .catch(() => undefined);
    }

    if (getAuthData()) {
      user.checkLogin(apiClient)
        .then((result) => {
          if (result.is_login) {
            void queryClient
              .fetchQuery({ queryKey: userKeys.info, queryFn: fetchUserInfo })
              .catch(() => undefined);
            navigate(redirect);
          }
        })
        .catch(() => undefined);
    }
  }, [navigate, queryClient, queryRedirect, redirect, tokenLogin, verify]);

  useEffect(() => {
    const keyDown = (event: KeyboardEvent) => {
      if (event.keyCode === 13) void onLogin();
    };

    window.addEventListener('keydown', keyDown, false);
    return () => window.removeEventListener('keydown', keyDown, false);
  }, [onLogin]);

  return (
    // Authored V2Board — reference implementation of the clean-modern reskin. Built on the
    // shared design tokens (@v2board/tokens) and base components (Card/Input/Label/Button), so
    // it carries no bespoke utility strings. Behavior is preserved exactly (ref-based submit,
    // global Enter-key shortcut, token login, redirect order); only the presentation diverges
    // from the packaged oracle, so this surface's pixel parity is retired (see `user-login`
    // visualRetired in visual-parity.mjs). The heading stays an <h2> (never <h1>/.block-title)
    // so the login-language-persistence interaction's titleText stays '' versus the oracle.
    <Card>
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
            <p className="tw:mt-2 tw:text-sm tw:text-muted-foreground">{description}</p>
          ) : null}
        </div>

        <div className="tw:space-y-5">
          <div className="tw:space-y-1.5">
            <Label htmlFor="login-email">{t('auth.email')}</Label>
            <Input id="login-email" type="text" ref={emailRef} />
          </div>
          <div className="tw:space-y-1.5">
            <Label htmlFor="login-password">{t('auth.password')}</Label>
            <Input id="login-password" type="password" ref={passwordRef} />
          </div>
          <Button block loading={isPending} onClick={() => void onLogin()}>
            {t('auth.submit_login')}
          </Button>
        </div>
      </CardBody>

      <CardFooter>
        {/* HashRouter — native `#/route` anchors navigate without JS handlers. */}
        <a className="tw:text-muted-foreground tw:transition tw:hover:text-foreground" href="#/register">
          {t('auth.sign_up')}
        </a>
        <span aria-hidden="true" className="tw:text-border">
          ·
        </span>
        <a
          className="tw:text-muted-foreground tw:transition tw:hover:text-foreground"
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
