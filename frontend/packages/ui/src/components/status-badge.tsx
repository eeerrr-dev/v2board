import type { HTMLAttributes, ReactNode } from 'react';
import { cva } from 'class-variance-authority';
import { Badge } from './badge';
import { cn } from '../lib/cn';

export type StatusTone = 'default' | 'success' | 'info' | 'warning' | 'destructive';

const statusBadgeVariants = cva('gap-1.5 border px-2 py-1 font-medium', {
  variants: {
    tone: {
      default: 'border-border bg-secondary text-secondary-foreground',
      destructive: 'border-destructive/30 bg-destructive/10 text-destructive',
      info: 'border-info/30 bg-info/10 text-info',
      success: 'border-success/30 bg-success/10 text-success',
      warning: 'border-warning/30 bg-warning/10 text-warning',
    },
  },
  defaultVariants: { tone: 'default' },
});

const statusDotVariants = cva('size-1.5 rounded-full', {
  variants: {
    tone: {
      default: 'bg-muted-foreground',
      destructive: 'bg-destructive',
      info: 'bg-info',
      success: 'bg-success',
      warning: 'bg-warning',
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
