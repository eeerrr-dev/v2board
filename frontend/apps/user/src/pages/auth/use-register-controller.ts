import { useState, type BaseSyntheticEvent, type ReactNode } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm, useFormState, type UseFormRegister } from 'react-hook-form';
import { useGuestConfig, useRegisterMutation } from '@/lib/guest';
import { toast } from '@/lib/toast';
import { i18nGet } from '@/lib/errors';
import { useAuthRecaptcha } from './auth-recaptcha';
import {
  authEmailSchema,
  createRegisterSchema,
  type RegisterFormInput,
  type RegisterFormValues,
} from './auth-validation';
import { useSendEmailVerifyFlow } from './use-send-email-verify-flow';

export interface RegisterController {
  config: ReturnType<typeof useGuestConfig>['data'];
  configLoading: boolean;
  configError: boolean;
  retryConfig: () => void;
  registerInput: UseFormRegister<RegisterFormInput>;
  submit: (event?: BaseSyntheticEvent) => Promise<void>;
  /** Validates email, then runs recaptcha and the email-verify flow. */
  sendCode: () => Promise<void>;
  errors: {
    email?: string;
    emailCode?: string;
    password?: string;
    confirmPassword?: string;
    inviteCode?: string;
  };
  isPending: boolean;
  isSendingCode: boolean;
  cooldownActive: boolean;
  cooldownRemaining: number;
  hasEmailWhitelist: boolean;
  emailSuffixes: string[];
  selectedEmailSuffix: string | undefined;
  setEmailSuffix: (value: string) => void;
  tosChecked: boolean;
  setTosChecked: (updater: (value: boolean) => boolean) => void;
  initialInviteCode: string | null;
  recaptchaModal: ReactNode;
}

// Authored V2Board — register behavior controller. The page is a thin presentation layer; all
// mutations / recaptcha orchestration / validation / navigation live here. The recaptcha-gated
// send-code + 60-second cooldown is shared with the forget surface via useSendEmailVerifyFlow, so
// this controller only owns register-specific concerns (TOS gating, whitelist suffix, invite code).
// The request payloads and toast behavior remain aligned with the backend contract.
export function useRegisterController(): RegisterController {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const guestConfig = useGuestConfig();
  const { data: config } = guestConfig;
  // Only show the full-form spinner on the true initial load; isFetching would
  // re-flash it on every background refetch (staleTime 0 + refetchOnMount).
  const configLoading = guestConfig.isLoading;
  // Registration policy is security-sensitive: an unavailable guest config must
  // never be interpreted as every server-side gate being disabled. A successful
  // query with a concrete payload is the only state in which actions may run.
  const configReady = guestConfig.isSuccess && config !== undefined;
  const configError = !configLoading && !configReady;
  const retryConfig = () => {
    void guestConfig.refetch();
  };
  const { mutate: register, isPending } = useRegisterMutation();
  const { run: runRecaptcha, recaptchaModal } = useAuthRecaptcha(
    Boolean(config?.is_recaptcha),
    config?.recaptcha_site_key,
  );
  const initialInviteCode = params.get('code');
  const [emailSuffix, setEmailSuffix] = useState<string | undefined>(undefined);
  const [tosChecked, setTosChecked] = useState(false);
  const emailWhitelistSuffix = config?.email_whitelist_suffix;
  const emailSuffixes = Array.isArray(emailWhitelistSuffix) ? emailWhitelistSuffix : [];
  const hasEmailWhitelist = emailSuffixes.length > 0;
  const selectedEmailSuffix = hasEmailWhitelist
    ? emailSuffixes.includes(emailSuffix ?? '')
      ? emailSuffix
      : (emailSuffixes[0] ?? '')
    : '';
  const registerSchema = createRegisterSchema({
    emailSuffix: hasEmailWhitelist ? selectedEmailSuffix : undefined,
    emailCodeRequired: Boolean(config?.is_email_verify),
    inviteCodeRequired: Boolean(config?.is_invite_force),
  });
  const form = useForm<RegisterFormInput, unknown, RegisterFormValues>({
    resolver: zodResolver(registerSchema),
    defaultValues: {
      email: '',
      email_code: '',
      password: '',
      confirm_password: '',
      invite_code: initialInviteCode ?? '',
    },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler caches
  // proxy reads, which would freeze these derived errors after the first render.
  const { errors } = useFormState({ control: form.control });

  const getEmail = (email: string) => {
    const normalized = email.trim();
    return hasEmailWhitelist ? `${normalized}@${selectedEmailSuffix}` : normalized;
  };

  const {
    sendCode: sendCodeWithConfig,
    isSendingCode,
    cooldownActive,
    cooldownRemaining,
  } = useSendEmailVerifyFlow({
    isForget: false,
    runRecaptcha,
  });

  const sendCode = async () => {
    if (!configReady || !config?.is_email_verify || isSendingCode || cooldownActive) return;
    const emailValid = await form.trigger('email', { shouldFocus: true });
    if (!emailValid) return;
    const email = authEmailSchema.safeParse(getEmail(form.getValues('email')));
    if (!email.success) return;
    sendCodeWithConfig(email.data);
  };

  const onRegister = (values: RegisterFormValues, recaptchaData?: string) => {
    // Keep the mutation guarded independently from the view. This also prevents
    // a stale/detached form submission from bypassing the fail-closed policy.
    if (!configReady) return;
    const validated = registerSchema.safeParse(values);
    if (!validated.success) return;
    const formValues = validated.data;
    if (config?.tos_url && !tosChecked) {
      toast.error(i18nGet('请求失败'), { description: t(($) => $.auth.tos_required) });
      return;
    }
    const inviteCode = formValues.invite_code || initialInviteCode || '';
    if (config?.is_invite_force && !inviteCode) {
      toast.error(i18nGet('请求失败'), { description: t(($) => $.auth.invite_code_required) });
      return;
    }
    register(
      {
        email: getEmail(formValues.email),
        password: formValues.password,
        invite_code: inviteCode,
        email_code: config?.is_email_verify ? formValues.email_code : '',
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      },
      { onSuccess: () => void navigate('/login') },
    );
  };

  const submitForm = form.handleSubmit((values) =>
    runRecaptcha((recaptchaData) => onRegister(values, recaptchaData)),
  );
  const submit = async (event?: BaseSyntheticEvent) => {
    if (!configReady) {
      event?.preventDefault();
      return;
    }
    await submitForm(event);
  };

  return {
    config,
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
      inviteCode: errors.invite_code?.message,
    },
    isPending,
    isSendingCode,
    cooldownActive,
    cooldownRemaining,
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
