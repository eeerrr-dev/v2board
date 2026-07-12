import type { FormHTMLAttributes, ReactNode } from 'react';
import { Link, type LinkProps } from 'react-router';
import { Card, CardContent, CardDescription, CardHeader } from '@/components/ui/card';
import { cn } from '@/lib/cn';

interface AuthPanelProps extends Omit<
  FormHTMLAttributes<HTMLFormElement>,
  'children' | 'className' | 'title'
> {
  children: ReactNode;
  footer: ReactNode;
  description?: ReactNode;
  formClassName?: string;
  title: ReactNode;
}

export function AuthPanel({
  children,
  footer,
  description,
  formClassName,
  title,
  ...formProps
}: AuthPanelProps) {
  return (
    <div className="mx-auto w-full max-w-md">
      <Card data-testid="auth-card">
        <CardHeader className="gap-2 px-7 text-center sm:px-8">
          <h1
            data-slot="auth-title"
            className="m-0 text-2xl font-bold leading-8 text-card-foreground"
          >
            {title}
          </h1>
          {description ? (
            <CardDescription className="mx-auto max-w-[30rem] text-balance leading-5">
              {description}
            </CardDescription>
          ) : null}
        </CardHeader>
        <CardContent className="px-7 sm:px-8">
          <form noValidate className={formClassName} {...formProps}>
            <div className="grid gap-6">
              {children}
              <div className="text-balance text-center text-sm leading-6 text-muted-foreground">
                {footer}
              </div>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

export function AuthFooterLink({ className, ...props }: LinkProps) {
  return (
    <Link
      className={cn(
        'rounded-sm font-medium text-foreground underline underline-offset-4 transition-colors hover:text-muted-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50',
        className,
      )}
      {...props}
    />
  );
}

export function AuthAuxiliaryLink({ className, ...props }: LinkProps) {
  return (
    <Link
      className={cn(
        'rounded-sm text-sm font-normal text-foreground underline-offset-4 transition-colors hover:underline focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50',
        className,
      )}
      {...props}
    />
  );
}
