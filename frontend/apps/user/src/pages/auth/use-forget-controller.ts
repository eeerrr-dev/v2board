import { type BaseSyntheticEvent, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm, useFormState, type UseFormRegister } from 'react-hook-form';
import { useForgetMutation, useGuestConfig } from '@/lib/guest';
import { useAuthRecaptcha } from './auth-recaptcha';
import {
  authEmailSchema,
  forgetSchema,
  type ForgetFormInput,
  type ForgetFormValues,
} from './auth-validation';
import { useSendEmailVerifyFlow } from './use-send-email-verify-flow';

export interface ForgetController {
  configLoading: boolean;
  configError: boolean;
  retryConfig: () => void;
  registerInput: UseFormRegister<ForgetFormInput>;
  submit: (event?: BaseSyntheticEvent) => Promise<void>;
  /** Validates email, then runs recaptcha and the email-verify flow. */
  sendCode: () => Promise<void>;
  errors: {
    email?: string;
    emailCode?: string;
    password?: string;
    confirmPassword?: string;
  };
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
  const navigate = useNavigate();
  const guestConfig = useGuestConfig();
  const { data: config } = guestConfig;
  // Only show the full-form spinner on the true initial load; isFetching would
  // re-flash it on every background refetch (staleTime 0 + refetchOnMount).
  const configLoading = guestConfig.isLoading;
  // Password-reset policy (notably recaptcha) is unknown until guest config has
  // loaded successfully. Treat every other settled state as unavailable.
  const configReady = guestConfig.isSuccess && config !== undefined;
  const configError = !configLoading && !configReady;
  const retryConfig = () => {
    void guestConfig.refetch();
  };
  const { mutate: forget, isPending } = useForgetMutation();
  const { run: runRecaptcha, recaptchaModal } = useAuthRecaptcha(
    Boolean(config?.is_recaptcha),
    config?.recaptcha_site_key,
  );
  const form = useForm<ForgetFormInput, unknown, ForgetFormValues>({
    resolver: zodResolver(forgetSchema),
    defaultValues: {
      email: '',
      email_code: '',
      password: '',
      confirm_password: '',
    },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler caches
  // proxy reads, which would freeze these derived errors after the first render.
  const { errors } = useFormState({ control: form.control });

  const {
    sendCode: sendCodeWithConfig,
    isSendingCode,
    cooldownActive,
    cooldownRemaining,
  } = useSendEmailVerifyFlow({
    isForget: true,
    runRecaptcha,
  });

  const sendCode = async () => {
    if (!configReady || isSendingCode || cooldownActive) return;
    const emailValid = await form.trigger('email', { shouldFocus: true });
    if (!emailValid) return;
    const email = authEmailSchema.safeParse(form.getValues('email'));
    if (!email.success) return;
    sendCodeWithConfig(email.data);
  };

  const onForget = (values: ForgetFormValues) => {
    if (!configReady) return;
    const validated = forgetSchema.safeParse(values);
    if (!validated.success) return;
    forget(
      {
        email: validated.data.email,
        password: validated.data.password,
        email_code: validated.data.email_code,
      },
      { onSuccess: () => void navigate('/login') },
    );
  };

  const submitForm = form.handleSubmit(onForget);
  const submit = async (event?: BaseSyntheticEvent) => {
    if (!configReady) {
      event?.preventDefault();
      return;
    }
    await submitForm(event);
  };

  return {
    configLoading,
    configError,
    retryConfig,
    registerInput: form.register,
    submit,
    sendCode,
    errors: {
      email: errors.email?.message,
      emailCode: errors.email_code?.message,
      password: errors.password?.message,
      confirmPassword: errors.confirm_password?.message,
    },
    isPending,
    isSendingCode,
    cooldownActive,
    cooldownRemaining,
    recaptchaModal,
  };
}
