import { useEffect } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState } from 'react-hook-form';
import { useTranslation } from 'react-i18next';
import type { AdminFilter } from '@v2board/api-client';
import { Button } from '@v2board/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@v2board/ui/dialog';
import { Input } from '@v2board/ui/input';
import { Field, FieldError, FieldLabel } from '@v2board/ui/field';
import { Textarea } from '@v2board/ui/textarea';
import { sendMailSchema, type SendMailValues } from './form-schema';
import { requestErrorMessage } from './shared';

export function SendMailModal({
  open,
  filter,
  loading,
  onClose,
  onSubmit,
}: {
  open: boolean;
  filter: AdminFilter[];
  loading: boolean;
  onClose: () => void;
  onSubmit: (values: SendMailValues) => Promise<void>;
}) {
  const { t } = useTranslation();
  const form = useForm<SendMailValues>({
    resolver: zodResolver(sendMailSchema),
    defaultValues: { subject: '', content: '' },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors, isSubmitting } = useFormState({ control: form.control });

  useEffect(() => {
    if (!open) form.reset();
  }, [form, open]);

  const close = () => {
    form.reset();
    onClose();
  };
  const submit = form.handleSubmit(async (values) => {
    form.clearErrors('root.serverError');
    try {
      await onSubmit(values);
    } catch (error) {
      form.setError('root.serverError', { message: requestErrorMessage(error) });
    }
  });

  return (
    <Dialog open={open} onOpenChange={(next) => (!next ? close() : undefined)}>
      <DialogContent data-testid="user-send-mail-dialog">
        <DialogHeader>
          <DialogTitle>{t(($) => $.admin.users.send_mail)}</DialogTitle>
          <DialogDescription>{t(($) => $.admin.users.send_mail_description)}</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={submit} noValidate>
          <FieldError errors={[formErrors.root?.serverError]} />
          <Field>
            <FieldLabel htmlFor="send-mail-recipient">
              {t(($) => $.admin.users.recipient)}
            </FieldLabel>
            <Input
              id="send-mail-recipient"
              disabled
              value={
                filter.length
                  ? t(($) => $.admin.users.filtered_users)
                  : t(($) => $.admin.users.all_users)
              }
            />
          </Field>
          <Field data-invalid={Boolean(formErrors.subject)}>
            <FieldLabel htmlFor="send-mail-subject">
              {t(($) => $.admin.users.mail_subject)}
            </FieldLabel>
            <Controller
              control={form.control}
              name="subject"
              render={({ field, fieldState }) => (
                <Input
                  {...field}
                  id="send-mail-subject"
                  placeholder={t(($) => $.admin.users.mail_subject_placeholder)}
                  data-testid="send-mail-subject"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.subject]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.content)}>
            <FieldLabel htmlFor="send-mail-content">
              {t(($) => $.admin.users.mail_content)}
            </FieldLabel>
            <Controller
              control={form.control}
              name="content"
              render={({ field, fieldState }) => (
                <Textarea
                  {...field}
                  id="send-mail-content"
                  rows={12}
                  placeholder={t(($) => $.admin.users.mail_content_placeholder)}
                  data-testid="send-mail-content"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.content]} />
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              {t(($) => $.common.cancel)}
            </Button>
            <Button
              type="submit"
              disabled={loading || isSubmitting}
              loading={loading || isSubmitting}
              data-testid="send-mail-submit"
            >
              {t(($) => $.common.confirm)}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
