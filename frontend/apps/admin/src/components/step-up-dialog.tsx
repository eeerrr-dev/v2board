import { useState, useSyncExternalStore } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { ApiError, ApiProblemError, passport } from '@v2board/api-client';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Field, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { apiClient } from '@/lib/api';
import { toast } from '@/lib/toast';
import {
  isStepUpPromptRequested,
  resolveStepUpPrompt,
  setStepUpGrant,
  subscribeStepUpPrompt,
} from '@/lib/step-up';

/**
 * Global re-auth prompt for the privileged step-up gate. Opened imperatively
 * (lib/step-up.ts maybePromptStepUp) when a request fails with the backend's
 * step_up_required 403 problem. A successful verification
 * stores the grant for the x-v2board-step-up header and refetches active
 * queries; the failed mutation's form state stays intact for a manual retry.
 */
export function StepUpDialogProvider() {
  const open = useSyncExternalStore(
    subscribeStepUpPrompt,
    isStepUpPromptRequested,
    isStepUpPromptRequested,
  );
  const queryClient = useQueryClient();
  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Radix still closes on Escape/overlay-click while a verify is in flight,
  // and the late rejection then writes its error into the closed dialog.
  // Reset render-time on the open transition so a reopen always starts clean.
  const [lastOpen, setLastOpen] = useState(open);
  if (open !== lastOpen) {
    setLastOpen(open);
    if (open) {
      setPassword('');
      setError(null);
      setSubmitting(false);
    }
  }

  const close = () => {
    setPassword('');
    setError(null);
    setSubmitting(false);
    resolveStepUpPrompt();
  };

  const onSubmit = async () => {
    if (submitting || password === '') return;
    setSubmitting(true);
    setError(null);
    try {
      const grant = await passport.stepUp(apiClient, { password });
      setStepUpGrant(grant.step_up_token, grant.expires_in);
      close();
      toast.success('验证成功，请重试刚才的操作');
      // Sensitive reads that failed on the same gate recover on refetch now
      // that the header is available.
      void queryClient.invalidateQueries();
    } catch (submitError) {
      setSubmitting(false);
      // A wrong password is a 400 invalid_credentials problem post-W2; the
      // ApiError arm keeps covering transport-level failures.
      setError(
        submitError instanceof ApiProblemError || submitError instanceof ApiError
          ? submitError.message
          : '验证失败，请稍后再试',
      );
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) close();
      }}
    >
      <DialogContent className="sm:max-w-[26rem]">
        <DialogHeader>
          <DialogTitle>验证管理员密码</DialogTitle>
          <DialogDescription>此操作需要重新验证您的登录密码。</DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void onSubmit();
          }}
          className="space-y-4"
        >
          <Field data-invalid={Boolean(error)}>
            <FieldLabel htmlFor="step-up-password">当前密码</FieldLabel>
            <Input
              id="step-up-password"
              type="password"
              autoComplete="current-password"
              value={password}
              aria-invalid={Boolean(error)}
              onChange={(event) => {
                setPassword(event.target.value);
                setError(null);
              }}
            />
            {error ? <p className="text-destructive text-sm">{error}</p> : null}
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" disabled={submitting} onClick={close}>
              取消
            </Button>
            <Button type="submit" loading={submitting} disabled={password === ''}>
              验证
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
