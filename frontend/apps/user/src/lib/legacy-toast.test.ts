import { readFileSync } from 'node:fs';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from './legacy-toast';

const source = readFileSync(`${process.cwd()}/src/lib/legacy-toast.ts`, 'utf8');

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('legacy toast behavior', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    act(() => {
      toast.dismiss();
    });
    document.body.innerHTML = '';
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
