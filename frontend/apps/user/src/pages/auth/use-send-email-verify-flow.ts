import { useTranslation } from 'react-i18next';
import { useSendEmailVerifyMutation } from '@/lib/guest';
import { toast } from '@/lib/toast';
import { useCountdown } from './use-countdown';

interface SendEmailVerifyFlowOptions {
  /** The backend `is_forget` flag: false for register, true for forget-password. */
  isForget: boolean;
  /** Runs the action behind the recaptcha gate (no-op gate when disabled). */
  runRecaptcha: (action: (recaptchaData?: string) => void | Promise<void>) => void;
}

export interface SendEmailVerifyFlow {
  /** Send-code button handler — runs recaptcha, then the email-verify mutation. */
  sendCode: (email: string) => void;
  isSendingCode: boolean;
  cooldownActive: boolean;
  cooldownRemaining: number;
}

// Authored V2Board — shared send-code + cooldown flow for the register and forget
// surfaces. Both previously duplicated this verbatim: recaptcha-gated send, success
// toast, then a 60-second cooldown. The countdown owns its own timer cleanup
// (useCountdown), so no mountedRef guard is needed here — calling `start()` after
// unmount is a harmless no-op. The post-submit navigation guard still lives in each
// controller, which is the only place that work outlives this flow.
export function useSendEmailVerifyFlow({
  isForget,
  runRecaptcha,
}: SendEmailVerifyFlowOptions): SendEmailVerifyFlow {
  const { t } = useTranslation();
  const { mutate: sendCodeMutation, isPending: isSendingCode } = useSendEmailVerifyMutation();
  const cooldown = useCountdown(60);

  const onSendCode = (email: string, recaptchaData?: string) => {
    sendCodeMutation(
      {
        email,
        is_forget: isForget,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      },
      {
        // POST /auth/email-codes succeeds as a bodiless 204; reaching
        // onSuccess is the success signal itself.
        onSuccess: () => {
          toast.success(
            t(($) => $.auth.email_code_sent_title),
            {
              description: t(($) => $.auth.email_code_sent_description),
            },
          );
          cooldown.start();
        },
      },
    );
  };

  // Capture the schema-validated email before opening recaptcha. Reading the
  // live input only after the challenge would let edits made while it is open
  // bypass the validation that preceded the action.
  const sendCode = (email: string) => {
    runRecaptcha((recaptchaData) => onSendCode(email, recaptchaData));
  };

  return {
    sendCode,
    isSendingCode,
    cooldownActive: cooldown.isActive,
    cooldownRemaining: cooldown.remaining,
  };
}
