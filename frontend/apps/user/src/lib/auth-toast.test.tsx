import { readFileSync } from 'node:fs';
import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { authToast } from './auth-toast';

const source = readFileSync(`${process.cwd()}/src/lib/auth-toast.tsx`, 'utf8');

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('authToast', () => {
  afterEach(() => {
    act(() => {
      authToast.dismiss();
    });
    document.body.innerHTML = '';
  });

  it('renders modern auth feedback without legacy ant message or notification DOM', async () => {
    await act(async () => {
      authToast.success('发送成功', { description: '如果没有收到验证码请检查垃圾箱。' });
      await Promise.resolve();
    });

    const toast = document.body.querySelector('.v2board-auth-toast-root') as HTMLElement;
    expect(toast).not.toBeNull();
    expect(toast.textContent).toContain('发送成功');
    expect(toast.textContent).toContain('如果没有收到验证码请检查垃圾箱。');
    expect(document.body.querySelector('.ant-message')).toBeNull();
    expect(document.body.querySelector('.ant-notification')).toBeNull();
  });

  it('uses Radix Toast and lucide icons instead of hand-built ant markup', () => {
    expect(source).toContain("@radix-ui/react-toast");
    expect(source).toContain("from 'lucide-react'");
    expect(source).not.toContain('ant-message');
    expect(source).not.toContain('ant-notification');
    expect(source).not.toContain('innerHTML');
  });
});
