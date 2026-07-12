import { useState, type ReactElement } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
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
import { serverGroupFormSchema, type ServerGroupFormValues } from './server-form-schema';

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
  const [open, setOpen] = useState(false);
  const form = useForm<ServerGroupFormValues>({
    resolver: zodResolver(serverGroupFormSchema),
    defaultValues: { id: record?.id, name: record?.name ?? '' },
  });

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
            <DialogTitle>{record?.id ? '编辑组' : '创建组'}</DialogTitle>
            <DialogDescription>设置权限组名称及其节点访问范围。</DialogDescription>
          </DialogHeader>
          <Field className="mt-4" data-invalid={Boolean(form.formState.errors.name)}>
            <FieldLabel htmlFor="server-group-name">组名</FieldLabel>
            <Input
              {...form.register('name')}
              id="server-group-name"
              placeholder="请输入组名"
              aria-invalid={Boolean(form.formState.errors.name)}
              aria-describedby={form.formState.errors.name ? groupNameErrorId : undefined}
              data-testid="server-group-name"
            />
            <FieldError id={groupNameErrorId} errors={[form.formState.errors.name]} />
          </Field>
          <DialogFooter className="mt-4">
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              type="submit"
              disabled={pending || form.formState.isSubmitting}
              data-testid="server-group-submit"
            >
              {pending || form.formState.isSubmitting ? (
                <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
              ) : null}
              提交
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
