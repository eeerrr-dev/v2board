import type { HTMLAttributes, ReactNode } from 'react';
import { cva } from 'class-variance-authority';
import { Badge } from './badge';
import { cn } from '@/lib/cn';

export type StatusTone = 'default' | 'success' | 'info' | 'warning' | 'destructive';

const statusBadgeVariants = cva('gap-1.5 border px-2 py-1 font-medium', {
  variants: {
    tone: {
      default: 'border-border bg-secondary text-secondary-foreground',
      destructive: 'border-destructive/30 bg-destructive/10 text-destructive',
      info: 'border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-900 dark:bg-sky-950 dark:text-sky-300',
      success:
        'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-300',
      warning:
        'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-300',
    },
  },
  defaultVariants: { tone: 'default' },
});

const statusDotVariants = cva('size-1.5 rounded-full', {
  variants: {
    tone: {
      default: 'bg-muted-foreground',
      destructive: 'bg-destructive',
      info: 'bg-sky-500',
      success: 'bg-emerald-500',
      warning: 'bg-amber-500',
    },
  },
  defaultVariants: { tone: 'default' },
});

interface StatusBadgeProps extends HTMLAttributes<HTMLSpanElement> {
  children: ReactNode;
  showDot?: boolean;
  tone?: StatusTone;
}

export function StatusBadge({
  children,
  className,
  showDot = false,
  tone = 'default',
  ...props
}: StatusBadgeProps) {
  return (
    <Badge variant="outline" className={cn(statusBadgeVariants({ tone }), className)} {...props}>
      {showDot ? <span className={statusDotVariants({ tone })} /> : null}
      {children}
    </Badge>
  );
}
