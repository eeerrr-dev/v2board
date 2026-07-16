import { cva, type VariantProps } from 'class-variance-authority';
import {
  createContext,
  type ComponentProps,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useId,
  useMemo,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { translateRuntimeMessage } from '@/lib/translate-runtime-message';
import { Label } from './label';
import { Separator } from './separator';

type FieldSlot = 'description' | 'error';

interface FieldContextValue {
  defaultIds: Record<FieldSlot, string>;
  register: (slot: FieldSlot, id: string | undefined) => () => void;
}

const FieldContext = createContext<FieldContextValue | null>(null);

function mergeAriaIds(...ids: Array<string | undefined>) {
  const uniqueIds = [...new Set(ids.flatMap((id) => id?.split(/\s+/).filter(Boolean) ?? []))];
  return uniqueIds.length > 0 ? uniqueIds.join(' ') : undefined;
}

function useFieldSlotId(slot: FieldSlot, requestedId: string | undefined, present = true) {
  const context = useContext(FieldContext);
  const id = present ? (requestedId ?? context?.defaultIds[slot]) : undefined;

  useEffect(() => context?.register(slot, id), [context, id, slot]);
  return id;
}

function FieldSet({ className, ...props }: ComponentProps<'fieldset'>) {
  return (
    <fieldset
      data-slot="field-set"
      className={cn('flex min-w-0 flex-col gap-6', className)}
      {...props}
    />
  );
}

const fieldLegendVariants = cva('mb-3 font-medium', {
  variants: { variant: { legend: 'text-base', label: 'text-sm' } },
  defaultVariants: { variant: 'legend' },
});

function FieldLegend({
  className,
  variant,
  ...props
}: ComponentProps<'legend'> & VariantProps<typeof fieldLegendVariants>) {
  return (
    <legend
      data-slot="field-legend"
      data-variant={variant}
      className={cn(fieldLegendVariants({ variant }), className)}
      {...props}
    />
  );
}

function FieldGroup({ className, ...props }: ComponentProps<'div'>) {
  return (
    <div
      data-slot="field-group"
      className={cn('flex w-full flex-col gap-5', className)}
      {...props}
    />
  );
}

const fieldVariants = cva('group/field flex w-full gap-2 data-[invalid=true]:text-destructive', {
  variants: {
    orientation: {
      vertical: 'flex-col',
      horizontal: 'flex-row items-center',
      responsive: 'flex-col sm:flex-row sm:items-center',
    },
  },
  defaultVariants: { orientation: 'vertical' },
});

type FieldProps = ComponentProps<'div'> &
  VariantProps<typeof fieldVariants> & {
    'data-invalid'?: boolean | 'false' | 'true';
  };

function Field({
  className,
  orientation,
  children,
  id: requestedId,
  role = 'group',
  'aria-describedby': requestedDescribedBy,
  'aria-errormessage': requestedErrorMessage,
  'data-invalid': dataInvalid,
  ...props
}: FieldProps) {
  const generatedId = useId();
  const fieldId = requestedId ?? `field-${generatedId}`;
  const defaultIds = useMemo(
    () => ({
      description: `${fieldId}-description`,
      error: `${fieldId}-error`,
    }),
    [fieldId],
  );
  const [slotIds, setSlotIds] = useState<Record<FieldSlot, string | undefined>>({
    description: undefined,
    error: undefined,
  });
  const register = useCallback((slot: FieldSlot, id: string | undefined) => {
    setSlotIds((current) => (current[slot] === id ? current : { ...current, [slot]: id }));
    return () => {
      setSlotIds((current) =>
        current[slot] === id ? { ...current, [slot]: undefined } : current,
      );
    };
  }, []);
  const context = useMemo(() => ({ defaultIds, register }), [defaultIds, register]);
  const invalid = dataInvalid === true || dataInvalid === 'true';

  return (
    <FieldContext value={context}>
      <div
        {...props}
        id={fieldId}
        role={role}
        aria-describedby={mergeAriaIds(
          requestedDescribedBy,
          slotIds.description,
          invalid ? slotIds.error : undefined,
        )}
        aria-errormessage={mergeAriaIds(
          requestedErrorMessage,
          invalid ? slotIds.error : undefined,
        )}
        aria-invalid={invalid || undefined}
        data-slot="field"
        data-invalid={dataInvalid}
        data-orientation={orientation}
        className={cn(fieldVariants({ orientation }), className)}
      >
        {children}
      </div>
    </FieldContext>
  );
}

function FieldContent({ className, ...props }: ComponentProps<'div'>) {
  return (
    <div
      data-slot="field-content"
      className={cn('flex flex-1 flex-col gap-1', className)}
      {...props}
    />
  );
}

function FieldLabel({ className, ...props }: ComponentProps<typeof Label>) {
  return (
    <Label
      data-slot="field-label"
      className={cn(
        'group-data-[disabled=true]/field:opacity-50 group-data-[invalid=true]/field:text-destructive',
        className,
      )}
      {...props}
    />
  );
}

function FieldTitle({ className, ...props }: ComponentProps<'div'>) {
  return (
    <div data-slot="field-title" className={cn('text-sm font-medium', className)} {...props} />
  );
}

function FieldDescription({ className, id: requestedId, ...props }: ComponentProps<'p'>) {
  const id = useFieldSlotId('description', requestedId);
  return (
    <p
      id={id}
      data-slot="field-description"
      className={cn('text-sm leading-normal text-muted-foreground', className)}
      {...props}
    />
  );
}

function FieldSeparator({
  children,
  className,
  ...props
}: ComponentProps<'div'> & { children?: ReactNode }) {
  return (
    <div
      data-slot="field-separator"
      data-content={children ? true : undefined}
      className={cn('relative -my-2 flex h-5 items-center text-sm', className)}
      {...props}
    >
      <Separator className="absolute inset-x-0" />
      {children ? (
        <span className="relative mx-auto bg-background px-2 text-muted-foreground">
          {children}
        </span>
      ) : null}
    </div>
  );
}

interface FieldErrorProps extends ComponentProps<'div'> {
  errors?: Array<{ message?: string } | undefined>;
}

function FieldError({ className, children, errors, id: requestedId, ...props }: FieldErrorProps) {
  const { i18n } = useTranslation();
  const uniqueErrors = [
    ...new Map(errors?.filter(Boolean).map((error) => [error?.message, error])).values(),
  ];
  const message = (error: { message?: string } | undefined) =>
    error?.message ? translateRuntimeMessage(i18n, error.message) : undefined;
  const body =
    children ??
    (uniqueErrors.length === 1 ? (
      message(uniqueErrors[0])
    ) : uniqueErrors.length > 1 ? (
      <ul className="ml-4 list-disc space-y-1">
        {uniqueErrors.map((error) => (
          <li key={error?.message}>{message(error)}</li>
        ))}
      </ul>
    ) : null);
  const id = useFieldSlotId('error', requestedId, Boolean(body));

  if (!body) return null;
  return (
    <div
      id={id}
      role="alert"
      data-slot="field-error"
      className={cn('text-sm font-normal text-destructive', className)}
      {...props}
    >
      {body}
    </div>
  );
}

export {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSeparator,
  FieldSet,
  FieldTitle,
};
