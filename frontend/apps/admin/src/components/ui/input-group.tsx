import { Slot as SlotPrimitive } from 'radix-ui';
import { type ComponentProps } from 'react';
import { cn } from '@v2board/ui/cn';
import { Button } from '@v2board/ui/button';
import { Input } from '@v2board/ui/input';
import { Textarea } from '@v2board/ui/textarea';

function InputGroup({ className, ...props }: ComponentProps<'div'>) {
  return (
    <div
      role="group"
      data-slot="input-group"
      className={cn(
        'group/input-group relative flex h-10 w-full min-w-0 items-center rounded-md border border-input bg-transparent shadow-xs transition-[color,box-shadow] outline-none has-[[data-slot=input-group-control]:focus-visible]:border-ring has-[[data-slot=input-group-control]:focus-visible]:ring-[3px] has-[[data-slot=input-group-control]:focus-visible]:ring-ring/50 has-[[data-slot=input-group-control][aria-invalid=true]]:border-destructive has-[[data-slot=input-group-control][aria-invalid=true]]:ring-[3px] has-[[data-slot=input-group-control][aria-invalid=true]]:ring-destructive/20 dark:bg-input/30 dark:has-[[data-slot=input-group-control][aria-invalid=true]]:ring-destructive/40',
        className,
      )}
      {...props}
    />
  );
}

type InputGroupAddonProps = ComponentProps<'div'> & {
  align?: 'inline-start' | 'inline-end' | 'block-start' | 'block-end';
};

function InputGroupAddon({
  className,
  align = 'inline-start',
  onClick,
  ...props
}: InputGroupAddonProps) {
  return (
    <div
      data-slot="input-group-addon"
      data-align={align}
      className={cn(
        'flex h-auto cursor-text items-center justify-center gap-2 py-1.5 text-sm font-medium text-muted-foreground [&>svg:not([class*=size-])]:size-4',
        'data-[align=inline-end]:order-last data-[align=inline-end]:pr-3 data-[align=inline-start]:order-first data-[align=inline-start]:pl-3',
        'data-[align=block-start]:order-first data-[align=block-start]:w-full data-[align=block-start]:justify-start data-[align=block-start]:px-3 data-[align=block-start]:pt-3',
        'data-[align=block-end]:order-last data-[align=block-end]:w-full data-[align=block-end]:justify-start data-[align=block-end]:px-3 data-[align=block-end]:pb-3',
        className,
      )}
      onClick={(event) => {
        if ((event.target as HTMLElement).closest('button')) return;
        event.currentTarget.parentElement?.querySelector<HTMLElement>('input,textarea')?.focus();
        onClick?.(event);
      }}
      {...props}
    />
  );
}

function InputGroupButton({
  className,
  type = 'button',
  variant = 'ghost',
  size = 'icon',
  ...props
}: ComponentProps<typeof Button>) {
  return (
    <Button
      data-slot="input-group-button"
      type={type}
      variant={variant}
      size={size}
      className={cn('size-7 rounded-sm shadow-none', className)}
      {...props}
    />
  );
}

function InputGroupText({ className, ...props }: ComponentProps<'span'>) {
  return (
    <span
      data-slot="input-group-text"
      className={cn('text-sm text-muted-foreground', className)}
      {...props}
    />
  );
}

function InputGroupInput({ className, ...props }: ComponentProps<typeof Input>) {
  return (
    <Input
      data-slot="input-group-control"
      className={cn(
        'flex-1 border-0 bg-transparent shadow-none focus-visible:ring-0 dark:bg-transparent',
        className,
      )}
      {...props}
    />
  );
}

function InputGroupTextarea({ className, ...props }: ComponentProps<typeof Textarea>) {
  return (
    <Textarea
      data-slot="input-group-control"
      className={cn(
        'min-h-24 flex-1 resize-none border-0 bg-transparent py-3 shadow-none focus-visible:ring-0 dark:bg-transparent',
        className,
      )}
      {...props}
    />
  );
}

function InputGroupSlot({ className, ...props }: ComponentProps<typeof SlotPrimitive.Root>) {
  return <SlotPrimitive.Root data-slot="input-group-control" className={className} {...props} />;
}

export {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
  InputGroupSlot,
  InputGroupText,
  InputGroupTextarea,
};
