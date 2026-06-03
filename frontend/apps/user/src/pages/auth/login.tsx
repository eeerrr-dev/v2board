import { useCallback, useEffect, useRef } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { user } from '@v2board/api-client';
import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { LanguageMenu } from '@/components/layout/language-menu';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { getAuthData, setAuthData } from '@/lib/auth';
import { useLoginMutation, useTokenLoginMutation } from '@/lib/guest';
import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';
import { apiClient } from '@/lib/api';
import { fetchUserInfo, userKeys } from '@/lib/queries';
import { legacyHref } from '@/lib/legacy-href';

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
    <div
      className="block block-rounded block-transparent block-fx-pop w-100 mb-0 overflow-hidden bg-image"
      style={{ boxShadow: '0 0.5rem 2rem #0000000d' }}
    >
      <div className="row no-gutters">
        <div className="col-md-12 order-md-1 bg-white">
          <div className="block-content block-content-full px-lg-4 py-md-4 py-lg-4">
            <div className="mb-3 text-center">
              <a className="font-size-h1" ref={legacyHref()}>
                {logo ? (
                  <img className="v2board-logo mb-3" src={logo} />
                ) : (
                  <span className="text-dark">{title || 'V2Board'}</span>
                )}
              </a>
              {description && <p className="font-size-sm text-muted mb-3">{description}</p>}
            </div>
            <div className="form-group">
              <input
                type="text"
                className="form-control form-control-alt"
                placeholder={t('auth.email')}
                ref={emailRef}
              />
            </div>
            <div className="form-group">
              <input
                type="password"
                className="form-control form-control-alt"
                placeholder={t('auth.password')}
                ref={passwordRef}
              />
            </div>
            <div className="form-group mb-0">
              <button
                disabled={isPending}
                type="submit"
                className="btn btn-block btn-primary font-w400"
                onClick={() => void onLogin()}
              >
                {isPending ? (
                  <LegacyLoadingIcon />
                ) : (
                  <span>
                    <i className="si si-login mr-1" />
                    {t('auth.submit_login')}
                  </span>
                )}
              </button>
            </div>
          </div>
        </div>
      </div>
      <div className="text-left bg-gray-lighter p-3 px-4">
        <a
          className="font-size-sm text-muted"
          ref={legacyHref()}
          onClick={() => navigate('/register')}
        >
          {t('auth.sign_up')}
        </a>
        <div className="ant-divider ant-divider-vertical" />
        <a
          className="font-size-sm text-muted"
          ref={legacyHref()}
          onClick={() => navigate('/forgetpassword')}
        >
          {t('auth.forget_password')}
        </a>
        <LanguageMenu
          legacyIcon
          showLabel
          triggerClassName="v2board-login-i18n-btn"
        />
      </div>
    </div>
  );
}
