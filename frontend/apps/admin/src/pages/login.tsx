import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useLoaderData, useNavigate } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm } from 'react-hook-form';
import { z } from 'zod';
import { LogIn, ShieldCheck } from 'lucide-react';
import { hasProblemCode, passport } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { logout, setAuthData, type AdminLoginLoaderData } from '@/lib/auth';
import { canEnterAdminNamespace, firstAllowedRoute, sessionAllowsRoute } from '@/lib/permissions';
import { adminSessionQueryOptions } from '@/lib/session-queries';
import { getAdminBackgroundUrl, getAdminLogo, getAdminTitle } from '@/lib/runtime-config';
import { Button } from '@v2board/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@v2board/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@v2board/ui/dialog';
import { Field, FieldError, FieldLabel } from '@v2board/ui/field';
import { Input } from '@v2board/ui/input';
import { toast } from '@v2board/app-shell/toast';

// Flat runtime message keys (FieldError resolves them through
// translateRuntimeMessage); they must match the admin.auth fragment.
const LOGIN_VALIDATION = {
  emailRequired: 'admin.auth.email_required',
  emailInvalid: 'admin.auth.email_invalid',
  passwordMin: 'admin.auth.password_min',
  mfaCodeInvalid: 'admin.auth.mfa_code_invalid',
} as const;

const emailInput = z.string().trim().min(1, LOGIN_VALIDATION.emailRequired);
const loginSchema = z.object({
  email: emailInput.pipe(z.email(LOGIN_VALIDATION.emailInvalid)),
  // AuthLogin counts Unicode characters (Laravel mb_strlen semantics), not
  // UTF-16 code units. Keep spaces intact because passwords are not trimmed.
  password: z
    .string()
    .min(8, LOGIN_VALIDATION.passwordMin)
    .refine(
      (value) => value.length < 8 || Array.from(value).length >= 8,
      LOGIN_VALIDATION.passwordMin,
    ),
  totp_code: z.string().optional(),
});

type LoginValues = z.infer<typeof loginSchema>;

const LOGIN_EMAIL_ID = 'admin-login-email';
const LOGIN_PASSWORD_ID = 'admin-login-password';
const LOGIN_TOTP_ID = 'admin-login-totp';

export default function LoginPage() {
  const { t } = useTranslation();
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

  // §6.12: the login body only carries `is_admin`; a staff account proves its
  // grants through the session probe before entering the admin namespace.
  const enterAsStaff = async () => {
    try {
      const session = await queryClient.fetchQuery(adminSessionQueryOptions.session());
      if (!canEnterAdminNamespace(session)) {
        logout();
        toast.error(t(($) => $.admin.auth.not_admin));
        return;
      }
      void queryClient.prefetchQuery(adminSessionQueryOptions.userInfo());
      const target = sessionAllowsRoute(session, redirectTarget)
        ? redirectTarget
        : firstAllowedRoute(session);
      void navigate(target, { replace: true });
    } catch {
      logout();
      toast.error(t(($) => $.admin.auth.not_admin));
    }
  };

  const submit = form.handleSubmit(({ email, password, totp_code }) => {
    const code = totp_code?.trim();
    login.mutate(
      { email, password, ...(mfaRequired && code ? { totp_code: code } : {}) },
      {
        onSuccess: (result) => {
          if (result.is_admin) {
            setAuthData(result.auth_data);
            void queryClient.prefetchQuery(adminSessionQueryOptions.userInfo());
            void navigate(redirectTarget, { replace: true });
            return;
          }
          setAuthData(result.auth_data);
          void enterAsStaff();
        },
        onError: (error) => {
          if (hasProblemCode(error, 'mfa_code_required')) {
            setMfaRequired(true);
            form.setFocus('totp_code');
            return;
          }
          if (hasProblemCode(error, 'mfa_code_invalid')) {
            setMfaRequired(true);
            form.setError('totp_code', { message: LOGIN_VALIDATION.mfaCodeInvalid });
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
          <CardDescription>{t(($) => $.admin.auth.login_description)}</CardDescription>
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
                    <FieldLabel htmlFor={LOGIN_EMAIL_ID}>{t(($) => $.admin.auth.email)}</FieldLabel>
                    <Input
                      {...field}
                      id={LOGIN_EMAIL_ID}
                      type="email"
                      autoComplete="username"
                      placeholder={t(($) => $.admin.auth.email)}
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
                    <FieldLabel htmlFor={LOGIN_PASSWORD_ID}>
                      {t(($) => $.admin.auth.password)}
                    </FieldLabel>
                    <Input
                      {...field}
                      id={LOGIN_PASSWORD_ID}
                      type="password"
                      autoComplete="current-password"
                      placeholder={t(($) => $.admin.auth.password)}
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
                        {t(($) => $.admin.auth.mfa_code_label)}
                      </FieldLabel>
                      <Input
                        {...field}
                        id={LOGIN_TOTP_ID}
                        inputMode="numeric"
                        autoComplete="one-time-code"
                        maxLength={6}
                        placeholder={t(($) => $.admin.auth.mfa_code_placeholder)}
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
              {t(($) => $.admin.auth.sign_in)}
            </Button>
          </form>
          <div className="mt-4 text-center">
            <button
              type="button"
              data-testid="admin-forgot-password"
              className="text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
              onClick={() => setForgotOpen(true)}
            >
              {t(($) => $.admin.auth.forgot_password)}
            </button>
          </div>
        </CardContent>
      </Card>

      <Dialog open={forgotOpen} onOpenChange={setForgotOpen}>
        <DialogContent className="sm:max-w-md" data-testid="admin-forgot-dialog">
          <DialogHeader>
            <DialogTitle>{t(($) => $.admin.auth.forgot_password)}</DialogTitle>
            <DialogDescription>{t(($) => $.admin.auth.forgot_description)}</DialogDescription>
          </DialogHeader>
          <code className="grid rounded-md bg-muted px-3 py-2 font-mono text-sm text-foreground">
            {t(($) => $.admin.auth.reset_password_command)}
          </code>
          <DialogFooter>
            <Button type="button" onClick={() => setForgotOpen(false)}>
              {t(($) => $.admin.auth.got_it)}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
