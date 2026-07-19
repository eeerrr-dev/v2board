import { type ComponentProps } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { Skeleton } from '@/components/ui/skeleton';

export interface LoadingStateProps extends ComponentProps<'div'> {
  /** Overrides the default status text. */
  label?: string;
}

/**
 * Shared skeleton loading state: an accessible `role="status"` region whose
 * visible placeholder mimics the incoming content instead of a spinner.
 * Compose the shape from `SkeletonRows`/`SkeletonFields`/`SkeletonLines` or
 * raw `Skeleton` blocks. Spinners remain only for button-pending states and
 * the pre-shell full-screen boot fallbacks.
 */
export function LoadingState({ label, className, children, ...props }: LoadingStateProps) {
  const { t } = useTranslation();
  return (
    <div role="status" data-slot="loading-state" className={cn('w-full', className)} {...props}>
      <span className="sr-only">{label ?? t(($) => $.common.loading)}</span>
      {children}
    </div>
  );
}

/** List/table placeholder: rows of column-shaped blocks. */
export function SkeletonRows({ rows = 3, className }: { rows?: number; className?: string }) {
  return (
    <div className={cn('space-y-3', className)} aria-hidden>
      {Array.from({ length: rows }, (_, index) => (
        <div key={index} className="flex items-center gap-3">
          <Skeleton className="h-4 w-1/5 min-w-16" />
          <Skeleton className="h-4 flex-1" />
          <Skeleton className="h-4 w-1/6 min-w-12" />
        </div>
      ))}
    </div>
  );
}

/** Form placeholder: label + control pairs. */
export function SkeletonFields({ fields = 4, className }: { fields?: number; className?: string }) {
  return (
    <div className={cn('space-y-5', className)} aria-hidden>
      {Array.from({ length: fields }, (_, index) => (
        <div key={index} className="space-y-2">
          <Skeleton className="h-4 w-24" />
          <Skeleton className="h-9 w-full" />
        </div>
      ))}
    </div>
  );
}

/** Text/detail placeholder: paragraph lines with a short last line. */
export function SkeletonLines({ lines = 3, className }: { lines?: number; className?: string }) {
  return (
    <div className={cn('space-y-2.5', className)} aria-hidden>
      {Array.from({ length: lines }, (_, index) => (
        <Skeleton key={index} className={cn('h-4', index === lines - 1 ? 'w-2/3' : 'w-full')} />
      ))}
    </div>
  );
}
