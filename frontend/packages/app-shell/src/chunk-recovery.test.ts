import { afterEach, describe, expect, it, vi } from 'vitest';
import { installChunkReloadRecovery } from './chunk-recovery';

const RELOADED_AT_KEY = 'v2board:chunk-recovery-reloaded-at';

function dispatchPreloadError(): Event {
  const event = new Event('vite:preloadError', { cancelable: true });
  window.dispatchEvent(event);
  return event;
}

describe('chunk load failure recovery', () => {
  let uninstall: (() => void) | undefined;

  afterEach(() => {
    uninstall?.();
    uninstall = undefined;
    window.sessionStorage.clear();
    vi.restoreAllMocks();
  });

  it('reloads once and swallows the error when a chunk fails to preload', () => {
    const reload = vi.fn();
    uninstall = installChunkReloadRecovery(reload);

    const event = dispatchPreloadError();

    expect(reload).toHaveBeenCalledTimes(1);
    expect(event.defaultPrevented).toBe(true);
    expect(Number(window.sessionStorage.getItem(RELOADED_AT_KEY))).toBeGreaterThan(0);
  });

  it('swallows follow-up failures while the recovery reload is in flight', () => {
    const reload = vi.fn();
    uninstall = installChunkReloadRecovery(reload);

    dispatchPreloadError();
    const second = dispatchPreloadError();

    expect(reload).toHaveBeenCalledTimes(1);
    expect(second.defaultPrevented).toBe(true);
  });

  it('lets the error propagate instead of reload-looping within the cooldown', () => {
    window.sessionStorage.setItem(RELOADED_AT_KEY, String(Date.now() - 1_000));
    const reload = vi.fn();
    uninstall = installChunkReloadRecovery(reload);

    const event = dispatchPreloadError();

    expect(reload).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });

  it('recovers again once the cooldown has expired', () => {
    window.sessionStorage.setItem(RELOADED_AT_KEY, String(Date.now() - 31_000));
    const reload = vi.fn();
    uninstall = installChunkReloadRecovery(reload);

    const event = dispatchPreloadError();

    expect(reload).toHaveBeenCalledTimes(1);
    expect(event.defaultPrevented).toBe(true);
  });

  it('never auto-reloads when sessionStorage is unusable', () => {
    vi.spyOn(window.sessionStorage, 'getItem').mockImplementation(() => {
      throw new Error('denied');
    });
    const reload = vi.fn();
    uninstall = installChunkReloadRecovery(reload);

    const event = dispatchPreloadError();

    expect(reload).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });
});
