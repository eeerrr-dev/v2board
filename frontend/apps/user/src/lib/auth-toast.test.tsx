import { readFileSync } from 'node:fs';
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { Toaster } from '@/components/ui/toaster';
import { authToast } from './auth-toast';

const source = readFileSync(`${process.cwd()}/src/lib/auth-toast.tsx`, 'utf8');

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('authToast', () => {
  let root: Root;

  beforeEach(() => {
    act(() => {
      authToast.dismiss();
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
      authToast.dismiss();
      root.unmount();
    });
  });

  it('renders modern auth feedback without legacy ant message or notification DOM', async () => {
    await act(async () => {
      authToast.success('发送成功', { description: '如果没有收到验证码请检查垃圾箱。' });
      await Promise.resolve();
    });

    const toast = document.body.querySelector('.v2board-toast-notification') as HTMLElement;
    expect(toast).not.toBeNull();
    expect(toast.textContent).toContain('发送成功');
    expect(toast.textContent).toContain('如果没有收到验证码请检查垃圾箱。');
    expect(document.body.querySelector('.v2board-auth-toast-root')).toBeNull();
    expect(document.body.querySelector('.v2board-auth-toast-host')).toBeNull();
    expect(document.body.querySelector('.ant-message')).toBeNull();
    expect(document.body.querySelector('.ant-notification')).toBeNull();
  });

  it('delegates to the unified Radix Toaster instead of owning a second React root', () => {
    expect(source).toContain("from './toast'");
    expect(source).not.toContain('createRoot');
    expect(source).not.toContain("@radix-ui/react-toast");
    expect(source).not.toContain("from 'lucide-react'");
    expect(source).not.toContain('ant-message');
    expect(source).not.toContain('ant-notification');
    expect(source).not.toContain('innerHTML');
  });
});
