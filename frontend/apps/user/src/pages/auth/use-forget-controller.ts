import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type SyntheticEvent,
  type ReactNode,
  type RefObject,
} from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useForgetMutation, useGuestConfig, useSendEmailVerifyMutation } from '@/lib/guest';
import { authToast } from '@/lib/auth-toast';
import { i18nGet } from '@/lib/errors';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { useAuthRecaptcha } from './auth-recaptcha';

function readFormValue(form: HTMLFormElement | null, name: string) {
  if (!form) return '';
  return String(new FormData(form).get(name) ?? '');
}

export interface ForgetController {
  configLoading: boolean;
  formRef: RefObject<HTMLFormElement | null>;
  /** Form submit handler — runs the reset flow. */
  submit: (event: SyntheticEvent<HTMLFormElement>) => void;
  /** Send-code button handler — runs recaptcha then the email-verify flow. */
  sendCode: () => void;
  isPending: boolean;
  isSendingCode: boolean;
  cooldown: number;
  recaptchaModal: ReactNode;
}

// Authored V2Board — forget-password behavior controller. Mirrors the login/register controller split
// so all three auth surfaces share one architecture: the page is a thin view, mutations / recaptcha /
// countdown / validation / navigation live here. Aligned with register on two points the forget page
// previously diverged on: validation errors route through i18nGet/t (no raw Chinese literals), and a
// guest-config loading guard gates the form so recaptcha is never treated as disabled mid-fetch. The
// payload contract is unchanged; the countdown is now a normal React side effect with unmount cleanup.
export function useForgetController(): ForgetController {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const guestConfig = useGuestConfig();
  const { data: config } = guestConfig;
  const configLoading = useLegacyFetchLoading(guestConfig.isFetching);
  const { mutateAsync: forget, isPending } = useForgetMutation();
  const { mutateAsync: sendCodeMutation, isPending: isSendingCode } = useSendEmailVerifyMutation();
  const { run: runRecaptcha, recaptchaModal } = useAuthRecaptcha(
    Boolean(config?.is_recaptcha),
    config?.recaptcha_site_key,
  );

  const formRef = useRef<HTMLFormElement | null>(null);
  const mountedRef = useRef(true);
  const [cooldown, setCooldown] = useState(60);

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
        email: readFormValue(formRef.current, 'email'),
        isforget: 1,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (!sent) return;
      authToast.success(t('auth.email_code_sent_title'), {
        description: t('auth.email_code_sent_description'),
      });
      startSendEmailVerifyCountdown();
    } catch {}
  };

  const onForget = async () => {
    const password = readFormValue(formRef.current, 'password');
    if (password !== readFormValue(formRef.current, 'confirm_password')) {
      authToast.error(i18nGet('请求失败'), { description: t('auth.password_mismatch') });
      return;
    }
    try {
      await forget({
        email: readFormValue(formRef.current, 'email'),
        password,
        email_code: readFormValue(formRef.current, 'email_code'),
      });
      if (mountedRef.current) navigate('/login');
    } catch {}
  };

  const submit = (event: SyntheticEvent<HTMLFormElement>) => {
    event.preventDefault();
    void onForget();
  };

  const sendCode = () => runRecaptcha(onSendCode);

  return {
    configLoading,
    formRef,
    submit,
    sendCode,
    isPending,
    isSendingCode,
    cooldown,
    recaptchaModal,
  };
}
