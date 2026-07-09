import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { LogIn } from 'lucide-react';
import { passport, user } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { getAuthData, setAuthData } from '@/lib/auth';
import { getAdminBackgroundUrl, getAdminLogo, getAdminTitle } from '@/lib/legacy-settings';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/components/ui/form';
import { Input } from '@/components/ui/input';

const loginSchema = z.object({
  email: z.string().trim().min(1, '请输入邮箱'),
  password: z.string().min(1, '请输入密码'),
});

type LoginValues = z.infer<typeof loginSchema>;

export default function LoginPage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const [forgotOpen, setForgotOpen] = useState(false);
  const logo = getAdminLogo();
  const title = getAdminTitle();
  const backgroundUrl = getAdminBackgroundUrl();
  const redirect = params.get('redirect') || 'dashboard';
  const form = useForm<LoginValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { email: '', password: '' },
  });

  const submit = form.handleSubmit(async ({ email, password }) => {
    try {
      const result = await passport.login(apiClient, { email, password });
      setAuthData(result.auth_data);
      // A non-admin login stays on this screen (the backend error is surfaced by
      // the global onError handler); only an admin session enters the console.
      if (!result.is_admin) return;
      navigate('/dashboard');
      void user.info(apiClient).catch(() => undefined);
    } catch {
      // Login failures are surfaced by the global onError handler (legacy parity).
    }
  });

  useEffect(() => {
    // An existing admin session skips the form and resumes at the redirect target.
    if (!getAuthData()) return;
    user
      .checkLogin(apiClient)
      .then((result) => {
        if (result.is_admin) {
          void user.info(apiClient).catch(() => undefined);
          navigate(redirect);
        }
      })
      .catch(() => undefined);
  }, [navigate, redirect]);

  return (
    <div className="v2board-island v2board-auth-box relative flex min-h-screen items-center justify-center bg-background px-4 py-12">
      {backgroundUrl ? (
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 bg-cover bg-center"
          style={{ backgroundImage: `url(${backgroundUrl})` }}
        />
      ) : null}
      <Card className="relative w-full max-w-sm shadow-lg" data-testid="admin-login-card">
        <CardHeader className="items-center gap-2 text-center">
          {logo ? (
            <img src={logo} alt={title || 'V2Board'} className="h-10 object-contain" />
          ) : (
            <CardTitle className="text-2xl">{title || 'V2Board'}</CardTitle>
          )}
          <CardDescription>登录到管理中心</CardDescription>
        </CardHeader>
        <CardContent>
          <Form {...form}>
            <form className="grid gap-4" onSubmit={submit} noValidate>
              <FormField
                control={form.control}
                name="email"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>邮箱</FormLabel>
                    <FormControl>
                      <Input type="email" autoComplete="username" placeholder="邮箱" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="password"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>密码</FormLabel>
                    <FormControl>
                      <Input
                        type="password"
                        autoComplete="current-password"
                        placeholder="密码"
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <Button
                type="submit"
                block
                loading={form.formState.isSubmitting}
                data-testid="admin-login-submit"
              >
                <LogIn className="size-4" />
                登入
              </Button>
            </form>
          </Form>
          <div className="mt-4 text-center">
            <button
              type="button"
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
          <code className="block rounded-md bg-muted px-3 py-2 font-mono text-sm text-foreground">
            php artisan reset:password 管理员邮箱
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
