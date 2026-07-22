import { beforeEach, describe, expect, it, vi } from 'vitest';

const captureBoundaryError = vi.hoisted(() => vi.fn());
vi.mock('./sentry', () => ({ captureBoundaryError }));

import { reportBoundaryError } from './error-reporting';

describe('reportBoundaryError DSN gate', () => {
  beforeEach(() => {
    captureBoundaryError.mockReset();
  });

  it('does nothing when reporting is off (the default)', async () => {
    const getSentryDsn = vi.fn<() => string | undefined>().mockReturnValue(undefined);
    reportBoundaryError(getSentryDsn, new Error('crash'), 'stack');
    await vi.waitFor(() => expect(getSentryDsn).toHaveBeenCalled());
    expect(captureBoundaryError).not.toHaveBeenCalled();
  });

  it('forwards the error and component stack when a DSN is configured', async () => {
    const getSentryDsn = vi
      .fn<() => string | undefined>()
      .mockReturnValue('https://key@sentry.example/1');
    const error = new Error('crash');
    reportBoundaryError(getSentryDsn, error, 'component stack');
    await vi.waitFor(() =>
      expect(captureBoundaryError).toHaveBeenCalledWith(error, 'component stack'),
    );
  });
});
