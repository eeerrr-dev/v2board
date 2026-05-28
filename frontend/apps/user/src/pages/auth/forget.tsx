import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { LanguageMenu } from '@/components/layout/language-menu';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { useLegacyRecaptcha } from '@/components/legacy-recaptcha';
import { useForgetMutation, useGuestConfig, useSendEmailVerifyMutation } from '@/lib/guest';
import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';

export default function ForgetPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data: config } = useGuestConfig();
  const { mutateAsync: forget, isPending } = useForgetMutation();
  const { mutateAsync: sendCode, isPending: isSendingCode } = useSendEmailVerifyMutation();
  const { run: runRecaptcha, recaptchaModal } = useLegacyRecaptcha(
    config?.is_recaptcha === 1,
    config?.recaptcha_site_key,
  );

  const [email, setEmail] = useState('');
  const [emailCode, setEmailCode] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [cooldown, setCooldown] = useState(60);
  const logo = getLegacyLogo();
  const title = getLegacyTitle();
  const description = getLegacyDescription();

  useEffect(() => {
    if (cooldown >= 60) return;
    const timer = window.setTimeout(() => {
      setCooldown((value) => (value > 0 ? value - 1 : 60));
    }, 1000);
    return () => window.clearTimeout(timer);
  }, [cooldown]);

  const onSendCode = async (recaptchaData?: string) => {
    try {
      const sent = await sendCode({
        email,
        isforget: 1,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (!sent) return;
      toast.success('发送成功', { description: '如果没有收到验证码请检查垃圾箱。' });
      window.setTimeout(() => setCooldown(59), 1000);
    } catch {}
  };

  const onForget = async () => {
    if (password !== confirmPassword) {
      toast.error('请求失败', { description: '两次密码输入不同' });
      return;
    }
    try {
      await forget({ email, password, email_code: emailCode });
      navigate('/login');
    } catch {}
  };

  return (
    <>
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
                <div className="form-group form-row">
                  <div className="col-9">
                    <input
                      type="text"
                      className="form-control form-control-alt"
                      placeholder={t('auth.email_code')}
                      value={emailCode}
                      onChange={(event) => setEmailCode(event.target.value)}
                    />
                  </div>
                  <div className="col-3">
                    <button
                      type="submit"
                      disabled={cooldown !== 60 || isSendingCode}
                      className="btn btn-block btn-primary"
                      onClick={() => runRecaptcha(onSendCode)}
                    >
                      {cooldown === 60
                        ? isSendingCode
                          ? <LegacyLoadingIcon />
                          : t('auth.send_code')
                        : cooldown}
                    </button>
                  </div>
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
                <div className="form-group">
                  <input
                    type="password"
                    className="form-control form-control-alt"
                    placeholder={t('auth.password')}
                    value={confirmPassword}
                    onChange={(event) => setConfirmPassword(event.target.value)}
                  />
                </div>
                <div className="form-group mb-0">
                  <button
                    disabled={isPending}
                    type="submit"
                    className="btn btn-block btn-primary font-w400"
                    onClick={() => void onForget()}
                  >
                    {isPending ? (
                      <LegacyLoadingIcon />
                    ) : (
                      <span>
                        <i className="si si-support mr-1" />
                        {t('auth.submit_reset')}
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
                onClick={() => navigate('/login')}
              >
                {t('auth.return_to_login')}
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
      {recaptchaModal}
    </>
  );
}
