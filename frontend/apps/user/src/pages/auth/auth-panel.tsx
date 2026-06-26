import type {
  AnchorHTMLAttributes,
  FormHTMLAttributes,
  ReactNode,
  Ref,
} from 'react';
import { Card, CardBody, CardFooter } from '@/components/ui/card';
import { cn } from '@/lib/cn';
import { AuthBrand } from './auth-brand';
import { AuthLanguageMenu } from './auth-language-menu';

interface AuthPanelProps extends Omit<FormHTMLAttributes<HTMLFormElement>, 'children' | 'className'> {
  children: ReactNode;
  footer: ReactNode;
  formClassName?: string;
  formRef?: Ref<HTMLFormElement>;
  size?: 'default' | 'wide';
}

export function AuthPanel({
  children,
  footer,
  formClassName,
  formRef,
  size = 'default',
  ...formProps
}: AuthPanelProps) {
  return (
    <Card className={cn('v2board-auth-card', size === 'wide' && 'v2board-auth-card--wide')}>
      <form ref={formRef} noValidate className={formClassName} {...formProps}>
        <CardBody>
          <AuthBrand />
          {children}
        </CardBody>
      </form>
      <CardFooter className="v2board-auth-footer">
        <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-3">
          {footer}
        </div>
        <div className="tw:ml-auto tw:shrink-0">
          <AuthLanguageMenu />
        </div>
      </CardFooter>
    </Card>
  );
}

export function AuthFooterLink({ className, ...props }: AnchorHTMLAttributes<HTMLAnchorElement>) {
  return (
    <a
      className={cn(
        'tw:rounded tw:text-foreground-muted tw:transition tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40 tw:focus-visible:ring-offset-2 tw:ring-offset-surface',
        className,
      )}
      {...props}
    />
  );
}

export function AuthFooterDivider() {
  return (
    <span aria-hidden="true" className="tw:text-border">
      ·
    </span>
  );
}
