import {
  type BaseSyntheticEvent,
  type ReactNode,
} from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm, type UseFormRegister } from 'react-hook-form';
import { z } from 'zod';
import { useForgetMutation, useGuestConfig } from '@/lib/guest';
import { toast } from '@/lib/toast';
import { i18nGet } from '@/lib/errors';
import { useAuthRecaptcha } from './auth-recaptcha';
import { makeConfirmPasswordRefinement } from './refine-confirm-password';
import { useSendEmailVerifyFlow } from './use-send-email-verify-flow';

const forgetSchema = z
  .object({
    email: z.string(),
    email_code: z.string(),
    password: z.string(),
    confirm_password: z.string(),
  })
  .superRefine(makeConfirmPasswordRefinement({ passwordKey: 'password', confirmKey: 'confirm_password' }));

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

// Authored V2Board — forget-password behavior controller. Mirrors the register controller's split:
// the page is a thin view; mutations / recaptcha / validation / navigation live here, and the
// recaptcha-gated send-code + 60-second cooldown is shared with register via useSendEmailVerifyFlow.
// The payload contract is unchanged.
export function useForgetController(): ForgetController {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const guestConfig = useGuestConfig();
  const { data: config } = guestConfig;
  // Only show the full-form spinner on the true initial load; isFetching would
  // re-flash it on every background refetch (staleTime 0 + refetchOnMount).
  const configLoading = guestConfig.isLoading;
  const { mutateAsync: forget, isPending } = useForgetMutation();
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

  const { sendCode, isSendingCode, cooldownActive, cooldownRemaining } = useSendEmailVerifyFlow({
    isforget: 1,
    getEmail: () => form.getValues('email'),
    runRecaptcha,
  });

  const onForget = async (values: ForgetFormValues) => {
    try {
      await forget({
        email: values.email,
        password: values.password,
        email_code: values.email_code,
      });
      navigate('/login');
    } catch {}
  };

  const submit = form.handleSubmit(
    onForget,
    () => toast.error(i18nGet('请求失败'), { description: t('auth.password_mismatch') }),
  );

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
    cooldownActive,
    cooldownRemaining,
    recaptchaModal,
  };
}
