import type { FormHTMLAttributes, ReactNode } from 'react';
import { Link, type LinkProps } from 'react-router';
import { Card, CardContent, CardDescription, CardHeader } from '@v2board/ui/card';
import { cn } from '@v2board/ui/cn';

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
            className="m-0 text-2xl leading-8 font-bold text-card-foreground"
          >
            {title}
          </h1>
          {description ? (
            <CardDescription className="mx-auto max-w-[30rem] leading-5 text-balance">
              {description}
            </CardDescription>
          ) : null}
        </CardHeader>
        <CardContent className="px-7 sm:px-8">
          <form noValidate className={formClassName} {...formProps}>
            <div className="grid gap-6">
              {children}
              <div className="text-center text-sm leading-6 text-balance text-muted-foreground">
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
        'rounded-sm font-medium text-foreground underline underline-offset-4 transition-colors hover:text-muted-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
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
        'rounded-sm text-sm font-normal text-foreground underline-offset-4 transition-colors hover:underline focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
        className,
      )}
      {...props}
    />
  );
}
