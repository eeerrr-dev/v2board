import { useState } from 'react';
import { Copy, ShieldCheck, ShieldOff } from 'lucide-react';
import { QRCodeSVG } from 'qrcode.react';
import { ApiError, ApiProblemError, hasProblemCode } from '@v2board/api-client';
import { copyText } from '@v2board/config/clipboard';
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
import { Skeleton } from '@/components/ui/skeleton';
import {
  useAccountMfa,
  useConfirmTotpMutation,
  useDisableTotpMutation,
  useSetupTotpMutation,
} from '@/lib/queries';
import { toast } from '@/lib/toast';

interface MfaDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function codeErrorMessage(error: unknown): string {
  if (hasProblemCode(error, 'mfa_code_invalid')) return '验证码错误或已被使用';
  if (error instanceof ApiProblemError || error instanceof ApiError) return error.message;
  return '操作失败，请稍后重试';
}

/**
 * Account two-factor management (§6.10): enroll a TOTP factor by scanning the
 * one-time provisioning secret, confirm it with a live code, or disable an
 * enabled factor with a live code. The mutations run behind the standard
 * step-up gate, so the shared prompt may interleave.
 */
export function MfaDialog({ open, onOpenChange }: MfaDialogProps) {
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

  const submitConfirm = () => {
    if (busy || code.trim() === '') return;
    setCodeError(null);
    confirm.mutate(code.trim(), {
      onSuccess: () => {
        toast.success('两步验证已启用');
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
        toast.success('两步验证已关闭');
        setCode('');
      },
      onError: (error) => setCodeError(codeErrorMessage(error)),
    });
  };

  const copyValue = async (value: string) => {
    if (await copyText(value)) toast.success('复制成功');
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" data-testid="admin-mfa-dialog">
        <DialogHeader>
          <DialogTitle>两步验证</DialogTitle>
          <DialogDescription>
            使用 TOTP 验证器 App（如 Google Authenticator、1Password）为管理账号增加第二重保护。
          </DialogDescription>
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
                两步验证已启用
                {status.data?.totp_enabled_at
                  ? `（${new Date(status.data.totp_enabled_at).toLocaleString()}）`
                  : null}
              </span>
            </div>
            <Field data-invalid={codeError !== null}>
              <FieldLabel htmlFor="admin-mfa-disable-code">输入当前验证码以关闭</FieldLabel>
              <Input
                id="admin-mfa-disable-code"
                value={code}
                inputMode="numeric"
                autoComplete="one-time-code"
                maxLength={6}
                placeholder="6 位验证码"
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
                关闭两步验证
              </Button>
            </DialogFooter>
          </div>
        ) : provisioning ? (
          <div className="grid gap-4">
            <div className="flex justify-center rounded-md bg-white p-4">
              <QRCodeSVG value={provisioning.otpauth_url} size={168} />
            </div>
            <div className="grid gap-1 text-sm">
              <span className="text-muted-foreground">无法扫码时，手动输入密钥：</span>
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
              <FieldLabel htmlFor="admin-mfa-confirm-code">输入 App 中的验证码完成绑定</FieldLabel>
              <Input
                id="admin-mfa-confirm-code"
                value={code}
                inputMode="numeric"
                autoComplete="one-time-code"
                maxLength={6}
                placeholder="6 位验证码"
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
                确认并启用
              </Button>
            </DialogFooter>
          </div>
        ) : (
          <div className="grid gap-4">
            <p className="text-sm text-muted-foreground">
              当前账号未启用两步验证。启用后，登录除密码外还需输入验证器 App 中的动态验证码；
              如手机遗失，可由服务器操作员执行{' '}
              <code className="rounded bg-muted px-1 py-0.5 font-mono text-xs">
                v2board-api reset-admin-totp 邮箱
              </code>{' '}
              解除。
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
                生成密钥并开始设置
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
