import { Component, type ErrorInfo, type ReactNode } from 'react';
import { useLocation } from 'react-router-dom';

interface BoundaryProps {
  children: ReactNode;
}

interface BoundaryState {
  error: Error | null;
}

class RouteErrorBoundaryInner extends Component<BoundaryProps, BoundaryState> {
  override state: BoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): BoundaryState {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(error, info.componentStack);
  }

  override render() {
    if (!this.state.error) return this.props.children;

    return (
      <div className="block block-rounded">
        <div className="block-content block-content-full text-center py-5">
          <div className="font-size-h3 font-w600 mb-2">页面加载失败</div>
          <p className="text-muted mb-4">当前页面遇到异常，已阻止整站白屏。</p>
          <button
            className="btn btn-primary mr-2"
            type="button"
            onClick={() => {
              window.location.hash = '/dashboard';
            }}
          >
            返回仪表盘
          </button>
          <button
            className="btn btn-alt-primary"
            type="button"
            onClick={() => window.location.reload()}
          >
            刷新
          </button>
        </div>
      </div>
    );
  }
}

export function RouteErrorBoundary({ children }: BoundaryProps) {
  const location = useLocation();

  return <RouteErrorBoundaryInner key={location.pathname}>{children}</RouteErrorBoundaryInner>;
}
