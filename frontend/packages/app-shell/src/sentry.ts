import * as Sentry from '@sentry/react';

/**
 * Initializes Sentry error reporting. This module is only ever loaded via a
 * dynamic import from the entry when the Rust-injected runtime config carries
 * a `sentry_dsn`, so the SDK chunk is never fetched when reporting is off
 * (the default). Error monitoring only: no performance tracing, no session
 * replay, and no default PII.
 */
export function initSentry(dsn: string): void {
  Sentry.init({
    dsn,
    environment: import.meta.env.MODE,
    sendDefaultPii: false,
  });
}

/** Capture path for render-boundary catches; see `lib/error-reporting.ts`. */
export function captureBoundaryError(error: unknown, componentStack?: string | null): void {
  Sentry.captureException(
    error,
    componentStack ? { contexts: { react: { componentStack } } } : undefined,
  );
}
