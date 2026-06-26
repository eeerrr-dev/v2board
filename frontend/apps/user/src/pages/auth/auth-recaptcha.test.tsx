import { act } from 'react';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAuthRecaptcha } from './auth-recaptcha';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const source = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'auth-recaptcha.tsx'),
  'utf8',
);

describe('useAuthRecaptcha', () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    window.grecaptcha = {
      render: vi.fn((target: HTMLElement) => {
        target.className = 'grecaptcha-render-target';
        return 1;
      }),
      reset: vi.fn(),
    };
  });

  afterEach(() => {
    if (root) act(() => root?.unmount());
    root = null;
    container.remove();
    document.body.innerHTML = '';
    delete window.grecaptcha;
    delete window.onloadcallback;
  });

  it('renders the challenge inside the auth shadcn dialog', async () => {
    function Harness() {
      const { run, recaptchaModal } = useAuthRecaptcha(true, 'site-key');
      return (
        <>
          <button onClick={() => run(() => {})}>open</button>
          {recaptchaModal}
        </>
      );
    }

    await act(async () => {
      root!.render(<Harness />);
      await Promise.resolve();
    });

    await act(async () => {
      container.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(document.body.querySelector('.ant-modal-body')).toBeNull();
    const dialog = document.body.querySelector('[role="dialog"]')!;
    expect(dialog.className).toContain('bg-background');
    expect(dialog.className).toContain('rounded-lg');
    expect(dialog.querySelector('.grecaptcha-render-target')).toBeTruthy();
    expect(dialog.querySelector('.sr-only')?.textContent).toBe('reCAPTCHA');
  });

  it('uses the auth Radix dialog wrapper instead of the legacy modal bridge', () => {
    expect(source).toContain("from '@/components/ui/shadcn-dialog'");
    expect(source).not.toContain("from '@/components/ui/dialog'");
    expect(source).not.toContain('DialogContent key={widgetKey} closable=');
    expect(source).not.toContain('ant-modal');
  });
});
