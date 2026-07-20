import { getSentryDsn } from './runtime-config';

/**
 * Reports an error caught by a render boundary to Sentry when reporting is
 * enabled. Reuses the `./sentry` module promise, so the SDK chunk still loads
 * only when the runtime config carries a DSN, and `initSentry` (whose `.then`
 * was registered first at boot) always runs before this capture callback.
 */
export function reportBoundaryError(error: unknown, componentStack?: string | null): void {
  if (!getSentryDsn()) return;
  void import('./sentry').then(({ captureBoundaryError }) =>
    captureBoundaryError(error, componentStack),
  );
}
