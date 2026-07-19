import { useState, type ReactElement } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import { Loader2 } from 'lucide-react';
import type { Plan } from '@v2board/types';
import { Button } from '@/components/ui/button';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from '@/components/ui/input-group';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '@/components/ui/sheet';
import { giftcardEditorSchema, type GiftcardEditorValues } from './form-schema';
import {
  downloadGeneratedCsv,
  fromDateTimeLocal,
  normalizeGenerationPayload,
  planOptions,
  rfc3339ToUnixInput,
  toDateTimeLocal,
  type GenerateResponse,
  type GiftcardRow,
  type GiftcardSubmit,
} from './shared';

function giftcardValueUnit(type: GiftcardSubmit['type']) {
  switch (type) {
    case 1:
      return '¥';
    case 2:
      return '天';
    case 3:
      return 'GB';
    case 4:
      return '';
    case 5:
      return '天';
    default:
      return '';
  }
}

export function GiftcardEditor({
  record,
  plans,
  pending,
  onSave,
  children,
}: {
  record?: GiftcardRow;
  plans: Plan[];
  pending: boolean;
  onSave: (payload: GiftcardSubmit, onSuccess: (response?: GenerateResponse) => void) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const form = useForm<GiftcardEditorValues>({
    resolver: zodResolver(giftcardEditorSchema),
    defaultValues: {
      type: record?.type ?? 1,
      id: record?.id,
      name: record?.name,
      code: record?.code,
      value: record?.value ?? undefined,
      plan_id: record?.plan_id ?? null,
      started_at: rfc3339ToUnixInput(record?.started_at),
      ended_at: rfc3339ToUnixInput(record?.ended_at),
      limit_use: record?.limit_use ?? null,
      generate_count: undefined,
    },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  const values = useWatch({ control: form.control });

  const openSheet = () => {
    form.reset({
      type: record?.type ?? 1,
      id: record?.id,
      name: record?.name,
      code: record?.code,
      value: record?.value ?? undefined,
      plan_id: record?.plan_id ?? null,
      started_at: rfc3339ToUnixInput(record?.started_at),
      ended_at: rfc3339ToUnixInput(record?.ended_at),
      limit_use: record?.limit_use ?? null,
      generate_count: undefined,
    });
    setOpen(true);
  };

  const save = form.handleSubmit((validValues) => {
    onSave(normalizeGenerationPayload(validValues) as GiftcardSubmit, (response) => {
      if (validValues.generate_count && response?.buffer)
        downloadGeneratedCsv('GIFTCARD', response.buffer);
      setOpen(false);
    });
  });

  return (
    <Sheet
      open={open}
      onOpenChange={(nextOpen) => {
        if (nextOpen) openSheet();
        else setOpen(false);
      }}
    >
      <SheetTrigger asChild>{children}</SheetTrigger>
      <SheetContent
        side="right"
        className="w-full gap-0 overflow-y-auto sm:max-w-md"
        data-testid="giftcard-editor"
      >
        <SheetHeader>
          <SheetTitle>{record?.id ? '编辑礼品卡' : '新建礼品卡'}</SheetTitle>
          <SheetDescription>设置礼品卡额度、订阅计划、数量和有效期。</SheetDescription>
        </SheetHeader>

        <form id="giftcard-editor-form" className="space-y-4 px-4 pb-4" onSubmit={save} noValidate>
          <Field data-invalid={Boolean(formErrors.name)}>
            <FieldLabel htmlFor="giftcard-name">名称</FieldLabel>
            <Input
              id="giftcard-name"
              placeholder="请输入礼品卡名称"
              aria-invalid={Boolean(formErrors.name)}
              {...form.register('name')}
              data-testid="giftcard-name"
            />
            <FieldError errors={[formErrors.name]} />
          </Field>

          {!values.generate_count ? (
            <Field>
              <FieldLabel htmlFor="giftcard-code">自定义礼品卡卡密</FieldLabel>
              <Input
                id="giftcard-code"
                placeholder="自定义礼品卡卡密(留空随机生成)"
                {...form.register('code', {
                  onChange: () => form.setValue('generate_count', undefined),
                })}
                data-testid="giftcard-code"
              />
            </Field>
          ) : null}

          <Field data-invalid={Boolean(formErrors.value)}>
            <FieldLabel htmlFor="giftcard-value">礼品卡类型</FieldLabel>
            <div className="flex gap-2">
              <Select
                value={String(values.type ?? 1)}
                onValueChange={(value) =>
                  form.setValue('type', Number(value) as GiftcardEditorValues['type'])
                }
              >
                <SelectTrigger
                  className="w-40 shrink-0"
                  data-testid="giftcard-type"
                  aria-label="礼品卡类型"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">增加账户余额</SelectItem>
                  <SelectItem value="2">增加订阅时长</SelectItem>
                  <SelectItem value="3">增加套餐流量</SelectItem>
                  <SelectItem value="4">重置套餐流量</SelectItem>
                  <SelectItem value="5">兑换订阅套餐</SelectItem>
                </SelectContent>
              </Select>
              <InputGroup className="flex-1">
                <InputGroupInput
                  id="giftcard-value"
                  type="number"
                  step={values.type === 1 ? '0.01' : '1'}
                  disabled={values.type === 4}
                  placeholder={values.type === 5 ? '一次性套餐输入0' : '请输入值'}
                  value={values.type === 4 ? 0 : (values.value ?? '')}
                  onChange={(event) => form.setValue('value', event.target.value)}
                  aria-invalid={Boolean(formErrors.value)}
                  data-testid="giftcard-value"
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupText>{giftcardValueUnit(values.type)}</InputGroupText>
                </InputGroupAddon>
              </InputGroup>
            </div>
            <FieldError errors={[formErrors.value]} />
          </Field>

          {values.type === 5 ? (
            <Field data-invalid={Boolean(formErrors.plan_id)}>
              <FieldLabel htmlFor="giftcard-plan">指定订阅</FieldLabel>
              <Select
                value={values.plan_id != null ? String(values.plan_id) : ''}
                onValueChange={(value) => form.setValue('plan_id', value || null)}
              >
                <SelectTrigger
                  id="giftcard-plan"
                  className="w-full"
                  aria-invalid={Boolean(formErrors.plan_id)}
                  data-testid="giftcard-plan"
                >
                  <SelectValue placeholder="指定订阅" />
                </SelectTrigger>
                <SelectContent>
                  {planOptions(plans).map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <FieldError errors={[formErrors.plan_id]} />
            </Field>
          ) : null}

          <div className="grid grid-cols-2 gap-3">
            <Field data-invalid={Boolean(formErrors.started_at)}>
              <FieldLabel htmlFor="giftcard-start">开始时间</FieldLabel>
              <Controller
                control={form.control}
                name="started_at"
                render={({ field }) => (
                  <Input
                    id="giftcard-start"
                    name={field.name}
                    type="datetime-local"
                    value={toDateTimeLocal(field.value)}
                    onChange={(event) =>
                      form.setValue('started_at', fromDateTimeLocal(event.target.value), {
                        shouldDirty: true,
                        shouldValidate: true,
                      })
                    }
                    onBlur={field.onBlur}
                    ref={field.ref}
                    aria-invalid={Boolean(formErrors.started_at)}
                    data-testid="giftcard-start"
                  />
                )}
              />
              <FieldError errors={[formErrors.started_at]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.ended_at)}>
              <FieldLabel htmlFor="giftcard-end">结束时间</FieldLabel>
              <Controller
                control={form.control}
                name="ended_at"
                render={({ field }) => (
                  <Input
                    id="giftcard-end"
                    name={field.name}
                    type="datetime-local"
                    value={toDateTimeLocal(field.value)}
                    onChange={(event) =>
                      form.setValue('ended_at', fromDateTimeLocal(event.target.value), {
                        shouldDirty: true,
                        shouldValidate: true,
                      })
                    }
                    onBlur={field.onBlur}
                    ref={field.ref}
                    aria-invalid={Boolean(formErrors.ended_at)}
                    data-testid="giftcard-end"
                  />
                )}
              />
              <FieldError errors={[formErrors.ended_at]} />
            </Field>
          </div>

          <Field data-invalid={Boolean(formErrors.limit_use)}>
            <FieldLabel htmlFor="giftcard-limit-use">最大使用次数</FieldLabel>
            <Input
              id="giftcard-limit-use"
              type="number"
              step="1"
              placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"
              aria-invalid={Boolean(formErrors.limit_use)}
              {...form.register('limit_use')}
              data-testid="giftcard-limit-use"
            />
            <FieldError errors={[formErrors.limit_use]} />
          </Field>

          {!values.code && !values.id ? (
            <Field data-invalid={Boolean(formErrors.generate_count)}>
              <FieldLabel htmlFor="giftcard-generate-count">生成数量</FieldLabel>
              <Input
                id="giftcard-generate-count"
                type="number"
                min="1"
                max="500"
                step="1"
                placeholder="输入数量批量生成"
                aria-invalid={Boolean(formErrors.generate_count)}
                {...form.register('generate_count', {
                  onChange: () => form.setValue('code', undefined),
                })}
                data-testid="giftcard-generate-count"
              />
              <FieldError errors={[formErrors.generate_count]} />
            </Field>
          ) : null}
        </form>

        <SheetFooter>
          <Button
            type="submit"
            form="giftcard-editor-form"
            disabled={pending}
            data-testid="giftcard-submit"
          >
            {pending ? (
              <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            提交
          </Button>
          <Button variant="outline" onClick={() => setOpen(false)}>
            取消
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
