import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { App } from 'antd';
import { passport, user } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { getAuthData, setAuthData } from '@/lib/auth';
import { i18nGet } from '@/lib/errors';
import {
  getAdminBackgroundUrl,
  getAdminLogo,
  getAdminTitle,
} from '@/lib/legacy-settings';
import { legacyHref } from '@/lib/legacy-href';

function LegacyLoadingIcon() {
  return (
    <i aria-label="图标: loading" className="anticon anticon-loading">
      <svg
        className="anticon-spin"
        viewBox="0 0 1024 1024"
        focusable="false"
        data-icon="loading"
        width="1em"
        height="1em"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M988 548c-19.9 0-36-16.1-36-36 0-59.4-11.6-117-34.6-171.3a440.45 440.45 0 0 0-94.3-139.9 437.71 437.71 0 0 0-139.9-94.3C629 83.6 571.4 72 512 72c-19.9 0-36-16.1-36-36s16.1-36 36-36c69.1 0 136.2 13.5 199.3 40.3C772.3 66 827 103 874 150c47 47 83.9 101.8 109.7 162.7 26.7 63.1 40.2 130.2 40.2 199.3.1 19.9-16 36-35.9 36z" />
      </svg>
    </i>
  );
}

function normalizeRedirectTarget(target: string | null): string {
  if (!target) return '/dashboard';
  return target.startsWith('/') ? target : `/${target}`;
}

export default function LoginPage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const { message, modal } = App.useApp();
  const [submitting, setSubmitting] = useState(false);
  const emailRef = useRef<HTMLInputElement | null>(null);
  const passwordRef = useRef<HTMLInputElement | null>(null);
  const logo = getAdminLogo();
  const title = getAdminTitle();
  const backgroundUrl = getAdminBackgroundUrl();
  const redirectParam = params.get('redirect');
  const redirect = normalizeRedirectTarget(redirectParam);

  const onLogin = useCallback(async () => {
    setSubmitting(true);
    try {
      const result = await passport.login(apiClient, {
        email: emailRef.current!.value,
        password: passwordRef.current!.value,
      });
      setSubmitting(false);
      setAuthData(result.auth_data);
      if (!result.is_admin) {
        return;
      }
      navigate('/dashboard');
      void user.info(apiClient).catch(() => undefined);
    } catch (error) {
      if (error instanceof Error) message.error(i18nGet(error.message));
      setSubmitting(false);
    }
  }, [message, navigate]);

  useEffect(() => {
    if (getAuthData()) {
      user.checkLogin(apiClient)
        .then((result) => {
          if (result.is_admin) {
            void user.info(apiClient).catch(() => undefined);
            navigate(redirect);
          }
        })
        .catch(() => undefined);
    }
  }, [navigate, redirect]);

  useEffect(() => {
    const keyDown = (event: KeyboardEvent) => {
      if (event.keyCode === 13) void onLogin();
    };
    window.addEventListener('keydown', keyDown, false);
    return () => window.removeEventListener('keydown', keyDown, false);
  }, [onLogin]);

  return (
    <div id="page-container">
      <main id="main-container">
        <div
          className="v2board-background"
          style={{ backgroundImage: backgroundUrl ? `url(${backgroundUrl})` : undefined }}
        />
        <div className="no-gutters v2board-auth-box">
          <div className="" style={{ maxWidth: 450, width: '100%', margin: 'auto' }}>
            <div className="mx-2 mx-sm-0">
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
                        <p className="font-size-sm text-muted mb-3">登录到管理中心</p>
                      </div>
                      <div className="form-group">
                        <input
                          type="text"
                          className="form-control form-control-alt"
                          placeholder="邮箱"
                          ref={emailRef}
                        />
                      </div>
                      <div className="form-group">
                        <input
                          type="password"
                          className="form-control form-control-alt"
                          placeholder="密码"
                          ref={passwordRef}
                        />
                      </div>
                      <div className="form-group mb-0">
                        <button
                          disabled={submitting}
                          type="submit"
                          className="btn btn-block btn-primary font-w400"
                          onClick={() => void onLogin()}
                        >
                          {submitting ? (
                            <LegacyLoadingIcon />
                          ) : (
                            <span>
                              <i className="si si-login mr-1" />
                              登入
                            </span>
                          )}
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
                <div className="text-center bg-gray-lighter p-3 px-4">
                  <a
                    onClick={() =>
                      modal.info({
                        title: '忘记密码',
                        content: (
                          <div>
                            <div>在站点目录下执行命令找回密码</div>
                            <code>php artisan reset:password 管理员邮箱</code>
                          </div>
                        ),
                        centered: true,
                        okText: '我知道了',
                        onOk() {},
                      })}
                  >
                    忘记密码
                  </a>
                </div>
              </div>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
