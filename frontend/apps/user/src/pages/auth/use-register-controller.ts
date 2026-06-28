import {
  useCallback,
  useEffect,
  useState,
  useRef,
  type BaseSyntheticEvent,
  type ReactNode,
} from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm, type UseFormRegister } from 'react-hook-form';
import { z } from 'zod';
import {
  useGuestConfig,
  useRegisterMutation,
  useSendEmailVerifyMutation,
} from '@/lib/guest';
import { authToast } from '@/lib/auth-toast';
import { i18nGet } from '@/lib/errors';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { useAuthRecaptcha } from './auth-recaptcha';

const registerSchema = z
  .object({
    email: z.string(),
    email_code: z.string().optional(),
    password: z.string(),
    confirm_password: z.string(),
    invite_code: z.string().optional(),
  })
  .superRefine((values, context) => {
    if (values.password !== values.confirm_password) {
      context.addIssue({
        code: 'custom',
        path: ['confirm_password'],
        message: 'password_mismatch',
      });
    }
  });

type RegisterFormValues = z.infer<typeof registerSchema>;

export interface RegisterController {
  config: ReturnType<typeof useGuestConfig>['data'];
  configLoading: boolean;
  registerInput: UseFormRegister<RegisterFormValues>;
  submit: (event?: BaseSyntheticEvent) => Promise<void>;
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
  const form = useForm<RegisterFormValues>({
    resolver: zodResolver(registerSchema),
    defaultValues: {
      email: '',
      email_code: '',
      password: '',
      confirm_password: '',
      invite_code: initialInviteCode ?? '',
    },
  });

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
  const getEmail = (email: string) => {
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
        email: getEmail(form.getValues('email')),
        isforget: 0,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (!sent) return;
      authToast.success(t('auth.email_code_sent_title'), {
        description: t('auth.email_code_sent_description'),
      });
      startSendEmailVerifyCountdown();
    } catch {}
  };

  const onRegister = async (values: RegisterFormValues, recaptchaData?: string) => {
    if (config?.tos_url && !tosChecked) {
      authToast.error(i18nGet('请求失败'), { description: t('auth.tos_required') });
      return;
    }
    try {
      await register({
        email: getEmail(values.email),
        password: values.password,
        invite_code:
          values.invite_code || initialInviteCode || '',
        email_code: config?.is_email_verify ? values.email_code ?? '' : '',
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (mountedRef.current) navigate('/login');
    } catch {}
  };

  const submit = form.handleSubmit(
    (values) => runRecaptcha((recaptchaData) => onRegister(values, recaptchaData)),
    () => authToast.error(i18nGet('请求失败'), { description: t('auth.password_mismatch') }),
  );

  const sendCode = () => runRecaptcha(onSendCode);

  return {
    config,
    configLoading,
    registerInput: form.register,
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
