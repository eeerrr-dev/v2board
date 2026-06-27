import { Component, type ErrorInfo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Outlet, useLocation } from 'react-router';
import { AlertCircle, RefreshCw } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';

interface RouteErrorBoundaryProps {
  children: ReactNode;
  resetKey: string;
}

interface RouteErrorBoundaryState {
  hasError: boolean;
}

export class RouteErrorBoundary extends Component<
  RouteErrorBoundaryProps,
  RouteErrorBoundaryState
> {
  override state: RouteErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): RouteErrorBoundaryState {
    return { hasError: true };
  }

  override componentDidUpdate(previousProps: RouteErrorBoundaryProps) {
    if (previousProps.resetKey !== this.props.resetKey && this.state.hasError) {
      this.setState({ hasError: false });
    }
  }

  override componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error(error, errorInfo);
  }

  override render() {
    if (this.state.hasError) return <RouteErrorFallback />;
    return this.props.children;
  }
}

export function RouteErrorFallback() {
  const { t } = useTranslation();

  return (
    <Card className="mx-auto max-w-lg">
      <CardContent className="grid gap-4 p-6 text-center">
        <Alert variant="destructive" className="text-left">
          <AlertCircle className="size-4" />
          <AlertDescription>
            <span className="font-medium">{t('common.route_load_failed')}</span>
            <span className="text-muted-foreground">{t('common.route_refresh_hint')}</span>
          </AlertDescription>
        </Alert>
        <Button type="button" className="mx-auto" onClick={() => window.location.reload()}>
          <RefreshCw className="size-4" />
          {t('common.refresh_page')}
        </Button>
      </CardContent>
    </Card>
  );
}

export function RouteBoundaryOutlet() {
  const location = useLocation();
  return (
    <RouteErrorBoundary resetKey={`${location.pathname}${location.search}`}>
      <Outlet />
    </RouteErrorBoundary>
  );
}

export function RouteBoundaryElement({ children }: { children: ReactNode }) {
  const location = useLocation();
  return (
    <RouteErrorBoundary resetKey={`${location.pathname}${location.search}`}>
      {children}
    </RouteErrorBoundary>
  );
}
