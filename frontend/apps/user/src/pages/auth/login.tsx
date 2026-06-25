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
    // Authored V2Board — clean-modern reskin of the login surface (Tailwind utilities).
    // Behavior is preserved (ref-based submit, Enter-key, token login, redirect); only the
    // presentation diverges from the packaged oracle, so this surface's pixel parity is
    // retired (see `user-login` visualRetired in visual-parity.mjs). The heading is an
    // <h2>, never <h1>/.block-title, so the login-language-persistence interaction's
    // titleText stays '' and keeps matching the oracle.
    <div className="tw:overflow-hidden tw:rounded-2xl tw:bg-white tw:shadow-[0_10px_40px_-12px_rgba(15,23,42,0.25)] tw:ring-1 tw:ring-slate-900/5">
      <div className="tw:px-6 tw:py-9 tw:sm:px-9">
        <div className="tw:mb-7 tw:text-center">
          {logo ? (
            <img className="v2board-logo tw:mx-auto tw:mb-3 tw:h-11 tw:w-auto" src={logo} alt={title || 'V2Board'} />
          ) : (
            <h2 className="tw:text-2xl tw:font-semibold tw:tracking-tight tw:text-slate-900">
              {title || 'V2Board'}
            </h2>
          )}
          {description ? <p className="tw:mt-2 tw:text-sm tw:text-slate-500">{description}</p> : null}
        </div>

        <div className="tw:space-y-5">
          <div className="tw:space-y-1.5">
            <label htmlFor="login-email" className="tw:block tw:text-sm tw:font-medium tw:text-slate-700">
              {t('auth.email')}
            </label>
            <input
              id="login-email"
              type="text"
              className="tw:block tw:w-full tw:rounded-lg tw:border tw:border-slate-300 tw:bg-white tw:px-3.5 tw:py-2.5 tw:text-sm tw:text-slate-900 tw:shadow-sm tw:transition tw:placeholder:text-slate-400 tw:focus:border-blue-500 tw:focus:outline-none tw:focus:ring-2 tw:focus:ring-blue-500/25"
              placeholder={t('auth.email')}
              ref={emailRef}
            />
          </div>
          <div className="tw:space-y-1.5">
            <label htmlFor="login-password" className="tw:block tw:text-sm tw:font-medium tw:text-slate-700">
              {t('auth.password')}
            </label>
            <input
              id="login-password"
              type="password"
              className="tw:block tw:w-full tw:rounded-lg tw:border tw:border-slate-300 tw:bg-white tw:px-3.5 tw:py-2.5 tw:text-sm tw:text-slate-900 tw:shadow-sm tw:transition tw:placeholder:text-slate-400 tw:focus:border-blue-500 tw:focus:outline-none tw:focus:ring-2 tw:focus:ring-blue-500/25"
              placeholder={t('auth.password')}
              ref={passwordRef}
            />
          </div>
          <button
            disabled={isPending}
            type="submit"
            className="tw:flex tw:w-full tw:items-center tw:justify-center tw:gap-2 tw:rounded-lg tw:bg-blue-600 tw:px-4 tw:py-2.5 tw:text-sm tw:font-semibold tw:text-white tw:shadow-sm tw:transition tw:hover:bg-blue-700 tw:focus:outline-none tw:focus:ring-2 tw:focus:ring-blue-500/40 tw:disabled:cursor-not-allowed tw:disabled:opacity-60"
            onClick={() => void onLogin()}
          >
            {isPending ? <LegacyLoadingIcon /> : t('auth.submit_login')}
          </button>
        </div>
      </div>

      <div className="tw:flex tw:items-center tw:gap-3 tw:border-t tw:border-slate-100 tw:bg-slate-50/80 tw:px-6 tw:py-4 tw:text-sm tw:sm:px-9">
        <a
          className="tw:text-slate-500 tw:transition tw:hover:text-slate-900"
          ref={legacyHref()}
          onClick={() => navigate('/register')}
        >
          {t('auth.sign_up')}
        </a>
        <span aria-hidden="true" className="tw:text-slate-300">
          ·
        </span>
        <a
          className="tw:text-slate-500 tw:transition tw:hover:text-slate-900"
          ref={legacyHref()}
          onClick={() => navigate('/forgetpassword')}
        >
          {t('auth.forget_password')}
        </a>
        <div className="tw:ml-auto">
          <LanguageMenu legacyIcon showLabel triggerClassName="v2board-login-i18n-btn" />
        </div>
      </div>
    </div>
  );
}
