import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Outlet, useLocation } from 'react-router-dom';
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
  return (
    <Card className="mx-auto max-w-lg">
      <CardContent className="grid gap-4 p-6 text-center">
        <Alert variant="destructive" className="text-left">
          <AlertCircle className="size-4" />
          <AlertDescription>
            <span className="font-medium">页面加载失败</span>
            <span className="text-muted-foreground">请刷新页面后重试。</span>
          </AlertDescription>
        </Alert>
        <Button type="button" className="mx-auto" onClick={() => window.location.reload()}>
          <RefreshCw className="size-4" />
          刷新页面
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
