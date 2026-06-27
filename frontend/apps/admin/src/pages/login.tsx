import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { passport, user } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { getAuthData, setAuthData } from '@/lib/auth';
import { getAdminBackgroundUrl, getAdminLogo, getAdminTitle } from '@/lib/legacy-settings';
import { legacyHref } from '@/lib/legacy-href';
import { LegacyLoadingIcon } from '@/components/legacy-ant-icon';
import { legacyInfo } from '@/components/legacy-confirm';

export default function LoginPage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const [submitting, setSubmitting] = useState(false);
  const emailRef = useRef<HTMLInputElement | null>(null);
  const passwordRef = useRef<HTMLInputElement | null>(null);
  const logo = getAdminLogo();
  const title = getAdminTitle();
  const backgroundUrl = getAdminBackgroundUrl();
  const legacyBackgroundImage = (backgroundUrl && `url(${backgroundUrl})`) as string;
  const redirectParam = params.get('redirect');
  const redirect = redirectParam || 'dashboard';

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
    } catch {
      // Login failures are surfaced by the global onError handler (legacy parity).
      setSubmitting(false);
    }
  }, [navigate]);

  useEffect(() => {
    if (getAuthData()) {
      user
        .checkLogin(apiClient)
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
      // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
      if (event.keyCode === 13) void onLogin();
    };
    window.addEventListener('keydown', keyDown, false);
    return () => window.removeEventListener('keydown', keyDown, false);
  }, [onLogin]);

  return (
    <div id="page-container">
      <main id="main-container">
        <div className="v2board-background" style={{ backgroundImage: legacyBackgroundImage }} />
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
                    onClick={() => {
                      void legacyInfo({
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
                      });
                    }}
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
