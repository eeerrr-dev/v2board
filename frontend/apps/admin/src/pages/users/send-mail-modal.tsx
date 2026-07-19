import { useEffect } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState } from 'react-hook-form';
import type { AdminFilter } from '@v2board/api-client';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Textarea } from '@/components/ui/textarea';
import { sendMailSchema, type SendMailValues } from '../user-action-form-schema';
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
          <DialogTitle>发送邮件</DialogTitle>
          <DialogDescription>向当前筛选范围内的用户发送邮件。</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={submit} noValidate>
          <FieldError errors={[formErrors.root?.serverError]} />
          <Field>
            <FieldLabel htmlFor="send-mail-recipient">收件人</FieldLabel>
            <Input
              id="send-mail-recipient"
              disabled
              value={filter.length ? '过滤用户' : '全部用户'}
            />
          </Field>
          <Field data-invalid={Boolean(formErrors.subject)}>
            <FieldLabel htmlFor="send-mail-subject">主题</FieldLabel>
            <Controller
              control={form.control}
              name="subject"
              render={({ field, fieldState }) => (
                <Input
                  {...field}
                  id="send-mail-subject"
                  placeholder="请输入邮件主题"
                  data-testid="send-mail-subject"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.subject]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.content)}>
            <FieldLabel htmlFor="send-mail-content">发送内容</FieldLabel>
            <Controller
              control={form.control}
              name="content"
              render={({ field, fieldState }) => (
                <Textarea
                  {...field}
                  id="send-mail-content"
                  rows={12}
                  placeholder="请输入邮件内容"
                  data-testid="send-mail-content"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.content]} />
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              取消
            </Button>
            <Button
              type="submit"
              disabled={loading || isSubmitting}
              loading={loading || isSubmitting}
              data-testid="send-mail-submit"
            >
              确定
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
