import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm } from 'react-hook-form';
import { z } from 'zod';
import { KeyRound } from 'lucide-react';
import { Button } from '@v2board/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@v2board/ui/card';
import { Field, FieldError, FieldLabel } from '@v2board/ui/field';
import { Input } from '@v2board/ui/input';
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
  // 'password_mismatch' key) so FieldError translates the profile-specific
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

const OLD_PASSWORD_ID = 'profile-old-password';
const NEW_PASSWORD_ID = 'profile-new-password';
const CONFIRM_PASSWORD_ID = 'profile-confirm-password';

export function PasswordCard() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const changePassword = useChangePasswordMutation();
  const passwordForm = useForm<PasswordFormValues>({
    resolver: zodResolver(passwordSchema),
    defaultValues: { oldPassword: '', newPassword: '', confirmPassword: '' },
  });

  const onChangePwd = passwordForm.handleSubmit(
    (values) => {
      changePassword.mutate(
        {
          oldPassword: values.oldPassword,
          newPassword: values.newPassword,
        },
        {
          onSuccess: () => {
            toast.success(t(($) => $.profile.change_password_success));
            void navigate('/login');
          },
        },
      );
    },
    () => toast.error(t(($) => $.profile.password_mismatch)),
  );

  return (
    <Card data-testid="profile-password-card">
      <CardHeader>
        <div className="flex items-center gap-3">
          <SectionIcon>
            <KeyRound className="size-4" />
          </SectionIcon>
          <CardTitle className="text-lg" data-testid="profile-card-title">
            {t(($) => $.profile.change_password)}
          </CardTitle>
        </div>
      </CardHeader>
      <CardContent>
        <form className="space-y-5" onSubmit={onChangePwd} noValidate>
          <div className="grid gap-4">
            <Controller
              control={passwordForm.control}
              name="oldPassword"
              render={({ field, fieldState }) => {
                const errorId = `${OLD_PASSWORD_ID}-error`;
                return (
                  <Field className="gap-2.5" data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor={OLD_PASSWORD_ID}>
                      {t(($) => $.profile.old_password)}
                    </FieldLabel>
                    <Input
                      {...field}
                      id={OLD_PASSWORD_ID}
                      type="password"
                      autoComplete="current-password"
                      placeholder={t(($) => $.profile.old_password_placeholder)}
                      aria-invalid={fieldState.invalid}
                      aria-describedby={fieldState.invalid ? errorId : undefined}
                    />
                    <FieldError id={errorId} errors={[fieldState.error]} />
                  </Field>
                );
              }}
            />
            <Controller
              control={passwordForm.control}
              name="newPassword"
              render={({ field, fieldState }) => {
                const errorId = `${NEW_PASSWORD_ID}-error`;
                return (
                  <Field className="gap-2.5" data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor={NEW_PASSWORD_ID}>
                      {t(($) => $.profile.new_password)}
                    </FieldLabel>
                    <Input
                      {...field}
                      id={NEW_PASSWORD_ID}
                      type="password"
                      autoComplete="new-password"
                      placeholder={t(($) => $.profile.new_password_placeholder)}
                      aria-invalid={fieldState.invalid}
                      aria-describedby={fieldState.invalid ? errorId : undefined}
                    />
                    <FieldError id={errorId} errors={[fieldState.error]} />
                  </Field>
                );
              }}
            />
            <Controller
              control={passwordForm.control}
              name="confirmPassword"
              render={({ field, fieldState }) => {
                const errorId = `${CONFIRM_PASSWORD_ID}-error`;
                return (
                  <Field className="gap-2.5" data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor={CONFIRM_PASSWORD_ID}>
                      {t(($) => $.profile.new_password)}
                    </FieldLabel>
                    <Input
                      {...field}
                      id={CONFIRM_PASSWORD_ID}
                      type="password"
                      autoComplete="new-password"
                      placeholder={t(($) => $.profile.new_password_placeholder)}
                      aria-invalid={fieldState.invalid}
                      aria-describedby={fieldState.invalid ? errorId : undefined}
                    />
                    <FieldError id={errorId} errors={[fieldState.error]} />
                  </Field>
                );
              }}
            />
          </div>
          <Button
            type="submit"
            className="w-full sm:w-fit"
            data-testid="profile-password-save"
            loading={changePassword.isPending}
          >
            {t(($) => $.profile.save)}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
