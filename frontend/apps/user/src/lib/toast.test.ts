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

function activeToasts() {
  return Array.from(document.querySelectorAll('[data-sonner-toast]')).filter(
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

  it('lets independent notifications coexist', async () => {
    await act(async () => {
      toast.success('first');
      toast.error('second');
      await flushToasts();
    });

    const notices = activeToasts();
    expect(notices).toHaveLength(2);
    expect(notices.map((notice) => notice.textContent)).toEqual(
      expect.arrayContaining([expect.stringContaining('first'), expect.stringContaining('second')]),
    );
  });

  it('keeps loading notifications alive until their own id is dismissed', async () => {
    let loadingId: string | number | undefined;
    await act(async () => {
      loadingId = toast.loading('loading');
      toast.error('error', { description: 'details' });
      await flushToasts();
      vi.advanceTimersByTime(10_000);
      await flushToasts(250);
    });

    const [loading] = activeToasts();
    expect(loading).toHaveTextContent('loading');

    await act(async () => {
      toast.dismiss(loadingId);
      await flushToasts(250);
    });

    expect(activeToasts()).toHaveLength(0);
  });

  it('allows notifications with descriptions to stack', async () => {
    await act(async () => {
      toast.error('error', { description: 'first' });
      toast.info('info', { description: 'second' });
      await flushToasts();
    });

    expect(activeToasts()).toHaveLength(2);
  });

  it('keeps notification message and description visible together', async () => {
    await act(async () => {
      toast.error('Request failed', { description: 'Server Error' });
      await flushToasts();
    });

    const notification = activeToasts()[0];
    expect(notification).toHaveTextContent('Request failed');
    expect(notification).toHaveTextContent('Server Error');
  });

  it('marks rendered toasts with Sonner data attributes', async () => {
    await act(async () => {
      toast.success('copied');
      await flushToasts();
    });

    const [message] = activeToasts();
    expect(message).toBeDefined();
    expect(message).toHaveAttribute('data-sonner-toast');
  });
});
