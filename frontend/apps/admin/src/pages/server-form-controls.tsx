import type { ReactNode } from 'react';
import { get, type FieldErrors, type FieldPath, type UseFormSetValue } from 'react-hook-form';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';
import { FieldError } from '@/components/ui/field';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { cn } from '@/lib/cn';
import {
  AVAILABLE_STATUS_DOT,
  SERVER_TYPE_BADGE_CLASSES,
  getAvailableStatus,
  type ServerType,
  type SelectOption,
  type SelectValueType,
} from './server-domain';
import type { ServerNodeEditorValues } from './server-form-schema';

export interface NodeForm {
  values: ServerNodeEditorValues;
  errors: FieldErrors<ServerNodeEditorValues>;
  setField: UseFormSetValue<ServerNodeEditorValues>;
  setFieldOptions: {
    shouldDirty: boolean;
    shouldValidate: boolean;
  };
  replaceValues: (values: ServerNodeEditorValues) => void;
}

export type NodeAdvancedField =
  | 'network_settings'
  | 'networkSettings'
  | 'padding_scheme'
  | 'tls_settings'
  | 'tlsSettings'
  | 'encryption_settings';

export function NodeFieldError({
  form,
  name,
}: {
  form: NodeForm;
  name: FieldPath<ServerNodeEditorValues>;
}) {
  const candidate: unknown = get(form.errors, name);
  if (!candidate || typeof candidate !== 'object' || !('message' in candidate)) return null;
  const message = candidate.message;
  return <FieldError errors={[typeof message === 'string' ? { message } : undefined]} />;
}

// Round-trips a typed (string | number | null) select value through Radix Select,
// which only speaks non-empty strings, by keying options on their index and mapping
// back to the original typed value on change so the Zod output transform keeps
// receiving the backend's exact scalar type.
export function NodeSelect({
  value,
  options,
  placeholder,
  onChange,
  className,
  id,
  testId,
}: {
  value: SelectValueType;
  options: SelectOption[];
  placeholder?: string;
  onChange: (value: string | number | null) => void;
  className?: string;
  id?: string;
  testId?: string;
}) {
  const selectedIndex = options.findIndex((option) => option.value === value);
  return (
    <Select
      value={selectedIndex >= 0 ? String(selectedIndex) : ''}
      onValueChange={(next) => {
        const option = options[Number(next)];
        onChange(option ? option.value : null);
      }}
    >
      <SelectTrigger
        id={id ?? testId}
        className={cn('w-full', className)}
        data-testid={testId}
        aria-label={!id && !testId ? placeholder : undefined}
      >
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
        {options.map((option, index) => (
          <SelectItem key={index} value={String(index)}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function MultiCheckboxField({
  options,
  value,
  onChange,
  testId,
  emptyText,
}: {
  options: { value: string; label: string }[];
  value: string[];
  onChange: (value: string[]) => void;
  testId?: string;
  emptyText?: string;
}) {
  if (!options.length) {
    return <p className="text-sm text-muted-foreground">{emptyText ?? '暂无可选项'}</p>;
  }
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

export function ServerTypeTag({ type, children }: { type: ServerType; children: ReactNode }) {
  return (
    <Badge className={cn('border-transparent', SERVER_TYPE_BADGE_CLASSES[type])}>{children}</Badge>
  );
}

export function AvailabilityDot({ status }: { status?: number | null }) {
  const tone = getAvailableStatus(status);
  if (!tone) return null;
  return (
    <span
      aria-hidden="true"
      className={cn('inline-block size-2 shrink-0 rounded-full', AVAILABLE_STATUS_DOT[tone])}
    />
  );
}
