import { createElement } from 'react';
import { act, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@/components/ui/toaster';
import { toast } from './toast';

async function flushToasts(duration = 16) {
  await Promise.resolve();
  vi.advanceTimersByTime(duration);
  await Promise.resolve();
}

function activeToasts(selector: string) {
  return Array.from(document.querySelectorAll(selector)).filter(
    (toastElement) =>
      toastElement.getAttribute('data-removed') !== 'true' &&
      toastElement.getAttribute('data-visible') !== 'false',
  );
}

describe('toast', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    act(() => {
      toast.dismiss();
    });
    render(createElement(Toaster));
  });

  afterEach(() => {
    act(() => {
      toast.dismiss();
    });
    vi.useRealTimers();
  });

  it('keeps only one message toast', async () => {
    await act(async () => {
      toast.success('first');
      toast.error('second');
      await flushToasts();
    });

    const notices = activeToasts('.v2board-toast-message');
    expect(notices).toHaveLength(1);
    expect(notices[0]).toHaveTextContent('second');
  });

  it('destroys message toasts without closing notifications', async () => {
    await act(async () => {
      toast.loading('loading');
      toast.error('error', { description: 'details' });
      await flushToasts();
    });

    await act(async () => {
      toast.destroy();
      await flushToasts(250);
    });

    expect(activeToasts('.v2board-toast-message')).toHaveLength(0);
    expect(activeToasts('.v2board-toast-notification')).toHaveLength(1);
  });

  it('allows desktop notifications to stack', async () => {
    await act(async () => {
      toast.error('error', { description: 'first' });
      toast.info('info', { description: 'second' });
      await flushToasts();
    });

    expect(activeToasts('.v2board-toast-notification')).toHaveLength(2);
  });

  it('keeps notification message and description visible together', async () => {
    await act(async () => {
      toast.error('Request failed', { description: 'Server Error' });
      await flushToasts();
    });

    const notification = activeToasts('.v2board-toast-notification')[0];
    expect(notification).toHaveTextContent('Request failed');
    expect(notification).toHaveTextContent('Server Error');
  });

  it('marks rendered toasts with the island hooks the parity harness selects', async () => {
    await act(async () => {
      toast.success('copied');
      await flushToasts();
    });

    const [message] = activeToasts('.v2board-toast-message');
    expect(message).toBeDefined();
    // The interaction-parity harness waits on `.v2board-toast-root`, and `v2board-island`
    // scopes island tokens onto toast DOM rendered outside the island root.
    expect(message!.classList.contains('v2board-toast-root')).toBe(true);
    expect(message!.classList.contains('v2board-island')).toBe(true);
  });
});
