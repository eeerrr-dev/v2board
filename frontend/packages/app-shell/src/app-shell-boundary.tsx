import { Component, type ErrorInfo, type ReactNode } from 'react';
import { reportBoundaryError } from './error-reporting';

interface AppShellBoundaryProps {
  children: ReactNode;
  /**
   * Each app injects its own Sentry DSN reader (backed by its own
   * runtime-config module); see `./error-reporting`.
   */
  getSentryDsn: () => string | undefined;
}

interface AppShellBoundaryState {
  hasError: boolean;
}

/**
 * Last-resort boundary above the router, i18n, and query providers: a crash in
 * the shell itself (provider setup, layout chrome, the route boundary's own
 * fallback) would otherwise unmount the tree to a blank page. The fallback is
 * deliberately provider-free — static copy, no i18n or router hooks — because
 * any of those providers may be exactly what crashed.
 */
export class AppShellBoundary extends Component<AppShellBoundaryProps, AppShellBoundaryState> {
  override state: AppShellBoundaryState = { hasError: false };

  static getDerivedStateFromError(): AppShellBoundaryState {
    return { hasError: true };
  }

  override componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error(error, errorInfo);
    reportBoundaryError(this.props.getSentryDsn, error, errorInfo.componentStack);
  }

  override render() {
    if (this.state.hasError) {
      return (
        <div
          data-testid="app-shell-error"
          className="flex min-h-screen items-center justify-center bg-background p-6"
        >
          <div className="flex max-w-sm flex-col items-center gap-4 text-center">
            <h1 className="text-lg font-semibold text-foreground">
              页面出错了 / Something went wrong
            </h1>
            <p className="text-sm text-muted-foreground">
              请刷新页面重试。Please refresh the page and try again.
            </p>
            <button
              type="button"
              onClick={() => window.location.reload()}
              className="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary/90"
            >
              刷新 / Refresh
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
