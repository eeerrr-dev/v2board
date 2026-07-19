import { useState, type ReactElement } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm, useFormState } from 'react-hook-form';
import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import type { admin } from '@v2board/api-client';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { serverGroupFormSchema, type ServerGroupFormValues } from './form-schema';

export function ServerGroupDialog({
  record,
  pending,
  onSave,
  children,
}: {
  record?: admin.ServerGroup;
  pending: boolean;
  onSave: (payload: ServerGroupFormValues, onSuccess: () => void) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const form = useForm<ServerGroupFormValues>({
    resolver: zodResolver(serverGroupFormSchema),
    defaultValues: { id: record?.id, name: record?.name ?? '' },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors, isSubmitting } = useFormState({ control: form.control });

  const openModal = () => {
    form.reset({ id: record?.id, name: record?.name ?? '' });
    setOpen(true);
  };

  const saveGroup = form.handleSubmit((values) => {
    onSave(values.id === undefined ? { name: values.name } : values, () => setOpen(false));
  });
  const groupNameErrorId = 'server-group-name-error';

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (nextOpen) openModal();
        else setOpen(false);
      }}
    >
      <DialogTrigger asChild>{children}</DialogTrigger>
      <DialogContent data-testid="server-group-editor">
        <form onSubmit={(event) => void saveGroup(event)}>
          <DialogHeader>
            <DialogTitle>
              {record?.id
                ? t(($) => $.admin.servers.edit_group)
                : t(($) => $.admin.servers.create_group)}
            </DialogTitle>
            <DialogDescription>
              {t(($) => $.admin.servers.group_editor_description)}
            </DialogDescription>
          </DialogHeader>
          <Field className="mt-4" data-invalid={Boolean(formErrors.name)}>
            <FieldLabel htmlFor="server-group-name">
              {t(($) => $.admin.servers.group_name_label)}
            </FieldLabel>
            <Input
              {...form.register('name')}
              id="server-group-name"
              placeholder={t(($) => $.admin.servers.group_name_placeholder)}
              aria-invalid={Boolean(formErrors.name)}
              aria-describedby={formErrors.name ? groupNameErrorId : undefined}
              data-testid="server-group-name"
            />
            <FieldError id={groupNameErrorId} errors={[formErrors.name]} />
          </Field>
          <DialogFooter className="mt-4">
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              {t(($) => $.common.cancel)}
            </Button>
            <Button
              type="submit"
              disabled={pending || isSubmitting}
              data-testid="server-group-submit"
            >
              {pending || isSubmitting ? (
                <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
              ) : null}
              {t(($) => $.common.submit)}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
