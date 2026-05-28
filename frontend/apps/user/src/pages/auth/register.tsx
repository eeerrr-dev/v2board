import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { LanguageMenu } from '@/components/layout/language-menu';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { useLegacyRecaptcha } from '@/components/legacy-recaptcha';
import {
  useGuestConfig,
  useRegisterMutation,
  useSendEmailVerifyMutation,
} from '@/lib/guest';
import { i18nGet } from '@/lib/errors';
import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';

export default function RegisterPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const { data: config, isFetching: isConfigLoading } = useGuestConfig();
  const { mutateAsync: register, isPending } = useRegisterMutation();
  const { mutateAsync: sendCode, isPending: isSendingCode } = useSendEmailVerifyMutation();
  const { run: runRecaptcha, recaptchaModal } = useLegacyRecaptcha(
    config?.is_recaptcha === 1,
    config?.recaptcha_site_key,
  );

  const initialInviteCode = params.get('code') ?? '';
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [emailCode, setEmailCode] = useState('');
  const [inviteCode, setInviteCode] = useState(initialInviteCode);
  const [emailSuffix, setEmailSuffix] = useState('');
  const [tosChecked, setTosChecked] = useState(false);
  const [cooldown, setCooldown] = useState(60);
  const logo = getLegacyLogo();
  const title = getLegacyTitle();
  const description = getLegacyDescription();
  const emailWhitelistSuffix = config?.email_whitelist_suffix;
  const emailSuffixes = Array.isArray(emailWhitelistSuffix) ? emailWhitelistSuffix : [];
  const hasEmailWhitelist = Boolean(emailWhitelistSuffix);
  const registerEmail = hasEmailWhitelist ? `${email}@${emailSuffix}` : email;

  useEffect(() => {
    if (cooldown >= 60) return;
    const timer = window.setTimeout(() => {
      setCooldown((value) => (value > 0 ? value - 1 : 60));
    }, 1000);
    return () => window.clearTimeout(timer);
  }, [cooldown]);

  useEffect(() => {
    if (emailSuffix || !hasEmailWhitelist) return;
    setEmailSuffix(emailSuffixes[0] ?? '');
  }, [emailSuffix, emailSuffixes, hasEmailWhitelist]);

  const onSendCode = async (recaptchaData?: string) => {
    try {
      const sent = await sendCode({
        email: registerEmail,
        isforget: 0,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (!sent) return;
      toast.success('发送成功', { description: '如果没有收到验证码请检查垃圾箱。' });
      window.setTimeout(() => setCooldown(59), 1000);
    } catch {}
  };

  const onRegister = async (recaptchaData?: string) => {
    if (config?.tos_url && !tosChecked) {
      toast.error(i18nGet('请求失败'), { description: t('auth.tos_required') });
      return;
    }
    if (password !== confirmPassword) {
      toast.error(i18nGet('请求失败'), { description: t('auth.password_mismatch') });
      return;
    }
    try {
      await register({
        email: registerEmail,
        password,
        invite_code: inviteCode,
        email_code: config?.is_email_verify ? emailCode : '',
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
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
              {isConfigLoading ? (
                <div className="content content-full text-center">
                  <div className="spinner-grow text-primary" role="status">
                    <span className="sr-only">Loading...</span>
                  </div>
                </div>
              ) : (
                <div>
                  <div
                    className={`form-group ${
                      hasEmailWhitelist ? 'v2board-email-whitelist-enable' : ''
                    }`}
                  >
                    <input
                      type="text"
                      className="form-control form-control-alt"
                      placeholder={t('auth.email')}
                      value={email}
                      onChange={(event) => setEmail(event.target.value)}
                    />
                    {hasEmailWhitelist ? (
                      <select
                        className="form-control form-control-alt"
                        value={emailSuffix}
                        onChange={(event) => setEmailSuffix(event.target.value)}
                      >
                        {emailSuffixes.map((suffix) => (
                          <option key={suffix} value={suffix}>
                            @{suffix}
                          </option>
                        ))}
                      </select>
                    ) : null}
                  </div>

                  {config?.is_email_verify ? (
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
                          className="btn btn-block btn-primary font-w400"
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
                  ) : null}

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
                  <div className="form-group">
                    <input
                      type="text"
                      disabled={Boolean(initialInviteCode)}
                      className="form-control form-control-alt"
                      placeholder={
                        config?.is_invite_force ? t('auth.invite_code') : t('auth.invite_code_optional')
                      }
                      value={inviteCode}
                      onChange={(event) => setInviteCode(event.target.value)}
                    />
                  </div>

                  {config?.tos_url ? (
                    <div className="form-group">
                      <div className="custom-control custom-checkbox custom-control-primary">
                        <input
                          type="checkbox"
                          className="custom-control-input"
                          checked={tosChecked}
                          style={{ zIndex: 1000 }}
                          onClick={() => setTosChecked((value) => !value)}
                          onChange={() => undefined}
                        />
                        <label className="custom-control-label">
                          <div
                            dangerouslySetInnerHTML={{
                              __html: t('auth.tos_html').replace('{url}', config.tos_url),
                            }}
                          />
                        </label>
                      </div>
                    </div>
                  ) : null}

                  <div className="form-group mb-0">
                    <button
                      disabled={isPending || Boolean(config?.tos_url && !tosChecked)}
                      type="submit"
                      className="btn btn-block btn-primary font-w400"
                      onClick={() => runRecaptcha(onRegister)}
                    >
                      {isPending ? (
                        <LegacyLoadingIcon />
                      ) : (
                        <span>
                          <i className="si si-emoticon-smile mr-1" />
                          {t('auth.submit_register')}
                        </span>
                      )}
                    </button>
                  </div>
                </div>
              )}
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
