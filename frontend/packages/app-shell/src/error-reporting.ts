/**
 * Reports an error caught by a render boundary to Sentry when reporting is
 * enabled. Each app injects its own `getSentryDsn` reader (backed by its own
 * runtime-config module, since the user and admin runtime configs are not
 * shape-compatible) so this shared reporter never assumes a fixed config
 * module. Reuses the `./sentry` module promise, so the SDK chunk still loads
 * only when the runtime config carries a DSN, and `initSentry` (whose `.then`
 * was registered first at boot) always runs before this capture callback.
 */
export function reportBoundaryError(
  getSentryDsn: () => string | undefined,
  error: unknown,
  componentStack?: string | null,
): void {
  if (!getSentryDsn()) return;
  void import('./sentry').then(({ captureBoundaryError }) =>
    captureBoundaryError(error, componentStack),
  );
}
