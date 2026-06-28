import { readFileSync } from 'node:fs';
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@/components/ui/toaster';
import { toast } from './toast';

const source = readFileSync(`${process.cwd()}/src/components/ui/toaster.tsx`, 'utf8');

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

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
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    act(() => {
      toast.dismiss();
    });
    document.body.innerHTML = '';
    const host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => {
      root.render(createElement(Toaster));
    });
  });

  afterEach(() => {
    act(() => {
      toast.dismiss();
      root.unmount();
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
    expect(notices[0]?.textContent).toContain('second');
    expect(document.body.querySelector('.ant-message')).toBeNull();
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

    const notificationText = document.querySelector('.v2board-toast-notification')?.textContent;
    expect(notificationText).toContain('Request failed');
    expect(notificationText).toContain('Server Error');
  });

  it('uses Sonner instead of a self-owned Radix toast store', () => {
    expect(source).toContain("from 'sonner'");
    expect(source).toContain('v2board-toast-root');
    expect(source).not.toContain("@radix-ui/react-toast");
    expect(source).not.toContain('useSyncExternalStore');
    expect(source).not.toContain('ant-message');
    expect(source).not.toContain('ant-notification');
    expect(source).not.toContain('innerHTML');
  });
});
