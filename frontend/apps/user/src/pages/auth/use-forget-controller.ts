import {
  useCallback,
  useEffect,
  useRef,
  type BaseSyntheticEvent,
  type ReactNode,
} from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm, type UseFormRegister } from 'react-hook-form';
import { z } from 'zod';
import { useForgetMutation, useGuestConfig, useSendEmailVerifyMutation } from '@/lib/guest';
import { authToast } from '@/lib/auth-toast';
import { i18nGet } from '@/lib/errors';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { useAuthRecaptcha } from './auth-recaptcha';
import { useCountdown } from './use-countdown';

const forgetSchema = z
  .object({
    email: z.string(),
    email_code: z.string(),
    password: z.string(),
    confirm_password: z.string(),
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

type ForgetFormValues = z.infer<typeof forgetSchema>;

export interface ForgetController {
  configLoading: boolean;
  registerInput: UseFormRegister<ForgetFormValues>;
  submit: (event?: BaseSyntheticEvent) => Promise<void>;
  /** Send-code button handler — runs recaptcha then the email-verify flow. */
  sendCode: () => void;
  /** True once the confirm-password superRefine flags a mismatch (drives the inline field error). */
  passwordMismatch: boolean;
  isPending: boolean;
  isSendingCode: boolean;
  cooldownActive: boolean;
  cooldownRemaining: number;
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
  const form = useForm<ForgetFormValues>({
    resolver: zodResolver(forgetSchema),
    defaultValues: {
      email: '',
      email_code: '',
      password: '',
      confirm_password: '',
    },
  });

  const mountedRef = useRef(true);
  const cooldown = useCountdown(60);

  useEffect(() => {
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const startSendEmailVerifyCountdown = useCallback(() => {
    if (!mountedRef.current) return;
    cooldown.start();
  }, [cooldown]);

  const onSendCode = async (recaptchaData?: string) => {
    try {
      const sent = await sendCodeMutation({
        email: form.getValues('email'),
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

  const onForget = async (values: ForgetFormValues) => {
    try {
      await forget({
        email: values.email,
        password: values.password,
        email_code: values.email_code,
      });
      if (mountedRef.current) navigate('/login');
    } catch {}
  };

  const submit = form.handleSubmit(
    onForget,
    () => authToast.error(i18nGet('请求失败'), { description: t('auth.password_mismatch') }),
  );

  const sendCode = () => runRecaptcha(onSendCode);

  // Read the proxied error here so the controller re-renders when the confirm-password
  // superRefine toggles; the inline field error mirrors the existing mismatch toast.
  const passwordMismatch = Boolean(form.formState.errors.confirm_password);

  return {
    configLoading,
    registerInput: form.register,
    submit,
    sendCode,
    passwordMismatch,
    isPending,
    isSendingCode,
    cooldownActive: cooldown.isActive,
    cooldownRemaining: cooldown.remaining,
    recaptchaModal,
  };
}
