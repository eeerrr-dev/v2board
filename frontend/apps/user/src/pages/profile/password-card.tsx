import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { KeyRound } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/components/ui/form';
import { Input } from '@/components/ui/input';
import { useChangePasswordMutation } from '@/lib/queries';
import { toast } from '@/lib/toast';
import { SectionIcon } from './profile-ui';

const passwordSchema = z
  .object({
    oldPassword: z.string(),
    newPassword: z.string(),
    confirmPassword: z.string(),
  })
  // Inlined (not the shared makeConfirmPasswordRefinement, which stamps the raw
  // 'password_mismatch' key) so FormMessage translates the profile-specific
  // display key — the two keys resolve to different strings in the dictionary.
  .superRefine((values, context) => {
    if (values.newPassword !== values.confirmPassword) {
      context.addIssue({
        code: 'custom',
        path: ['confirmPassword'],
        message: 'profile.password_mismatch',
      });
    }
  });

type PasswordFormValues = z.infer<typeof passwordSchema>;

export function PasswordCard() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const changePassword = useChangePasswordMutation();
  const passwordForm = useForm<PasswordFormValues>({
    resolver: zodResolver(passwordSchema),
    defaultValues: { oldPassword: '', newPassword: '', confirmPassword: '' },
  });

  const onChangePwd = passwordForm.handleSubmit(
    async (values) => {
      try {
        await changePassword.mutateAsync({
          oldPassword: values.oldPassword,
          newPassword: values.newPassword,
        });
        toast.success(t('profile.change_password_success'));
        navigate('/login');
      } catch {}
    },
    () => toast.error(t('profile.password_mismatch')),
  );

  return (
    <Card data-testid="profile-password-card">
      <CardHeader>
        <div className="flex items-center gap-3">
          <SectionIcon>
            <KeyRound className="size-4" />
          </SectionIcon>
          <CardTitle className="text-lg" data-testid="profile-card-title">
            {t('profile.change_password')}
          </CardTitle>
        </div>
      </CardHeader>
      <CardContent>
        <Form {...passwordForm}>
          <form className="space-y-5" onSubmit={onChangePwd} noValidate>
            <div className="grid gap-4">
              <FormField
                control={passwordForm.control}
                name="oldPassword"
                render={({ field }) => (
                  <FormItem className="gap-2.5">
                    <FormLabel>{t('profile.old_password')}</FormLabel>
                    <FormControl>
                      <Input
                        type="password"
                        placeholder={t('profile.old_password_placeholder')}
                        {...field}
                      />
                    </FormControl>
                  </FormItem>
                )}
              />
              <FormField
                control={passwordForm.control}
                name="newPassword"
                render={({ field }) => (
                  <FormItem className="gap-2.5">
                    <FormLabel>{t('profile.new_password')}</FormLabel>
                    <FormControl>
                      <Input
                        type="password"
                        placeholder={t('profile.new_password_placeholder')}
                        {...field}
                      />
                    </FormControl>
                  </FormItem>
                )}
              />
              <FormField
                control={passwordForm.control}
                name="confirmPassword"
                render={({ field, fieldState }) => (
                  <FormItem className="gap-2.5">
                    <FormLabel>{t('profile.new_password')}</FormLabel>
                    <FormControl>
                      <Input
                        type="password"
                        placeholder={t('profile.new_password_placeholder')}
                        invalid={fieldState.error ? true : undefined}
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>
            <Button
              type="submit"
              className="w-full sm:w-fit"
              data-testid="profile-password-save"
              loading={changePassword.isPending}
            >
              {t('profile.save')}
            </Button>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}
