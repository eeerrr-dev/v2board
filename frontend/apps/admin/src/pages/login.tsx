import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useLoaderData, useNavigate } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm } from 'react-hook-form';
import { z } from 'zod';
import { LogIn, ShieldCheck } from 'lucide-react';
import { hasProblemCode, passport } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { logout, setAuthData, type AdminLoginLoaderData } from '@/lib/auth';
import { adminSessionQueryOptions } from '@/lib/session-queries';
import { getAdminBackgroundUrl, getAdminLogo, getAdminTitle } from '@/lib/runtime-config';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { toast } from '@/lib/toast';

const emailInput = z.string().trim().min(1, '请输入邮箱');
const passwordError = '密码至少需要 8 个字符';
const loginSchema = z.object({
  email: emailInput.pipe(z.email('请输入有效邮箱')),
  // AuthLogin counts Unicode characters (Laravel mb_strlen semantics), not
  // UTF-16 code units. Keep spaces intact because passwords are not trimmed.
  password: z
    .string()
    .min(8, passwordError)
    .refine((value) => value.length < 8 || Array.from(value).length >= 8, passwordError),
  totp_code: z.string().optional(),
});

type LoginValues = z.infer<typeof loginSchema>;

const LOGIN_EMAIL_ID = 'admin-login-email';
const LOGIN_PASSWORD_ID = 'admin-login-password';
const LOGIN_TOTP_ID = 'admin-login-totp';

export default function LoginPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { redirectTarget } = useLoaderData() as AdminLoginLoaderData;
  const [forgotOpen, setForgotOpen] = useState(false);
  // §6.10 two-phase login: a privileged account with an enabled TOTP factor
  // answers the plain submit with 401 `mfa_code_required`; reveal the code
  // field and resubmit with `totp_code`.
  const [mfaRequired, setMfaRequired] = useState(false);
  const logo = getAdminLogo();
  const title = getAdminTitle();
  const backgroundUrl = getAdminBackgroundUrl();
  const login = useMutation({
    mutationFn: (payload: Parameters<typeof passport.login>[1]) =>
      passport.login(apiClient, payload),
  });
  const form = useForm<LoginValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { email: '', password: '', totp_code: '' },
  });

  const submit = form.handleSubmit(({ email, password, totp_code }) => {
    const code = totp_code?.trim();
    login.mutate(
      { email, password, ...(mfaRequired && code ? { totp_code: code } : {}) },
      {
        onSuccess: (result) => {
          if (!result.is_admin) {
            logout();
            toast.error('无管理员权限');
            return;
          }
          setAuthData(result.auth_data);
          void queryClient.prefetchQuery(adminSessionQueryOptions.userInfo());
          void navigate(redirectTarget, { replace: true });
        },
        onError: (error) => {
          if (hasProblemCode(error, 'mfa_code_required')) {
            setMfaRequired(true);
            form.setFocus('totp_code');
            return;
          }
          if (hasProblemCode(error, 'mfa_code_invalid')) {
            setMfaRequired(true);
            form.setError('totp_code', { message: '验证码错误或已被使用' });
          }
        },
      },
    );
  });

  return (
    <div
      data-testid="admin-login-surface"
      className="relative flex min-h-screen items-center justify-center bg-background px-4 py-12"
    >
      {backgroundUrl ? (
        <img
          src={backgroundUrl}
          alt=""
          aria-hidden
          decoding="async"
          fetchPriority="high"
          className="pointer-events-none absolute inset-0 size-full object-cover"
        />
      ) : null}
      <Card className="relative w-full max-w-sm shadow-lg" data-testid="admin-login-card">
        <CardHeader className="items-center gap-2 text-center">
          {logo ? (
            <img
              src={logo}
              alt={title || 'V2Board'}
              decoding="async"
              className="h-10 object-contain"
            />
          ) : (
            <CardTitle className="text-2xl">{title || 'V2Board'}</CardTitle>
          )}
          <CardDescription>登录到管理中心</CardDescription>
        </CardHeader>
        <CardContent>
          <form className="grid gap-4" onSubmit={submit} noValidate>
            <Controller
              control={form.control}
              name="email"
              render={({ field, fieldState }) => {
                const errorId = `${LOGIN_EMAIL_ID}-error`;
                return (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor={LOGIN_EMAIL_ID}>邮箱</FieldLabel>
                    <Input
                      {...field}
                      id={LOGIN_EMAIL_ID}
                      type="email"
                      autoComplete="username"
                      placeholder="邮箱"
                      aria-invalid={fieldState.invalid}
                      aria-describedby={fieldState.invalid ? errorId : undefined}
                    />
                    <FieldError id={errorId} errors={[fieldState.error]} />
                  </Field>
                );
              }}
            />
            <Controller
              control={form.control}
              name="password"
              render={({ field, fieldState }) => {
                const errorId = `${LOGIN_PASSWORD_ID}-error`;
                return (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor={LOGIN_PASSWORD_ID}>密码</FieldLabel>
                    <Input
                      {...field}
                      id={LOGIN_PASSWORD_ID}
                      type="password"
                      autoComplete="current-password"
                      placeholder="密码"
                      aria-invalid={fieldState.invalid}
                      aria-describedby={fieldState.invalid ? errorId : undefined}
                    />
                    <FieldError id={errorId} errors={[fieldState.error]} />
                  </Field>
                );
              }}
            />
            {mfaRequired ? (
              <Controller
                control={form.control}
                name="totp_code"
                render={({ field, fieldState }) => {
                  const errorId = `${LOGIN_TOTP_ID}-error`;
                  return (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor={LOGIN_TOTP_ID}>
                        <ShieldCheck className="size-4" />
                        两步验证码
                      </FieldLabel>
                      <Input
                        {...field}
                        id={LOGIN_TOTP_ID}
                        inputMode="numeric"
                        autoComplete="one-time-code"
                        maxLength={6}
                        placeholder="验证器 App 中的 6 位验证码"
                        aria-invalid={fieldState.invalid}
                        aria-describedby={fieldState.invalid ? errorId : undefined}
                        data-testid="admin-login-totp"
                      />
                      <FieldError id={errorId} errors={[fieldState.error]} />
                    </Field>
                  );
                }}
              />
            ) : null}
            <Button type="submit" block loading={login.isPending} data-testid="admin-login-submit">
              <LogIn className="size-4" />
              登入
            </Button>
          </form>
          <div className="mt-4 text-center">
            <button
              type="button"
              data-testid="admin-forgot-password"
              className="text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
              onClick={() => setForgotOpen(true)}
            >
              忘记密码
            </button>
          </div>
        </CardContent>
      </Card>

      <Dialog open={forgotOpen} onOpenChange={setForgotOpen}>
        <DialogContent className="sm:max-w-md" data-testid="admin-forgot-dialog">
          <DialogHeader>
            <DialogTitle>忘记密码</DialogTitle>
            <DialogDescription>在站点目录下执行命令找回密码</DialogDescription>
          </DialogHeader>
          <code className="grid rounded-md bg-muted px-3 py-2 font-mono text-sm text-foreground">
            {"V2BOARD_NEW_PASSWORD='新密码' v2board-api reset-admin-password 管理员邮箱"}
          </code>
          <DialogFooter>
            <Button type="button" onClick={() => setForgotOpen(false)}>
              我知道了
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
