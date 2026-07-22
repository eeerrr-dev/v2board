import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, ShieldCheck, ShieldOff } from 'lucide-react';
import { QRCodeSVG } from 'qrcode.react';
import { ApiError, ApiProblemError, hasProblemCode } from '@v2board/api-client';
import { copyText } from '@v2board/config/clipboard';
import { Button } from '@v2board/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@v2board/ui/dialog';
import { Field, FieldLabel } from '@v2board/ui/field';
import { Input } from '@v2board/ui/input';
import { Skeleton } from '@v2board/ui/skeleton';
import {
  useAccountMfa,
  useConfirmTotpMutation,
  useDisableTotpMutation,
  useSetupTotpMutation,
} from '@/lib/queries';
import { toast } from '@v2board/app-shell/toast';

interface MfaDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * Account two-factor management (§6.10): enroll a TOTP factor by scanning the
 * one-time provisioning secret, confirm it with a live code, or disable an
 * enabled factor with a live code. The mutations run behind the standard
 * step-up gate, so the shared prompt may interleave.
 */
export function MfaDialog({ open, onOpenChange }: MfaDialogProps) {
  const { t } = useTranslation();
  const status = useAccountMfa(open);
  const setup = useSetupTotpMutation();
  const confirm = useConfirmTotpMutation();
  const disable = useDisableTotpMutation();
  const [code, setCode] = useState('');
  const [codeError, setCodeError] = useState<string | null>(null);

  // Reset transient enrollment state whenever the dialog transitions; the
  // provisioning secret must not survive a close/reopen.
  const [lastOpen, setLastOpen] = useState(open);
  if (open !== lastOpen) {
    setLastOpen(open);
    setCode('');
    setCodeError(null);
    setup.reset();
  }

  const provisioning = setup.data;
  const enabled = status.data?.totp_enabled === true;
  const busy = confirm.isPending || disable.isPending;

  const codeErrorMessage = (error: unknown): string => {
    if (hasProblemCode(error, 'mfa_code_invalid')) return t(($) => $.admin.auth.mfa_code_invalid);
    if (error instanceof ApiProblemError || error instanceof ApiError) return error.message;
    return t(($) => $.admin.auth.operation_failed);
  };

  const submitConfirm = () => {
    if (busy || code.trim() === '') return;
    setCodeError(null);
    confirm.mutate(code.trim(), {
      onSuccess: () => {
        toast.success(t(($) => $.admin.auth.mfa_enabled));
        setCode('');
        setup.reset();
      },
      onError: (error) => setCodeError(codeErrorMessage(error)),
    });
  };

  const submitDisable = () => {
    if (busy || code.trim() === '') return;
    setCodeError(null);
    disable.mutate(code.trim(), {
      onSuccess: () => {
        toast.success(t(($) => $.admin.auth.mfa_disabled));
        setCode('');
      },
      onError: (error) => setCodeError(codeErrorMessage(error)),
    });
  };

  const copyValue = async (value: string) => {
    if (await copyText(value)) toast.success(t(($) => $.admin.auth.copy_success));
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" data-testid="admin-mfa-dialog">
        <DialogHeader>
          <DialogTitle>{t(($) => $.admin.auth.mfa_title)}</DialogTitle>
          <DialogDescription>{t(($) => $.admin.auth.mfa_description)}</DialogDescription>
        </DialogHeader>

        {status.isPending ? (
          <div className="grid gap-2">
            <Skeleton className="h-5 w-40" />
            <Skeleton className="h-9 w-full" />
          </div>
        ) : enabled ? (
          <div className="grid gap-4">
            <div className="flex items-center gap-2 text-sm">
              <ShieldCheck className="size-4 text-emerald-500" />
              <span>
                {t(($) => $.admin.auth.mfa_enabled)}
                {status.data?.totp_enabled_at
                  ? `（${new Date(status.data.totp_enabled_at).toLocaleString()}）`
                  : null}
              </span>
            </div>
            <Field data-invalid={codeError !== null}>
              <FieldLabel htmlFor="admin-mfa-disable-code">
                {t(($) => $.admin.auth.mfa_disable_label)}
              </FieldLabel>
              <Input
                id="admin-mfa-disable-code"
                value={code}
                inputMode="numeric"
                autoComplete="one-time-code"
                maxLength={6}
                placeholder={t(($) => $.admin.auth.code_placeholder)}
                data-testid="admin-mfa-disable-code"
                onChange={(event) => setCode(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') submitDisable();
                }}
              />
              {codeError ? (
                <p role="alert" className="text-sm text-destructive">
                  {codeError}
                </p>
              ) : null}
            </Field>
            <DialogFooter>
              <Button
                type="button"
                variant="destructive"
                disabled={code.trim() === ''}
                loading={disable.isPending}
                data-testid="admin-mfa-disable-submit"
                onClick={submitDisable}
              >
                <ShieldOff className="size-4" />
                {t(($) => $.admin.auth.mfa_disable_submit)}
              </Button>
            </DialogFooter>
          </div>
        ) : provisioning ? (
          <div className="grid gap-4">
            <div className="flex justify-center rounded-md bg-white p-4">
              <QRCodeSVG value={provisioning.otpauth_url} size={168} />
            </div>
            <div className="grid gap-1 text-sm">
              <span className="text-muted-foreground">
                {t(($) => $.admin.auth.mfa_manual_secret)}
              </span>
              <button
                type="button"
                className="flex items-center gap-2 rounded-md bg-muted px-3 py-2 text-left font-mono text-xs break-all"
                data-testid="admin-mfa-secret"
                onClick={() => void copyValue(provisioning.secret)}
              >
                {provisioning.secret}
                <Copy className="size-3.5 shrink-0 text-muted-foreground" />
              </button>
            </div>
            <Field data-invalid={codeError !== null}>
              <FieldLabel htmlFor="admin-mfa-confirm-code">
                {t(($) => $.admin.auth.mfa_confirm_label)}
              </FieldLabel>
              <Input
                id="admin-mfa-confirm-code"
                value={code}
                inputMode="numeric"
                autoComplete="one-time-code"
                maxLength={6}
                placeholder={t(($) => $.admin.auth.code_placeholder)}
                data-testid="admin-mfa-confirm-code"
                onChange={(event) => setCode(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') submitConfirm();
                }}
              />
              {codeError ? (
                <p role="alert" className="text-sm text-destructive">
                  {codeError}
                </p>
              ) : null}
            </Field>
            <DialogFooter>
              <Button
                type="button"
                disabled={code.trim() === ''}
                loading={confirm.isPending}
                data-testid="admin-mfa-confirm-submit"
                onClick={submitConfirm}
              >
                <ShieldCheck className="size-4" />
                {t(($) => $.admin.auth.mfa_confirm_submit)}
              </Button>
            </DialogFooter>
          </div>
        ) : (
          <div className="grid gap-4">
            <p className="text-sm text-muted-foreground">
              {t(($) => $.admin.auth.mfa_intro)}{' '}
              <code className="rounded bg-muted px-1 py-0.5 font-mono text-xs">
                {t(($) => $.admin.auth.mfa_reset_command)}
              </code>{' '}
              {t(($) => $.admin.auth.mfa_intro_suffix)}
            </p>
            <DialogFooter>
              <Button
                type="button"
                loading={setup.isPending}
                data-testid="admin-mfa-setup"
                onClick={() =>
                  setup.mutate(undefined, {
                    onError: (error) => setCodeError(codeErrorMessage(error)),
                  })
                }
              >
                <ShieldCheck className="size-4" />
                {t(($) => $.admin.auth.mfa_setup_start)}
              </Button>
            </DialogFooter>
            {codeError ? (
              <p role="alert" className="text-sm text-destructive">
                {codeError}
              </p>
            ) : null}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
