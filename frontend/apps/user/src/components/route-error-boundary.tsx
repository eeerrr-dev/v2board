import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Outlet, useLocation } from 'react-router-dom';

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
    <div className="block block-rounded">
      <div className="block-content text-center py-5">
        <h3 className="font-w400 text-danger mb-2">页面加载失败</h3>
        <p className="text-muted mb-4">请刷新页面后重试。</p>
        <button type="button" className="btn btn-primary" onClick={() => window.location.reload()}>
          刷新页面
        </button>
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

export function RouteBoundaryElement({ children }: { children: ReactNode }) {
  const location = useLocation();
  return (
    <RouteErrorBoundary resetKey={`${location.pathname}${location.search}`}>
      {children}
    </RouteErrorBoundary>
  );
}
