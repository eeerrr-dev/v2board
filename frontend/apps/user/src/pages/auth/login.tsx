import { useCallback, useEffect, useState } from 'react';
import { useLocation, useNavigate, useSearchParams } from 'react-router-dom';
import { user } from '@v2board/api-client';
import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { LanguageMenu } from '@/components/layout/language-menu';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { getAuthData, setAuthData } from '@/lib/auth';
import { useLoginMutation, useTokenLoginMutation } from '@/lib/guest';
import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';
import { apiClient } from '@/lib/api';
import { userKeys } from '@/lib/queries';

export default function LoginPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const [params] = useSearchParams();
  const queryClient = useQueryClient();
  const { mutateAsync, isPending } = useLoginMutation();
  const { mutateAsync: tokenLogin } = useTokenLoginMutation();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const logo = getLegacyLogo();
  const title = getLegacyTitle();
  const description = getLegacyDescription();
  const stateRedirect = (location.state as { redirect?: string } | null)?.redirect;
  const queryRedirect = params.get('redirect');
  const redirect = queryRedirect ?? stateRedirect ?? '/dashboard';
  const verify = params.get('verify');

  const onLogin = useCallback(async () => {
    try {
      const result = await mutateAsync({ email, password });
      setAuthData(result.auth_data);
      await queryClient.invalidateQueries({ queryKey: userKeys.info });
      navigate(redirect || '/dashboard');
    } catch {}
  }, [email, mutateAsync, navigate, password, queryClient, redirect]);

  useEffect(() => {
    let cancelled = false;

    const finishLogin = async (authData: string) => {
      setAuthData(authData);
      await queryClient.invalidateQueries({ queryKey: userKeys.info });
      if (!cancelled) navigate(redirect || '/dashboard');
    };

    if (verify) {
      tokenLogin({
        verify,
        ...(queryRedirect !== null ? { redirect: queryRedirect } : {}),
      })
        .then((result) => {
          if (result?.auth_data) void finishLogin(result.auth_data);
        })
        .catch(() => undefined);
    }

    if (getAuthData()) {
      user.checkLogin(apiClient)
        .then(async (result) => {
          if (result.is_login && !cancelled) {
            await queryClient.invalidateQueries({ queryKey: userKeys.info });
            if (!cancelled) navigate(redirect || '/dashboard');
          }
        })
        .catch(() => undefined);
    }

    return () => {
      cancelled = true;
    };
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
              <a className="font-size-h1" href="javascript:void(0);">
                {logo ? (
                  <img className="v2board-logo mb-3" src={logo} />
                ) : (
                  <span className="text-dark">{title || 'V2Board'}</span>
                )}
              </a>
              {description && <p className="font-size-sm text-muted mb-3">{description}</p>}
            </div>
            <div>
              <div className="form-group">
                <input
                  type="text"
                  className="form-control form-control-alt"
                  placeholder={t('auth.email')}
                  value={email}
                  onChange={(event) => setEmail(event.target.value)}
                />
              </div>
              <div className="form-group">
                <input
                  type="password"
                  className="form-control form-control-alt"
                  placeholder={t('auth.password')}
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
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
          <div className="text-left bg-gray-lighter p-3 px-4">
            <a
              className="font-size-sm text-muted"
              href="javascript:void(0);"
              onClick={() => navigate('/register')}
            >
              {t('auth.sign_up')}
            </a>
            <span className="ant-divider ant-divider-vertical" role="separator" />
            <a
              className="font-size-sm text-muted"
              href="javascript:void(0);"
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
      </div>
    </div>
  );
}
