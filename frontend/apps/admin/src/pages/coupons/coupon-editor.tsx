import { useState, type ReactElement } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import { Loader2 } from 'lucide-react';
import type { CouponType, Plan } from '@v2board/types';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
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
import { couponEditorSchema, type CouponEditorValues } from '../coupon-form-schema';
import {
  downloadGeneratedCsv,
  fromDateTimeLocal,
  normalizeGenerationPayload,
  planOptions,
  rfc3339ToUnixInput,
  toDateTimeLocal,
  type CouponRow,
  type CouponSubmit,
  type GenerateResponse,
} from './shared';

const PERIOD_TEXT: Record<string, string> = {
  month_price: '月付',
  quarter_price: '季付',
  half_year_price: '半年付',
  year_price: '年付',
  two_year_price: '两年付',
  three_year_price: '三年付',
  onetime_price: '一次性',
  reset_price: '流量重置包',
};

const PERIOD_OPTIONS = Object.keys(PERIOD_TEXT).map((period) => ({
  value: period,
  label: PERIOD_TEXT[period] ?? period,
}));

interface CheckboxGroupProps {
  options: { value: string; label: string }[];
  value: string[];
  onChange: (value: string[]) => void;
  testId?: string;
}

function CheckboxGroup({ options, value, onChange, testId }: CheckboxGroupProps) {
  const toggle = (option: string, checked: boolean) => {
    onChange(checked ? [...value, option] : value.filter((item) => item !== option));
  };
  return (
    <div
      className="flex flex-wrap gap-x-4 gap-y-2 rounded-md border border-input p-3"
      data-testid={testId}
    >
      {options.map((option) => {
        const checked = value.includes(option.value);
        return (
          <label
            key={option.value}
            className="flex cursor-pointer items-center gap-2 text-sm text-foreground"
          >
            <Checkbox
              checked={checked}
              onCheckedChange={(next) => toggle(option.value, next === true)}
            />
            {option.label}
          </label>
        );
      })}
    </div>
  );
}

export function CouponEditor({
  record,
  plans,
  pending,
  onSave,
  children,
}: {
  record?: CouponRow;
  plans: Plan[];
  pending: boolean;
  onSave: (payload: CouponSubmit, onSuccess: (response?: GenerateResponse) => void) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const form = useForm<CouponEditorValues>({
    resolver: zodResolver(couponEditorSchema),
    defaultValues: {
      type: record?.type ?? 1,
      id: record?.id,
      name: record?.name,
      code: record?.code,
      value: record?.value ?? undefined,
      started_at: rfc3339ToUnixInput(record?.started_at),
      ended_at: rfc3339ToUnixInput(record?.ended_at),
      limit_use: record?.limit_use ?? null,
      limit_use_with_user: record?.limit_use_with_user ?? null,
      limit_plan_ids: record?.limit_plan_ids ?? null,
      limit_period: record?.limit_period ?? null,
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
      started_at: rfc3339ToUnixInput(record?.started_at),
      ended_at: rfc3339ToUnixInput(record?.ended_at),
      limit_use: record?.limit_use ?? null,
      limit_use_with_user: record?.limit_use_with_user ?? null,
      limit_plan_ids: record?.limit_plan_ids ?? null,
      limit_period: record?.limit_period ?? null,
      generate_count: undefined,
    });
    setOpen(true);
  };

  const save = form.handleSubmit((validValues) => {
    onSave(normalizeGenerationPayload(validValues) as CouponSubmit, (response) => {
      if (validValues.generate_count && response?.buffer)
        downloadGeneratedCsv('COUPON', response.buffer);
      setOpen(false);
    });
  });

  const selectedPlanIds = (values.limit_plan_ids ?? []).map(String);
  const selectedPeriods = (values.limit_period ?? []).map(String);

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
        data-testid="coupon-editor"
      >
        <SheetHeader>
          <SheetTitle>{record?.id ? '编辑优惠券' : '新建优惠券'}</SheetTitle>
          <SheetDescription>设置优惠额度、使用限制、有效期和适用订阅。</SheetDescription>
        </SheetHeader>

        <form id="coupon-editor-form" className="space-y-4 px-4 pb-4" onSubmit={save} noValidate>
          <Field data-invalid={Boolean(formErrors.name)}>
            <FieldLabel htmlFor="coupon-name">名称</FieldLabel>
            <Input
              id="coupon-name"
              placeholder="请输入优惠券名称"
              aria-invalid={Boolean(formErrors.name)}
              {...form.register('name')}
              data-testid="coupon-name"
            />
            <FieldError errors={[formErrors.name]} />
          </Field>

          {!values.generate_count ? (
            <Field>
              <FieldLabel htmlFor="coupon-code">自定义优惠券码</FieldLabel>
              <Input
                id="coupon-code"
                placeholder="自定义优惠券码(留空随机生成)"
                {...form.register('code', {
                  onChange: () => form.setValue('generate_count', undefined),
                })}
                data-testid="coupon-code"
              />
            </Field>
          ) : null}

          <Field data-invalid={Boolean(formErrors.value)}>
            <FieldLabel htmlFor="coupon-value">优惠信息</FieldLabel>
            <div className="flex gap-2">
              <Select
                value={String(values.type ?? 1)}
                onValueChange={(value) => form.setValue('type', Number(value) as CouponType)}
              >
                <SelectTrigger
                  className="w-36 shrink-0"
                  data-testid="coupon-type"
                  aria-label="优惠类型"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">按金额优惠</SelectItem>
                  <SelectItem value="2">按比例优惠</SelectItem>
                </SelectContent>
              </Select>
              <InputGroup className="flex-1">
                <InputGroupInput
                  id="coupon-value"
                  type="number"
                  step={values.type === 1 ? '0.01' : '1'}
                  placeholder="请输入值"
                  aria-invalid={Boolean(formErrors.value)}
                  {...form.register('value')}
                  data-testid="coupon-value"
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupText>{values.type === 1 ? '¥' : '%'}</InputGroupText>
                </InputGroupAddon>
              </InputGroup>
            </div>
            <FieldError errors={[formErrors.value]} />
          </Field>

          <div className="grid grid-cols-2 gap-3">
            <Field data-invalid={Boolean(formErrors.started_at)}>
              <FieldLabel htmlFor="coupon-start">开始时间</FieldLabel>
              <Controller
                control={form.control}
                name="started_at"
                render={({ field }) => (
                  <Input
                    id="coupon-start"
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
                    data-testid="coupon-start"
                  />
                )}
              />
              <FieldError errors={[formErrors.started_at]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.ended_at)}>
              <FieldLabel htmlFor="coupon-end">结束时间</FieldLabel>
              <Controller
                control={form.control}
                name="ended_at"
                render={({ field }) => (
                  <Input
                    id="coupon-end"
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
                    data-testid="coupon-end"
                  />
                )}
              />
              <FieldError errors={[formErrors.ended_at]} />
            </Field>
          </div>

          <Field data-invalid={Boolean(formErrors.limit_use)}>
            <FieldLabel htmlFor="coupon-limit-use">最大使用次数</FieldLabel>
            <Input
              id="coupon-limit-use"
              type="number"
              step="1"
              placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"
              aria-invalid={Boolean(formErrors.limit_use)}
              {...form.register('limit_use')}
              data-testid="coupon-limit-use"
            />
            <FieldError errors={[formErrors.limit_use]} />
          </Field>

          <Field data-invalid={Boolean(formErrors.limit_use_with_user)}>
            <FieldLabel htmlFor="coupon-limit-use-user">每个用户可使用次数</FieldLabel>
            <Input
              id="coupon-limit-use-user"
              type="number"
              step="1"
              placeholder="限制每个用户可使用次数(为空则不限制)"
              aria-invalid={Boolean(formErrors.limit_use_with_user)}
              {...form.register('limit_use_with_user')}
              data-testid="coupon-limit-use-user"
            />
            <FieldError errors={[formErrors.limit_use_with_user]} />
          </Field>

          <fieldset className="space-y-2">
            <legend className="text-sm font-medium text-foreground">指定订阅</legend>
            {plans.length ? (
              <CheckboxGroup
                options={planOptions(plans)}
                value={selectedPlanIds}
                onChange={(value) => form.setValue('limit_plan_ids', value.length ? value : null)}
                testId="coupon-plan-ids"
              />
            ) : (
              <p className="text-sm text-muted-foreground">暂无可选订阅</p>
            )}
          </fieldset>

          <fieldset className="space-y-2">
            <legend className="text-sm font-medium text-foreground">指定周期</legend>
            <CheckboxGroup
              options={PERIOD_OPTIONS}
              value={selectedPeriods}
              onChange={(value) => form.setValue('limit_period', value.length ? value : null)}
              testId="coupon-periods"
            />
          </fieldset>

          {!values.code && !values.id ? (
            <Field data-invalid={Boolean(formErrors.generate_count)}>
              <FieldLabel htmlFor="coupon-generate-count">生成数量</FieldLabel>
              <Input
                id="coupon-generate-count"
                type="number"
                min="1"
                max="500"
                step="1"
                placeholder="输入数量批量生成"
                aria-invalid={Boolean(formErrors.generate_count)}
                {...form.register('generate_count', {
                  onChange: () => form.setValue('code', undefined),
                })}
                data-testid="coupon-generate-count"
              />
              <FieldError errors={[formErrors.generate_count]} />
            </Field>
          ) : null}
        </form>

        <SheetFooter>
          <Button
            type="submit"
            form="coupon-editor-form"
            disabled={pending}
            data-testid="coupon-submit"
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
