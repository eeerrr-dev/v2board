import { type InputHTMLAttributes, type Ref } from 'react';
import { cn } from '@/lib/cn';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Renders the error treatment (destructive border/ring) and sets aria-invalid. */
  invalid?: boolean;
  ref?: Ref<HTMLInputElement>;
}

export function Input({ className, type = 'text', invalid, ref, ...props }: InputProps) {
  return (
    <input
      ref={ref}
      data-slot="input"
      type={type}
      aria-invalid={invalid || undefined}
      className={cn(
        'flex h-10 w-full min-w-0 rounded-md border bg-transparent px-3 py-1 text-base shadow-xs outline-none transition-[color,box-shadow] selection:bg-primary selection:text-primary-foreground placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm',
        invalid
          ? 'border-destructive focus-visible:border-destructive focus-visible:ring-[3px] focus-visible:ring-destructive/20'
          : 'border-input focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50',
        className,
      )}
      {...props}
    />
  );
}
