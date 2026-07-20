import { useEffect } from 'react';
import dayjs from 'dayjs';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useFieldArray, useForm, useFormState, useWatch } from 'react-hook-form';
import { Plus, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { AdminFilter } from '@v2board/api-client';
import { Button } from '@v2board/ui/button';
import { Input } from '@v2board/ui/input';
import { Field, FieldError } from '@v2board/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@v2board/ui/select';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@v2board/ui/sheet';
import { userFilterSchema, type UserFilterValues } from './form-schema';
import type { FilterField } from './shared';

export function UserFilterSheet({
  open,
  onOpenChange,
  fields,
  value,
  onApply,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  fields: FilterField[];
  value: AdminFilter[];
  onApply: (filter: AdminFilter[]) => void;
}) {
  const { t } = useTranslation();
  const form = useForm<UserFilterValues>({
    resolver: zodResolver(userFilterSchema),
    defaultValues: { rows: value },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  const {
    fields: filterRows,
    append,
    remove,
  } = useFieldArray({
    control: form.control,
    name: 'rows',
  });
  const rows = useWatch({ control: form.control, name: 'rows' }) ?? [];

  useEffect(() => {
    if (open) form.reset({ rows: value });
  }, [form, open, value]);

  const fieldOf = (key: string) => fields.find((field) => field.key === key) ?? fields[0]!;

  const addRow = () => {
    const field = fields[0]!;
    append({ key: field.key, condition: field.condition[0]!, value: '' });
  };

  const changeField = (index: number, key: string) => {
    const field = fieldOf(key);
    form.setValue(`rows.${index}.key`, key, { shouldDirty: true });
    form.setValue(`rows.${index}.condition`, field.condition[0]!, { shouldDirty: true });
    form.setValue(`rows.${index}.value`, '', { shouldDirty: true, shouldValidate: true });
  };

  const apply = form.handleSubmit(({ rows: nextRows }) => {
    onApply(nextRows);
    onOpenChange(false);
  });

  const reset = () => {
    form.reset({ rows: [] });
    onApply([]);
    onOpenChange(false);
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        className="flex w-full flex-col gap-0 overflow-hidden p-0 sm:max-w-md"
        data-testid="user-filter-sheet"
      >
        <SheetHeader className="border-b border-border px-6 py-4">
          <SheetTitle>{t(($) => $.admin.users.filter)}</SheetTitle>
          <SheetDescription>{t(($) => $.admin.users.filter_description)}</SheetDescription>
        </SheetHeader>

        <div className="flex-1 space-y-4 overflow-y-auto px-6 py-4">
          {filterRows.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              {t(($) => $.admin.users.filter_empty_hint)}
            </p>
          ) : null}
          {filterRows.map((filterRow, index) => {
            const row = rows[index] ?? filterRow;
            const field = fieldOf(row.key);
            const valueError = formErrors.rows?.[index]?.value;
            return (
              <div key={filterRow.id} className="space-y-2 rounded-md border border-border p-3">
                <div className="flex items-center gap-2">
                  <Select value={row.key} onValueChange={(key) => changeField(index, key)}>
                    <SelectTrigger
                      className="flex-1"
                      aria-label={t(($) => $.admin.users.filter_field_label, { index: index + 1 })}
                      data-testid={`user-filter-field-${index}`}
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {fields.map((item) => (
                        <SelectItem key={item.key} value={item.key}>
                          {item.title}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Controller
                    control={form.control}
                    name={`rows.${index}.condition`}
                    render={({ field: conditionField }) => (
                      <Select value={conditionField.value} onValueChange={conditionField.onChange}>
                        <SelectTrigger
                          className="w-24"
                          aria-label={t(($) => $.admin.users.filter_condition_label, {
                            index: index + 1,
                          })}
                          data-testid={`user-filter-condition-${index}`}
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {field.condition.map((condition) => (
                            <SelectItem key={condition} value={condition}>
                              {condition}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    )}
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-9 shrink-0 text-muted-foreground"
                    aria-label={t(($) => $.admin.users.filter_remove_label)}
                    onClick={() => remove(index)}
                    data-testid={`user-filter-remove-${index}`}
                  >
                    <X className="size-4" />
                  </Button>
                </div>
                <Field data-invalid={Boolean(valueError)}>
                  <Controller
                    control={form.control}
                    name={`rows.${index}.value`}
                    render={({ field: valueField }) => (
                      <FilterValueInput
                        index={index}
                        field={field}
                        value={valueField.value}
                        onChange={valueField.onChange}
                      />
                    )}
                  />
                  <FieldError errors={[valueError]} />
                </Field>
              </div>
            );
          })}

          <Button type="button" variant="outline" onClick={addRow} data-testid="user-filter-add">
            <Plus className="size-4" />
            {t(($) => $.admin.users.filter_add)}
          </Button>
        </div>

        <SheetFooter className="flex-row justify-end gap-2 border-t border-border px-6 py-4">
          <Button
            type="button"
            variant="outline"
            onClick={reset}
            data-testid="user-filter-reset-all"
          >
            {t(($) => $.admin.users.reset)}
          </Button>
          <Button type="button" onClick={() => void apply()} data-testid="user-filter-apply">
            {t(($) => $.common.confirm)}
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

function FilterValueInput({
  index,
  field,
  value,
  onChange,
}: {
  index: number;
  field: FilterField;
  value: AdminFilter['value'];
  onChange: (value: AdminFilter['value']) => void;
}) {
  const { t } = useTranslation();
  if (field.type === 'select') {
    const options = field.options ?? [];
    const current =
      value == null ? undefined : options.find((option) => String(option.value) === String(value));
    return (
      <Select
        value={current ? String(current.value) : undefined}
        onValueChange={(next) => {
          const option = options.find((item) => String(item.value) === next);
          onChange(option ? option.value : next);
        }}
      >
        <SelectTrigger
          className="w-full"
          aria-label={t(($) => $.admin.users.filter_value_label, { index: index + 1 })}
          data-testid={`user-filter-value-${index}`}
        >
          <SelectValue placeholder={t(($) => $.admin.users.select_placeholder)} />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={String(option.value)} value={String(option.value)}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  }

  if (field.type === 'date') {
    return (
      <Input
        type="datetime-local"
        aria-label={t(($) => $.admin.users.filter_value_label, { index: index + 1 })}
        value={value ? dayjs(1000 * Number(value)).format('YYYY-MM-DDTHH:mm') : ''}
        onChange={(event) =>
          onChange(event.target.value ? String(dayjs(event.target.value).unix()) : '')
        }
        data-testid={`user-filter-value-${index}`}
      />
    );
  }

  return (
    <Input
      placeholder={t(($) => $.admin.users.filter_value_placeholder)}
      aria-label={t(($) => $.admin.users.filter_value_label, { index: index + 1 })}
      value={value == null ? '' : String(value)}
      onChange={(event) => onChange(event.target.value)}
      data-testid={`user-filter-value-${index}`}
    />
  );
}
