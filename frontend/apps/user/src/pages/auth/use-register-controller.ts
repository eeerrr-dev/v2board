import {
  useState,
  type BaseSyntheticEvent,
  type ReactNode,
} from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm, type UseFormRegister } from 'react-hook-form';
import { z } from 'zod';
import { useGuestConfig, useRegisterMutation } from '@/lib/guest';
import { toast } from '@/lib/toast';
import { i18nGet } from '@/lib/errors';
import { useAuthRecaptcha } from './auth-recaptcha';
import { makeConfirmPasswordRefinement } from './refine-confirm-password';
import { useSendEmailVerifyFlow } from './use-send-email-verify-flow';

const registerSchema = z
  .object({
    email: z.string(),
    email_code: z.string().optional(),
    password: z.string(),
    confirm_password: z.string(),
    invite_code: z.string().optional(),
  })
  .superRefine(makeConfirmPasswordRefinement({ passwordKey: 'password', confirmKey: 'confirm_password' }));

type RegisterFormValues = z.infer<typeof registerSchema>;

export interface RegisterController {
  config: ReturnType<typeof useGuestConfig>['data'];
  configLoading: boolean;
  registerInput: UseFormRegister<RegisterFormValues>;
  submit: (event?: BaseSyntheticEvent) => Promise<void>;
  /** Send-code button handler — runs recaptcha then the email-verify flow. */
  sendCode: () => void;
  /** True once the confirm-password superRefine flags a mismatch (drives the inline field error). */
  passwordMismatch: boolean;
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
// The request payloads and toast contract remain legacy-compatible.
export function useRegisterController(): RegisterController {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const guestConfig = useGuestConfig();
  const { data: config } = guestConfig;
  const configLoading = guestConfig.isFetching;
  const { mutateAsync: register, isPending } = useRegisterMutation();
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
  const getEmail = (email: string) => {
    return hasEmailWhitelist ? `${email}@${selectedEmailSuffix}` : email;
  };

  const { sendCode, isSendingCode, cooldownActive, cooldownRemaining } = useSendEmailVerifyFlow({
    isforget: 0,
    getEmail: () => getEmail(form.getValues('email')),
    runRecaptcha,
  });

  const onRegister = async (values: RegisterFormValues, recaptchaData?: string) => {
    if (config?.tos_url && !tosChecked) {
      toast.error(i18nGet('请求失败'), { description: t('auth.tos_required') });
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
      navigate('/login');
    } catch {}
  };

  const submit = form.handleSubmit(
    (values) => runRecaptcha((recaptchaData) => onRegister(values, recaptchaData)),
    () => toast.error(i18nGet('请求失败'), { description: t('auth.password_mismatch') }),
  );

  // Read the proxied error here so the controller re-renders when the confirm-password
  // superRefine toggles; the inline field error mirrors the existing mismatch toast.
  const passwordMismatch = Boolean(form.formState.errors.confirm_password);

  return {
    config,
    configLoading,
    registerInput: form.register,
    submit,
    sendCode,
    passwordMismatch,
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
