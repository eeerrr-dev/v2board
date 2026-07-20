import { beforeEach, describe, expect, it, vi } from 'vitest';

const getSentryDsn = vi.hoisted(() => vi.fn<() => string | undefined>());
vi.mock('./runtime-config', () => ({ getSentryDsn }));

const captureBoundaryError = vi.hoisted(() => vi.fn());
vi.mock('./sentry', () => ({ captureBoundaryError }));

import { reportBoundaryError } from './error-reporting';

describe('reportBoundaryError DSN gate', () => {
  beforeEach(() => {
    getSentryDsn.mockReset();
    captureBoundaryError.mockReset();
  });

  it('does nothing when reporting is off (the default)', async () => {
    getSentryDsn.mockReturnValue(undefined);
    reportBoundaryError(new Error('crash'), 'stack');
    await vi.waitFor(() => expect(getSentryDsn).toHaveBeenCalled());
    expect(captureBoundaryError).not.toHaveBeenCalled();
  });

  it('forwards the error and component stack when a DSN is configured', async () => {
    getSentryDsn.mockReturnValue('https://key@sentry.example/1');
    const error = new Error('crash');
    reportBoundaryError(error, 'component stack');
    await vi.waitFor(() =>
      expect(captureBoundaryError).toHaveBeenCalledWith(error, 'component stack'),
    );
  });
});
