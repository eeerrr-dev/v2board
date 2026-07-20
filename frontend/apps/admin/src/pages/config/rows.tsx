import type { ReactNode } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { Controller } from 'react-hook-form';
import { cn } from '@v2board/ui/cn';
import { Alert, AlertDescription } from '@v2board/ui/alert';
import { Button } from '@v2board/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@v2board/ui/card';
import { Field, FieldError } from '@v2board/ui/field';
import { Input } from '@v2board/ui/input';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@v2board/ui/select';
import { Switch } from '@v2board/ui/switch';
import { Textarea } from '@v2board/ui/textarea';
import type {
  ConfigFieldValue,
  ConfigFieldName,
  ConfigGroupField,
  ConfigGroupFieldWithValue,
  ConfigGroupKey,
  FormCtx,
} from './schema';
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

function ResetToDefault({
  field,
  disabled,
  pending,
  onReset,
}: {
  field: ConfigFieldName;
  disabled: boolean;
  pending: boolean;
  onReset: () => void;
}) {
  const { t } = useTranslation();
  // The backend deliberately rejects a null/empty secure_path; changing this
  // security-sensitive route always requires an explicit replacement.
  if (field === 'secure_path') return null;

  return (
    <div className="mt-1 flex min-h-7 items-center justify-between gap-2">
      <span className="text-xs text-muted-foreground" aria-live="polite">
        {pending ? t(($) => $.admin.config.reset_default_pending) : null}
      </span>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="h-7 px-2 text-xs"
        disabled={disabled}
        onClick={onReset}
        data-testid={`config-${field}-reset-default`}
      >
        {t(($) => $.admin.config.reset_default)}
      </Button>
    </div>
  );
}

export function SwitchRow<Group extends ConfigGroupKey>({
  ctx,
  group,
  field,
  title,
  description,
  indent,
}: {
  ctx: FormCtx;
  group: Group;
  field: ConfigGroupFieldWithValue<Group, boolean>;
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
                  // §4.1: config flags stay real booleans in the local draft.
                  controlField.onChange(ctx.stage(group, field, checked));
                }}
                aria-label={title}
                aria-invalid={fieldState.invalid}
                data-testid={`config-${field}`}
                disabled={ctx.isSaving(field)}
              />
            </div>
            <ResetToDefault
              field={field}
              disabled={ctx.isSaving(field)}
              pending={fieldState.isDirty && controlField.value === null}
              onReset={() => controlField.onChange(ctx.stage(group, field, null))}
            />
            <FieldError errors={[fieldState.error]} />
          </Field>
        </SettingRow>
      )}
    />
  );
}

export function TextRow<Group extends ConfigGroupKey>({
  ctx,
  group,
  field,
  title,
  description,
  placeholder,
  type,
  suffix,
  indent,
}: {
  ctx: FormCtx;
  group: Group;
  field: ConfigGroupFieldWithValue<Group, string | number | string[]>;
  title: string;
  description?: string;
  placeholder?: string;
  type?: string;
  suffix?: string;
  indent?: boolean;
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
                disabled={ctx.isSaving(field)}
                value={toText(controlField.value)}
                onChange={(event) => controlField.onChange(event.target.value)}
                onBlur={(event) => {
                  controlField.onBlur();
                  controlField.onChange(ctx.stage(group, field, event.target.value));
                }}
              />
              {suffix ? (
                <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                  {suffix}
                </span>
              ) : null}
            </div>
            <ResetToDefault
              field={field}
              disabled={ctx.isSaving(field)}
              pending={fieldState.isDirty && controlField.value === null}
              onReset={() => controlField.onChange(ctx.stage(group, field, null))}
            />
            <FieldError errors={[fieldState.error]} />
          </Field>
        </SettingRow>
      )}
    />
  );
}

export function TextareaRow<Group extends ConfigGroupKey>({
  ctx,
  group,
  field,
  title,
  description,
  placeholder,
  rows,
  indent,
}: {
  ctx: FormCtx;
  group: Group;
  field: ConfigGroupFieldWithValue<Group, string | number | string[]>;
  title: string;
  description?: string;
  placeholder?: string;
  rows: number;
  indent?: boolean;
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
              disabled={ctx.isSaving(field)}
              value={toText(controlField.value)}
              onChange={(event) => controlField.onChange(event.target.value)}
              onBlur={(event) => {
                controlField.onBlur();
                controlField.onChange(ctx.stage(group, field, event.target.value));
              }}
            />
            <ResetToDefault
              field={field}
              disabled={ctx.isSaving(field)}
              pending={fieldState.isDirty && controlField.value === null}
              onReset={() => controlField.onChange(ctx.stage(group, field, null))}
            />
            <FieldError errors={[fieldState.error]} />
          </Field>
        </SettingRow>
      )}
    />
  );
}

export function SelectRow<Group extends ConfigGroupKey>({
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
  group: Group;
  field: ConfigGroupField<Group>;
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
                disabled={ctx.isSaving(field)}
                onValueChange={(value) => {
                  const wireValue = serialize ? serialize(value) : value;
                  controlField.onChange(ctx.stage(group, field, wireValue));
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
              <ResetToDefault
                field={field}
                disabled={ctx.isSaving(field)}
                pending={fieldState.isDirty && controlField.value === null}
                onReset={() => controlField.onChange(ctx.stage(group, field, null))}
              />
              <FieldError errors={[fieldState.error]} />
            </Field>
          </SettingRow>
        );
      }}
    />
  );
}

// The '0'/'1' option ids are the legacy wire spellings — data, not copy.
export function orderEventOptions(t: TFunction) {
  return [
    { value: '0', label: t(($) => $.admin.config.order_event_none) },
    { value: '1', label: t(($) => $.admin.config.order_event_reset_traffic) },
  ];
}

export function WarningAlert({ children }: { children: ReactNode }) {
  return (
    <Alert className="border-warning/30 bg-warning/10 text-warning">
      <AlertDescription className="text-warning">{children}</AlertDescription>
    </Alert>
  );
}
