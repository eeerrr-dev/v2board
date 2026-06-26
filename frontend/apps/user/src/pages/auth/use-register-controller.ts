import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type SyntheticEvent,
  type ReactNode,
  type RefObject,
} from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  useGuestConfig,
  useRegisterMutation,
  useSendEmailVerifyMutation,
} from '@/lib/guest';
import { authToast } from '@/lib/auth-toast';
import { i18nGet } from '@/lib/errors';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { useAuthRecaptcha } from './auth-recaptcha';

function readFormValue(form: HTMLFormElement | null, name: string) {
  if (!form) return '';
  return String(new FormData(form).get(name) ?? '');
}

export interface RegisterController {
  config: ReturnType<typeof useGuestConfig>['data'];
  configLoading: boolean;
  formRef: RefObject<HTMLFormElement | null>;
  /** Form submit handler — runs recaptcha then the register flow. */
  submit: (event: SyntheticEvent<HTMLFormElement>) => void;
  /** Send-code button handler — runs recaptcha then the email-verify flow. */
  sendCode: () => void;
  isPending: boolean;
  isSendingCode: boolean;
  cooldown: number;
  hasEmailWhitelist: boolean;
  emailSuffixes: string[];
  selectedEmailSuffix: string | undefined;
  setEmailSuffix: (value: string) => void;
  tosChecked: boolean;
  setTosChecked: (updater: (value: boolean) => boolean) => void;
  initialInviteCode: string | null;
  recaptchaModal: ReactNode;
}

// Authored V2Board — register behavior controller. Mirrors the login surface's controller/view split:
// the page is a thin presentation layer, all mutations / recaptcha orchestration / countdown /
// validation / navigation live here. The request payloads and toast contract remain legacy-compatible,
// while the countdown is now a normal React side effect with unmount cleanup.
export function useRegisterController(): RegisterController {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const guestConfig = useGuestConfig();
  const { data: config } = guestConfig;
  const configLoading = useLegacyFetchLoading(guestConfig.isFetching);
  const { mutateAsync: register, isPending } = useRegisterMutation();
  const { mutateAsync: sendCodeMutation, isPending: isSendingCode } = useSendEmailVerifyMutation();
  const { run: runRecaptcha, recaptchaModal } = useAuthRecaptcha(
    Boolean(config?.is_recaptcha),
    config?.recaptcha_site_key,
  );

  const initialInviteCode = params.get('code');
  const formRef = useRef<HTMLFormElement | null>(null);
  const mountedRef = useRef(true);
  const [emailSuffix, setEmailSuffix] = useState<string | undefined>(undefined);
  const [tosChecked, setTosChecked] = useState(false);
  const [cooldown, setCooldown] = useState(60);
  const emailWhitelistSuffix = config?.email_whitelist_suffix;
  const emailSuffixes = Array.isArray(emailWhitelistSuffix) ? emailWhitelistSuffix : [];
  const hasEmailWhitelist = emailSuffixes.length > 0;
  const selectedEmailSuffix = hasEmailWhitelist
    ? emailSuffixes.includes(emailSuffix ?? '')
      ? emailSuffix
      : (emailSuffixes[0] ?? '')
    : '';
  const getEmail = () => {
    const email = readFormValue(formRef.current, 'email');
    return hasEmailWhitelist ? `${email}@${selectedEmailSuffix}` : email;
  };

  useEffect(() => {
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (cooldown === 60) return undefined;

    const timer = window.setTimeout(() => {
      setCooldown((value) => (value <= 1 ? 60 : value - 1));
    }, 1000);

    return () => window.clearTimeout(timer);
  }, [cooldown]);

  const startSendEmailVerifyCountdown = useCallback(() => {
    if (!mountedRef.current) return;
    setCooldown(59);
  }, []);

  const onSendCode = async (recaptchaData?: string) => {
    try {
      const sent = await sendCodeMutation({
        email: getEmail(),
        isforget: 0,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (!sent) return;
      authToast.success('发送成功', { description: '如果没有收到验证码请检查垃圾箱。' });
      startSendEmailVerifyCountdown();
    } catch {}
  };

  const onRegister = async (recaptchaData?: string) => {
    if (config?.tos_url && !tosChecked) {
      authToast.error(i18nGet('请求失败'), { description: t('auth.tos_required') });
      return;
    }
    const password = readFormValue(formRef.current, 'password');
    if (password !== readFormValue(formRef.current, 'confirm_password')) {
      authToast.error(i18nGet('请求失败'), { description: t('auth.password_mismatch') });
      return;
    }
    try {
      await register({
        email: getEmail(),
        password,
        invite_code:
          readFormValue(formRef.current, 'invite_code') || initialInviteCode || '',
        email_code: config?.is_email_verify ? readFormValue(formRef.current, 'email_code') : '',
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (mountedRef.current) navigate('/login');
    } catch {}
  };

  const submit = (event: SyntheticEvent<HTMLFormElement>) => {
    event.preventDefault();
    runRecaptcha(onRegister);
  };

  const sendCode = () => runRecaptcha(onSendCode);

  return {
    config,
    configLoading,
    formRef,
    submit,
    sendCode,
    isPending,
    isSendingCode,
    cooldown,
    hasEmailWhitelist,
    emailSuffixes,
    selectedEmailSuffix,
    setEmailSuffix,
    tosChecked,
    setTosChecked,
    initialInviteCode,
    recaptchaModal,
  };
}
