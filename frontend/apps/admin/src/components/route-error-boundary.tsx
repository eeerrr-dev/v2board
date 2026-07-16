import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Outlet, useLocation } from 'react-router';
import { AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';

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
  // The boundary can mount outside AdminLayout (it wraps the login and
  // standalone-ticket routes), so it carries the island class itself to pull in
  // the token theming rather than inheriting it from the shell.
  return (
    <div
      data-slot="route-error-state"
      className="flex min-h-[60vh] items-center justify-center p-6"
      data-testid="route-error"
    >
      <div className="flex max-w-sm flex-col items-center gap-4 text-center">
        <div className="flex size-12 items-center justify-center rounded-full bg-destructive/10 text-destructive">
          <AlertTriangle className="size-6" />
        </div>
        <div className="space-y-1">
          <h3 className="text-lg font-semibold text-foreground">页面加载失败</h3>
          <p className="text-sm text-muted-foreground">请刷新页面后重试。</p>
        </div>
        <Button onClick={() => window.location.reload()}>刷新页面</Button>
      </div>
    </div>
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
