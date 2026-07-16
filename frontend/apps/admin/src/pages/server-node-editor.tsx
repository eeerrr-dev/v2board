import { useCallback, useMemo, useState } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import { ExternalLink, Loader2 } from 'lucide-react';
import type { admin } from '@v2board/api-client';
import { Button } from '@/components/ui/button';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { TagsInput } from '@/components/ui/tags-input';
import { Textarea } from '@/components/ui/textarea';
import { useSaveServerMutation } from '@/lib/queries';
import {
  ENCRYPTION_SETTINGS_DEFAULTS,
  normalizeNullableArray,
  normalizeSettings,
  type SelectOption,
} from './server-domain';
import { getNodeInitialValues, inputValue, selectValue } from './server-node-values';
import {
  MultiCheckboxField,
  NodeFieldError,
  NodeSelect,
  type NodeAdvancedField,
  type NodeForm,
} from './server-form-controls';
import {
  serverNodeFormSchema,
  type ServerNodeEditorValues,
  type ServerNodeSaveRequest,
} from './server-form-schema';
import {
  NodeAddressFields,
  NodeChildField,
  NodePortFields,
  ServerTypeFields,
} from './server-protocol-fields';

export function NodeEditor({
  open,
  type,
  record,
  nodes,
  groups,
  routes,
  dependenciesReady,
  onClose,
}: {
  open: boolean;
  type: admin.ServerTypeName;
  record?: admin.ServerNode;
  nodes: admin.ServerNode[];
  groups: admin.ServerGroup[];
  routes: admin.ServerRoute[];
  dependenciesReady: boolean;
  onClose: () => void;
}) {
  const saveServer = useSaveServerMutation();
  const id = record?.id;
  const nodeForm = useForm<ServerNodeEditorValues, unknown, ServerNodeSaveRequest>({
    resolver: zodResolver(serverNodeFormSchema),
    defaultValues: getNodeInitialValues(type, record),
  });
  // useFormState, not the mutable nodeForm.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: nodeFormErrors, isSubmitting, isSubmitted } = useFormState({ control: nodeForm.control });
  const values = useWatch({
    control: nodeForm.control,
    compute: (formValues) => formValues,
  });
  const [childDrawer, setChildDrawer] = useState<{
    open: boolean;
    title?: string;
    field?: NodeAdvancedField;
  }>({ open: false });
  const { reset, setValue: setField } = nodeForm;
  const setFieldOptions = useMemo<NodeForm['setFieldOptions']>(
    () => ({
      shouldDirty: true,
      shouldValidate: isSubmitted,
    }),
    [isSubmitted],
  );
  const replaceValues = useCallback<NodeForm['replaceValues']>(
    (nextValues) => reset(nextValues),
    [reset],
  );
  const form = useMemo<NodeForm>(
    () => ({
      values,
      errors: nodeFormErrors,
      setField,
      setFieldOptions,
      replaceValues,
    }),
    [nodeFormErrors, replaceValues, setField, setFieldOptions, values],
  );

  const parentCandidates = nodes.filter((node) => node.type === type && node.id !== id);
  const parentOptions: SelectOption[] = [
    { value: '', label: '无' },
    ...parentCandidates.map((node) => ({ value: node.id, label: node.name })),
  ];
  const groupOptions = groups.map((group) => ({ value: String(group.id), label: group.name }));
  const routeOptions = routes.map((route) => ({
    value: String(route.id),
    label: String(route.id),
  }));

  const showChildDrawer = (title?: string, field?: NodeAdvancedField) => {
    if (!childDrawer.open && field === 'encryption_settings') {
      if (values.type === 'vless') {
        setField(
          'encryption_settings',
          normalizeSettings(values.encryption_settings, ENCRYPTION_SETTINGS_DEFAULTS),
          setFieldOptions,
        );
      } else if (values.type === 'v2node' && values.config.protocol === 'vless') {
        setField(
          'config.encryption_settings',
          normalizeSettings(values.config.encryption_settings, ENCRYPTION_SETTINGS_DEFAULTS),
          setFieldOptions,
        );
      }
    }
    setChildDrawer((current) => ({ open: !current.open, title, field }));
  };

  const submit = nodeForm.handleSubmit((request) => {
    if (!dependenciesReady || request.type !== type) return;
    saveServer.mutate(request, { onSuccess: onClose });
  });

  return (
    <Sheet open={open} onOpenChange={(next) => (next ? undefined : onClose())}>
      <SheetContent
        side="right"
        className="w-full gap-0 overflow-y-auto sm:max-w-3xl"
        data-testid="node-editor"
      >
        <form className="contents" onSubmit={(event) => void submit(event)}>
          <SheetHeader>
            <SheetTitle>{id ? '编辑节点' : '新建节点'}</SheetTitle>
            <SheetDescription>配置节点协议、连接参数、权限组和路由规则。</SheetDescription>
          </SheetHeader>

          <div className="space-y-5 px-4 pb-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              <Field
                className="sm:col-span-2"
                data-invalid={Boolean(nodeFormErrors.name)}
              >
                <FieldLabel htmlFor="node-name">节点名称</FieldLabel>
                <Input
                  {...nodeForm.register('name')}
                  id="node-name"
                  placeholder="请输入节点名称"
                  aria-invalid={Boolean(nodeFormErrors.name)}
                  data-testid="node-name"
                />
                <FieldError errors={[nodeFormErrors.name]} />
              </Field>
              <Field data-invalid={Boolean(nodeFormErrors.rate)}>
                <FieldLabel htmlFor="node-rate">倍率</FieldLabel>
                <div className="relative">
                  <Input
                    {...nodeForm.register('rate')}
                    id="node-rate"
                    className="pr-8"
                    placeholder="请输入节点倍率"
                    aria-invalid={Boolean(nodeFormErrors.rate)}
                    data-testid="node-rate"
                  />
                  <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                    x
                  </span>
                </div>
                <FieldError errors={[nodeFormErrors.rate]} />
              </Field>
            </div>

            <Field data-invalid={Boolean(nodeFormErrors.tags)}>
              <FieldLabel htmlFor="node-tags">节点标签</FieldLabel>
              <Controller
                control={nodeForm.control}
                name="tags"
                render={({ field }) => (
                  <TagsInput
                    id="node-tags"
                    data-testid="node-tags"
                    value={Array.isArray(field.value) ? field.value : []}
                    onChange={(next) => field.onChange(normalizeNullableArray(next))}
                    onBlur={field.onBlur}
                    invalid={Boolean(nodeFormErrors.tags)}
                    placeholder="输入后回车添加标签"
                  />
                )}
              />
            </Field>

            <fieldset
              className="min-w-0 space-y-2"
              data-invalid={Boolean(nodeFormErrors.group_id)}
            >
              <legend className="text-sm font-medium text-foreground">权限组</legend>
              <Controller
                control={nodeForm.control}
                name="group_id"
                render={({ field }) => (
                  <MultiCheckboxField
                    options={groupOptions}
                    value={Array.isArray(field.value) ? field.value.map(String) : []}
                    onChange={field.onChange}
                    testId="node-group-ids"
                    emptyText="暂无可选权限组"
                  />
                )}
              />
              <FieldError errors={[nodeFormErrors.group_id]} />
            </fieldset>

            <NodeAddressFields form={form} showChildDrawer={showChildDrawer} />
            <NodePortFields form={form} />

            <ServerTypeFields editing={Boolean(id)} form={form} showChildDrawer={showChildDrawer} />

            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Label htmlFor="node-parent">父节点</Label>
                <a
                  className="inline-flex items-center gap-1 text-sm text-primary"
                  target="_blank"
                  href="https://docs.v2board.com/use/node.html#父节点与子节点关系"
                  rel="noopener noreferrer"
                >
                  <ExternalLink className="size-3.5" />
                  更多解答
                </a>
              </div>
              <Controller
                control={nodeForm.control}
                name="parent_id"
                render={({ field }) => (
                  <NodeSelect
                    value={selectValue(field.value) || ''}
                    options={parentOptions}
                    onChange={field.onChange}
                    testId="node-parent"
                  />
                )}
              />
            </div>

            <fieldset className="min-w-0 space-y-2">
              <legend className="text-sm font-medium text-foreground">路由组</legend>
              <Controller
                control={nodeForm.control}
                name="route_id"
                render={({ field }) => (
                  <MultiCheckboxField
                    options={routeOptions}
                    value={Array.isArray(field.value) ? field.value.map(String) : []}
                    onChange={(next) => field.onChange(normalizeNullableArray(next.map(Number)))}
                    testId="node-route-ids"
                    emptyText="暂无可选路由组"
                  />
                )}
              />
            </fieldset>

            {values.type === 'v2node' ? (
              <div className="space-y-2">
                <Label htmlFor="node-install-command">一键安装指令</Label>
                <Textarea
                  id="node-install-command"
                  rows={4}
                  readOnly
                  className="cursor-text bg-muted/40 font-mono text-xs"
                  value={inputValue(values.install_command)}
                  data-testid="node-install-command"
                />
              </div>
            ) : null}

            <div data-testid="node-form-errors">
              {values.type === 'v2node' ? (
                <>
                  <NodeFieldError form={form} name="config.network_settings" />
                  <NodeFieldError form={form} name="config.padding_scheme" />
                </>
              ) : (
                <>
                  <NodeFieldError form={form} name="networkSettings" />
                  <NodeFieldError form={form} name="network_settings" />
                  <NodeFieldError form={form} name="padding_scheme" />
                </>
              )}
            </div>
          </div>

          <SheetFooter>
            <Button
              type="submit"
              disabled={!dependenciesReady || isSubmitting}
              data-testid="node-submit"
            >
              {isSubmitting ? (
                <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
              ) : null}
              提交
            </Button>
            <Button type="button" variant="outline" onClick={onClose}>
              取消
            </Button>
          </SheetFooter>
        </form>
      </SheetContent>

      {childDrawer.field ? (
        <Sheet
          open={childDrawer.open}
          onOpenChange={(next) => (next ? undefined : showChildDrawer())}
        >
          <SheetContent
            side="right"
            className="w-full gap-0 overflow-y-auto sm:max-w-2xl"
            data-testid="node-child-editor"
          >
            <SheetHeader>
              <SheetTitle>{childDrawer.title}</SheetTitle>
              <SheetDescription>编辑当前节点的高级协议参数。</SheetDescription>
            </SheetHeader>
            <div className="space-y-4 px-4 pb-4">
              <NodeChildField field={childDrawer.field} form={form} />
            </div>
            <SheetFooter>
              <Button type="button" onClick={() => showChildDrawer()}>
                完成
              </Button>
            </SheetFooter>
          </SheetContent>
        </Sheet>
      ) : null}
    </Sheet>
  );
}
