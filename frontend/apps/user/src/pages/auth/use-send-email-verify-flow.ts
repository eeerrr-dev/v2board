import { useTranslation } from 'react-i18next';
import { useSendEmailVerifyMutation } from '@/lib/guest';
import { toast } from '@/lib/toast';
import { useCountdown } from './use-countdown';

interface SendEmailVerifyFlowOptions {
  /** The legacy `isforget` flag: 0 for register, 1 for forget-password. */
  isforget: 0 | 1;
  /** Resolves the address to send to (register wraps the whitelist suffix). */
  getEmail: () => string;
  /** Runs the action behind the recaptcha gate (no-op gate when disabled). */
  runRecaptcha: (action: (recaptchaData?: string) => void | Promise<void>) => void;
}

export interface SendEmailVerifyFlow {
  /** Send-code button handler — runs recaptcha, then the email-verify mutation. */
  sendCode: () => void;
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
  isforget,
  getEmail,
  runRecaptcha,
}: SendEmailVerifyFlowOptions): SendEmailVerifyFlow {
  const { t } = useTranslation();
  const { mutateAsync: sendCodeMutation, isPending: isSendingCode } = useSendEmailVerifyMutation();
  const cooldown = useCountdown(60);

  const onSendCode = async (recaptchaData?: string) => {
    try {
      const sent = await sendCodeMutation({
        email: getEmail(),
        isforget,
        ...(recaptchaData ? { recaptcha_data: recaptchaData } : {}),
      });
      if (!sent) return;
      toast.success(t('auth.email_code_sent_title'), {
        description: t('auth.email_code_sent_description'),
      });
      cooldown.start();
    } catch {}
  };

  const sendCode = () => runRecaptcha(onSendCode);

  return {
    sendCode,
    isSendingCode,
    cooldownActive: cooldown.isActive,
    cooldownRemaining: cooldown.remaining,
  };
}
