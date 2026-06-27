import { readFileSync } from 'node:fs';
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@/components/ui/toaster';
import { toast } from './toast';

const source = readFileSync(`${process.cwd()}/src/components/ui/toaster.tsx`, 'utf8');

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('legacy toast behavior', () => {
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
      await Promise.resolve();
    });

    const notices = document.querySelectorAll('.v2board-toast-message');
    expect(notices).toHaveLength(1);
    expect(notices[0]?.textContent).toContain('second');
    expect(document.body.querySelector('.ant-message')).toBeNull();
  });

  it('destroys message toasts without closing notifications', async () => {
    await act(async () => {
      toast.loading('loading');
      toast.error('error', { description: 'details' });
      await Promise.resolve();
    });

    await act(async () => {
      toast.destroy();
      await Promise.resolve();
    });

    expect(document.querySelectorAll('.v2board-toast-message')).toHaveLength(0);
    expect(document.querySelectorAll('.v2board-toast-notification')).toHaveLength(1);
  });

  it('allows desktop notifications to stack', async () => {
    await act(async () => {
      toast.error('error', { description: 'first' });
      toast.info('info', { description: 'second' });
      await Promise.resolve();
    });

    expect(document.querySelectorAll('.v2board-toast-notification')).toHaveLength(2);
  });

  it('keeps notification message and description text adjacent', async () => {
    await act(async () => {
      toast.error('Request failed', { description: 'Server Error' });
      await Promise.resolve();
    });

    expect(document.querySelector('.v2board-toast-notification')?.textContent).toContain(
      'Request failedServer Error',
    );
  });

  it('uses Radix Toast and lucide icons instead of hand-built ant markup', () => {
    expect(source).toContain("@radix-ui/react-toast");
    expect(source).toContain("from 'lucide-react'");
    expect(source).toContain('v2board-toast-root');
    expect(source).not.toContain('ant-message');
    expect(source).not.toContain('ant-notification');
    expect(source).not.toContain('ANT_ICONS');
    expect(source).not.toContain('innerHTML');
  });
});
