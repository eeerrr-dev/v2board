import { type HTMLAttributes, type ReactNode, type Ref } from 'react';
import { cn } from '../lib/cn';

export function PageShell({
  className,
  ref,
  ...props
}: HTMLAttributes<HTMLDivElement> & { ref?: Ref<HTMLDivElement> }) {
  // No width cap here: the app-layout content wrapper owns the shared
  // mx-auto/max-w-6xl column so the header and every page align.
  return (
    <div
      ref={ref}
      data-slot="page-shell"
      className={cn(
        'flex w-full animate-in flex-col gap-6 duration-200 fade-in-0 slide-in-from-bottom-1 motion-reduce:animate-none!',
        className,
      )}
      {...props}
    />
  );
}

interface PageHeaderProps extends Omit<HTMLAttributes<HTMLDivElement>, 'title'> {
  actions?: ReactNode;
  description?: ReactNode;
  eyebrow?: ReactNode;
  title: ReactNode;
}

export function PageHeader({
  actions,
  className,
  description,
  eyebrow,
  title,
  ...props
}: PageHeaderProps) {
  return (
    <div
      data-slot="page-header"
      className={cn('flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between', className)}
      {...props}
    >
      <div className="min-w-0 space-y-1.5">
        {eyebrow ? (
          <div className="text-xs font-medium tracking-normal text-muted-foreground uppercase">
            {eyebrow}
          </div>
        ) : null}
        <h2 className="truncate text-2xl font-semibold tracking-normal text-foreground">{title}</h2>
        {description ? (
          <p className="max-w-2xl text-sm leading-6 text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {actions ? <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  );
}

interface EmptyStateProps extends Omit<HTMLAttributes<HTMLDivElement>, 'title'> {
  action?: ReactNode;
  description?: ReactNode;
  icon?: ReactNode;
  title: ReactNode;
}

export function EmptyState({
  action,
  className,
  description,
  icon,
  title,
  ...props
}: EmptyStateProps) {
  return (
    <div
      className={cn(
        'flex min-h-44 flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border bg-card px-6 py-10 text-center text-card-foreground',
        className,
      )}
      {...props}
    >
      {icon ? (
        <div className="flex size-10 items-center justify-center rounded-md bg-muted text-muted-foreground">
          {icon}
        </div>
      ) : null}
      <div className="space-y-1">
        <div className="text-sm font-medium text-foreground">{title}</div>
        {description ? <div className="text-sm text-muted-foreground">{description}</div> : null}
      </div>
      {action ? <div className="pt-1">{action}</div> : null}
    </div>
  );
}
