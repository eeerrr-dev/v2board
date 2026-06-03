import { useEffect, useRef, useState } from 'react';
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
import { legacyHref } from '@/lib/legacy-href';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';

export default function RegisterPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const guestConfig = useGuestConfig();
  const { data: config } = guestConfig;
  const configLoading = useLegacyFetchLoading(guestConfig.isFetching);
  const { mutateAsync: register, isPending } = useRegisterMutation();
  const { mutateAsync: sendCode, isPending: isSendingCode } = useSendEmailVerifyMutation();
  const { run: runRecaptcha, recaptchaModal } = useLegacyRecaptcha(
    Boolean(config?.is_recaptcha),
    config?.recaptcha_site_key,
  );

  const initialInviteCode = params.get('code');
  const emailRef = useRef<HTMLInputElement | null>(null);
  const emailCodeRef = useRef<HTMLInputElement | null>(null);
  const passwordRef = useRef<HTMLInputElement | null>(null);
  const confirmPasswordRef = useRef<HTMLInputElement | null>(null);
  const inviteCodeRef = useRef<HTMLInputElement | null>(null);
  const cooldownRef = useRef(60);
  const [emailSuffix, setEmailSuffix] = useState<string | undefined>(undefined);
  const [tosChecked, setTosChecked] = useState(false);
  const [cooldown, setCooldown] = useState(60);
  const logo = getLegacyLogo();
  const title = getLegacyTitle();
  const description = getLegacyDescription();
  const emailWhitelistSuffix = config?.email_whitelist_suffix;
  const emailSuffixes = Array.isArray(emailWhitelistSuffix) ? emailWhitelistSuffix : [];
  const hasEmailWhitelist = Boolean(emailWhitelistSuffix);
  const selectedEmailSuffix = hasEmailWhitelist
    ? emailSuffixes.includes(emailSuffix ?? '')
      ? emailSuffix
      : emailSuffixes[0]
    : '';
  const getEmail = () => {
    const email = emailRef.current!.value;
    return hasEmailWhitelist ? `${email}@${selectedEmailSuffix}` : email;
  };

  const startSendEmailVerifyCountdown = () => {
    window.setTimeout(() => {
      if (cooldownRef.current !== 0) {
        cooldownRef.current -= 1;
        setCooldown(cooldownRef.current);
        startSendEmailVerifyCountdown();
      } else {
        cooldownRef.current = 60;
        setCooldown(60);
      }
    }, 1000);
  };

  useEffect(() => {
    if (!hasEmailWhitelist) {
      if (emailSuffix !== '') setEmailSuffix('');
      return;
    }
    if (emailSuffix !== selectedEmailSuffix) setEmailSuffix(selectedEmailSuffix);
  }, [emailSuffix, hasEmailWhitelist, selectedEmailSuffix]);

  const onSendCode = async (recaptchaData?: string) => {
    try {
      const sent = await sendCode({
        email: getEmail(),
        isforget: 0,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (!sent) return;
      toast.success('发送成功', { description: '如果没有收到验证码请检查垃圾箱。' });
      startSendEmailVerifyCountdown();
    } catch {}
  };

  const onRegister = async (recaptchaData?: string) => {
    if (config?.tos_url && !tosChecked) {
      toast.error(i18nGet('请求失败'), { description: t('auth.tos_required') });
      return;
    }
    const password = passwordRef.current!.value;
    if (password !== confirmPasswordRef.current!.value) {
      toast.error(i18nGet('请求失败'), { description: t('auth.password_mismatch') });
      return;
    }
    try {
      await register({
        email: getEmail(),
        password,
        invite_code: inviteCodeRef.current!.value,
        email_code: config?.is_email_verify ? emailCodeRef.current!.value : '',
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
                <a className="font-size-h1" ref={legacyHref()}>
                  {logo ? (
                    <img className="v2board-logo mb-3" src={logo} />
                  ) : (
                    <span className="text-dark">{title || 'V2Board'}</span>
                  )}
                </a>
                {description && <p className="font-size-sm text-muted mb-3">{description}</p>}
              </div>
              {configLoading ? (
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
                      ref={emailRef}
                    />
                    {hasEmailWhitelist ? (
                      <select
                        className="form-control form-control-alt"
                        value={selectedEmailSuffix}
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
                          ref={emailCodeRef}
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
                      ref={passwordRef}
                    />
                  </div>
                  <div className="form-group">
                    <input
                      type="password"
                      className="form-control form-control-alt"
                      placeholder={t('auth.password')}
                      ref={confirmPasswordRef}
                    />
                  </div>
                  <div className="form-group">
                    <input
                      type="text"
                      disabled={Boolean(initialInviteCode)}
                      defaultValue={initialInviteCode ?? undefined}
                      className="form-control form-control-alt"
                      placeholder={
                        config?.is_invite_force
                          ? t('auth.invite_code')
                          : t('auth.invite_code_optional')
                      }
                      ref={inviteCodeRef}
                    />
                  </div>

                  {config?.tos_url ? (
                    <div className="form-group">
                      <div className="custom-control custom-checkbox custom-control-primary">
                        {/* Original wires only onClick (umi.js @1339000) — a controlled
                            checkbox with no onChange and no readOnly, so it emits React's
                            dev-only warning; match its DOM exactly (no extra attributes). */}
                        <input
                          type="checkbox"
                          className="custom-control-input"
                          checked={tosChecked}
                          style={{ zIndex: 1000 }}
                          onClick={() => setTosChecked((value) => !value)}
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
          </div>
        </div>
        <div className="text-left bg-gray-lighter p-3 px-4">
          <a
            className="font-size-sm text-muted"
            ref={legacyHref()}
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
      {recaptchaModal}
    </>
  );
}
