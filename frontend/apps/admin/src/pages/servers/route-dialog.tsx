import { useState, type ReactElement } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import { ExternalLink, Loader2 } from 'lucide-react';
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
import { Textarea } from '@/components/ui/textarea';
import {
  ROUTE_ACTION_OPTIONS,
  SERVER_ROUTE_ACTIONS,
  getRouteMatchPlaceholder,
  getRouteMatchTextareaValue,
} from './domain';
import {
  serverRouteFormSchema,
  type ServerRouteAction,
  type ServerRouteFormValues,
} from './form-schema';
import { NodeSelect } from './form-controls';

export function ServerRouteDialog({
  route: initialRoute,
  pending,
  onSave,
  children,
}: {
  route?: admin.ServerRoute;
  pending: boolean;
  onSave: (route: ServerRouteFormValues, onSuccess: () => void) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const form = useForm<ServerRouteFormValues>({
    resolver: zodResolver(serverRouteFormSchema),
    defaultValues: getServerRouteFormValues(initialRoute),
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors, isSubmitting } = useFormState({ control: form.control });
  const action = useWatch({ control: form.control, name: 'action' });
  const openModal = () => {
    form.reset(getServerRouteFormValues(initialRoute));
    setOpen(true);
  };

  const saveRoute = form.handleSubmit((values) => {
    onSave(values, () => setOpen(false));
  });

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (nextOpen) openModal();
        else setOpen(false);
      }}
    >
      <DialogTrigger asChild>{children}</DialogTrigger>
      <DialogContent
        className="max-h-[calc(100vh-4rem)] overflow-y-auto"
        data-testid="server-route-editor"
      >
        <form onSubmit={(event) => void saveRoute(event)}>
          <DialogHeader>
            <DialogTitle>{initialRoute?.id ? '编辑路由' : '创建路由'}</DialogTitle>
            <DialogDescription>配置匹配条件、路由动作和目标值。</DialogDescription>
          </DialogHeader>

          <div className="mt-4 space-y-4">
            <Field data-invalid={Boolean(formErrors.remarks)}>
              <FieldLabel htmlFor="server-route-remarks">备注</FieldLabel>
              <Input
                {...form.register('remarks')}
                id="server-route-remarks"
                placeholder="请输入备注"
                aria-invalid={Boolean(formErrors.remarks)}
                data-testid="server-route-remarks"
              />
              <FieldError errors={[formErrors.remarks]} />
            </Field>

            {action !== 'default_out' ? (
              <Field data-invalid={Boolean(formErrors.match)}>
                <FieldLabel htmlFor="server-route-match" className="flex items-center gap-2">
                  匹配值
                  <a
                    className="inline-flex items-center gap-1 text-primary"
                    href="https://xtls.github.io/config/routing.html#ruleobject"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <ExternalLink className="size-3.5" />
                    填写参考
                  </a>
                </FieldLabel>
                <Textarea
                  {...form.register('match')}
                  id="server-route-match"
                  rows={5}
                  className="font-mono text-xs"
                  placeholder={getRouteMatchPlaceholder(action)}
                  aria-invalid={Boolean(formErrors.match)}
                  data-testid="server-route-match"
                />
                <FieldError errors={[formErrors.match]} />
              </Field>
            ) : null}

            <Field data-invalid={Boolean(formErrors.action)}>
              <FieldLabel htmlFor="server-route-action">动作</FieldLabel>
              <Controller
                control={form.control}
                name="action"
                render={({ field }) => (
                  <NodeSelect
                    value={field.value}
                    placeholder="请选择动作"
                    options={ROUTE_ACTION_OPTIONS}
                    onChange={(value) => {
                      const nextAction = value as ServerRouteAction;
                      field.onChange(nextAction);
                      if (!['dns', 'route', 'route_ip', 'default_out'].includes(nextAction)) {
                        form.setValue('action_value', null);
                      }
                      if (nextAction === 'default_out') form.setValue('match', '');
                    }}
                    testId="server-route-action"
                  />
                )}
              />
              <FieldError errors={[formErrors.action]} />
            </Field>

            {action === 'dns' ? (
              <Field>
                <FieldLabel htmlFor="server-route-dns">DNS服务器</FieldLabel>
                <Input
                  {...form.register('action_value')}
                  id="server-route-dns"
                  placeholder="请输入用于解析的DNS服务器地址"
                  data-testid="server-route-action-value"
                />
              </Field>
            ) : null}

            {action === 'route' || action === 'route_ip' || action === 'default_out' ? (
              <Field>
                <FieldLabel htmlFor="server-route-outbound" className="flex items-center gap-2">
                  Xray出站配置
                  <a
                    className="inline-flex items-center gap-1 text-primary"
                    href="https://xtls.github.io/config/outbound.html"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <ExternalLink className="size-3.5" />
                    填写参考
                  </a>
                </FieldLabel>
                <Textarea
                  {...form.register('action_value')}
                  id="server-route-outbound"
                  rows={8}
                  className="font-mono text-xs"
                  placeholder={JSON.stringify(
                    {
                      tag: 'ss_out',
                      sendThrough: '0.0.0.0',
                      protocol: 'shadowsocks',
                      settings: {
                        email: 'love@xray.com',
                        address: '8.8.8.8',
                        port: 5555,
                        method: 'chacha20-ietf-poly1305',
                        password: 'abcdefghijklmnopqrstuvwxyz',
                        level: 0,
                      },
                    },
                    null,
                    4,
                  )}
                  data-testid="server-route-action-value"
                />
              </Field>
            ) : null}
          </div>

          <DialogFooter className="mt-4">
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              type="submit"
              disabled={pending || isSubmitting}
              data-testid="server-route-submit"
            >
              {pending || isSubmitting ? (
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

function getServerRouteFormValues(route?: admin.ServerRoute): ServerRouteFormValues {
  const routeAction = route?.action;
  const action = isServerRouteAction(routeAction) ? routeAction : 'block';
  return {
    id: route?.id,
    remarks: route?.remarks ?? '',
    match: getRouteMatchTextareaValue(route?.match) ?? '',
    action,
    action_value: route?.action_value ?? null,
  };
}

function isServerRouteAction(value: unknown): value is ServerRouteAction {
  return SERVER_ROUTE_ACTIONS.some((action) => action === value);
}
