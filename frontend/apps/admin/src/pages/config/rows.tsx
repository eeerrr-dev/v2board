import type { ReactNode } from 'react';
import { Controller } from 'react-hook-form';
import { cn } from '@/lib/cn';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Field, FieldError } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import type { ConfigFieldValue, ConfigGroupKey, FormCtx } from './schema';
import { isBackendEnabled, toText } from './values';

// --- Shared field primitives ----------------------------------------------

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="divide-y divide-border">{children}</CardContent>
    </Card>
  );
}

export function SettingRow({
  title,
  description,
  indent,
  children,
}: {
  title: string;
  description?: string;
  indent?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        'flex flex-col gap-2 py-4 sm:flex-row sm:items-start sm:justify-between sm:gap-6',
        indent && 'sm:pl-6',
      )}
    >
      <div className="space-y-1 sm:max-w-md">
        <div className="text-sm font-medium text-foreground">{title}</div>
        {description ? (
          <p className="text-xs leading-5 text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <div className="w-full sm:w-72 sm:shrink-0">{children}</div>
    </div>
  );
}

export function SwitchRow({
  ctx,
  group,
  field,
  title,
  description,
  indent,
}: {
  ctx: FormCtx;
  group: ConfigGroupKey;
  field: string;
  title: string;
  description?: string;
  indent?: boolean;
}) {
  return (
    <Controller
      control={ctx.control}
      name={field}
      render={({ field: controlField, fieldState }) => (
        <SettingRow title={title} description={description} indent={indent}>
          <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(field)}>
            <div className="flex h-10 items-center sm:justify-end">
              <Switch
                ref={controlField.ref}
                name={controlField.name}
                checked={isBackendEnabled(controlField.value)}
                onBlur={controlField.onBlur}
                onCheckedChange={(checked) => {
                  // §4.1: config flags are real JSON booleans on the wire.
                  controlField.onChange(checked);
                  void ctx.save(group, field, checked);
                }}
                aria-label={title}
                aria-invalid={fieldState.invalid}
                data-testid={`config-${field}`}
              />
            </div>
            <FieldError errors={[fieldState.error]} />
          </Field>
        </SettingRow>
      )}
    />
  );
}

export function TextRow({
  ctx,
  group,
  field,
  title,
  description,
  placeholder,
  type,
  suffix,
  indent,
  coerce,
}: {
  ctx: FormCtx;
  group: ConfigGroupKey;
  field: string;
  title: string;
  description?: string;
  placeholder?: string;
  type?: string;
  suffix?: string;
  indent?: boolean;
  coerce?: (value: string) => ConfigFieldValue;
}) {
  return (
    <Controller
      control={ctx.control}
      name={field}
      render={({ field: controlField, fieldState }) => (
        <SettingRow title={title} description={description} indent={indent}>
          <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(field)}>
            <div className={suffix ? 'relative' : undefined}>
              <Input
                ref={controlField.ref}
                name={controlField.name}
                type={type}
                className={suffix ? 'pr-10' : undefined}
                placeholder={placeholder}
                aria-label={title}
                aria-invalid={fieldState.invalid}
                data-testid={`config-${field}`}
                disabled={field === 'secure_path' && ctx.isSaving(field)}
                value={toText(controlField.value)}
                onChange={(event) => controlField.onChange(event.target.value)}
                onBlur={(event) => {
                  controlField.onBlur();
                  void ctx.save(
                    group,
                    field,
                    coerce ? coerce(event.target.value) : event.target.value,
                  );
                }}
              />
              {suffix ? (
                <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                  {suffix}
                </span>
              ) : null}
            </div>
            <FieldError errors={[fieldState.error]} />
          </Field>
        </SettingRow>
      )}
    />
  );
}

export function TextareaRow({
  ctx,
  group,
  field,
  title,
  description,
  placeholder,
  rows,
  indent,
  coerce,
}: {
  ctx: FormCtx;
  group: ConfigGroupKey;
  field: string;
  title: string;
  description?: string;
  placeholder?: string;
  rows: number;
  indent?: boolean;
  coerce?: (value: string) => ConfigFieldValue;
}) {
  return (
    <Controller
      control={ctx.control}
      name={field}
      render={({ field: controlField, fieldState }) => (
        <SettingRow title={title} description={description} indent={indent}>
          <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(field)}>
            <Textarea
              ref={controlField.ref}
              name={controlField.name}
              rows={rows}
              placeholder={placeholder}
              aria-label={title}
              aria-invalid={fieldState.invalid}
              data-testid={`config-${field}`}
              value={toText(controlField.value)}
              onChange={(event) => controlField.onChange(event.target.value)}
              onBlur={(event) => {
                controlField.onBlur();
                void ctx.save(
                  group,
                  field,
                  coerce ? coerce(event.target.value) : event.target.value,
                );
              }}
            />
            <FieldError errors={[fieldState.error]} />
          </Field>
        </SettingRow>
      )}
    />
  );
}

export function SelectRow({
  ctx,
  group,
  field,
  title,
  description,
  placeholder,
  options,
  fallback,
  indent,
  serialize,
}: {
  ctx: FormCtx;
  group: ConfigGroupKey;
  field: string;
  title: string;
  description?: string;
  placeholder?: string;
  options: { value: string; label: string }[];
  fallback?: string;
  indent?: boolean;
  /**
   * Maps the picked option value to the saved wire value. Radix Select items
   * cannot carry an empty value, so an "off" option uses a sentinel here and
   * serializes to the backend's empty/clear value.
   */
  serialize?: (value: string) => ConfigFieldValue;
}) {
  return (
    <Controller
      control={ctx.control}
      name={field}
      render={({ field: controlField, fieldState }) => {
        const current =
          controlField.value == null || controlField.value === ''
            ? fallback
            : typeof controlField.value === 'boolean'
              ? // Boolean wire values (order-event toggles) display through
                // their legacy '0'/'1' option ids.
                controlField.value
                ? '1'
                : '0'
              : String(controlField.value);
        return (
          <SettingRow title={title} description={description} indent={indent}>
            <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(field)}>
              <Select
                name={controlField.name}
                value={current}
                onValueChange={(value) => {
                  const wireValue = serialize ? serialize(value) : value;
                  controlField.onChange(wireValue);
                  void ctx.save(group, field, wireValue);
                }}
              >
                <SelectTrigger
                  ref={controlField.ref}
                  className="w-full"
                  aria-label={title}
                  aria-invalid={fieldState.invalid}
                  data-testid={`config-${field}`}
                >
                  <SelectValue placeholder={placeholder} />
                </SelectTrigger>
                <SelectContent>
                  {options.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <FieldError errors={[fieldState.error]} />
            </Field>
          </SettingRow>
        );
      }}
    />
  );
}

export const ORDER_EVENT_OPTIONS = [
  { value: '0', label: '不执行任何动作' },
  { value: '1', label: '重置用户流量' },
];

export function WarningAlert({ children }: { children: ReactNode }) {
  return (
    <Alert className="border-warning/30 bg-warning/10 text-warning">
      <AlertDescription className="text-warning">{children}</AlertDescription>
    </Alert>
  );
}
