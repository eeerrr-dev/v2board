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

  it('does not run the gated action when the surface unmounts during the token hold', async () => {
    const action = vi.fn();
    let solve: ((token: string) => void) | undefined;
    window.grecaptcha = {
      render: vi.fn((target: HTMLElement, options: { callback: (token: string) => void }) => {
        solve = options.callback;
        target.className = 'grecaptcha-render-target';
        return 1;
      }),
      reset: vi.fn(),
    };

    function Harness() {
      const { run, recaptchaModal } = useAuthRecaptcha(true, 'site-key');
      return (
        <>
          <button onClick={() => run(action)}>open</button>
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

    expect(solve).toBeTypeOf('function');

    // grecaptcha solves (schedules the 500ms hold), then the surface unmounts before
    // the hold elapses. The legacy timer would fire the captured mutation; the
    // cancelable timer must be cleared on unmount instead.
    await act(async () => {
      solve?.('token-value');
      root!.unmount();
      root = null;
      await new Promise((resolve) => setTimeout(resolve, 600));
    });

    expect(action).not.toHaveBeenCalled();
  });

  it('releases the cached loader after a failed load so a later attempt retries', async () => {
    // happy-dom refuses to load external scripts, so appending the loader <script>
    // rejects the promise — exactly the "load failed" path. The fix must reset the
    // module singleton so a second attempt re-tries the load instead of reusing the
    // cached rejection; we count load attempts via the appendChild calls.
    delete window.grecaptcha;
    const appendSpy = vi.spyOn(document.body, 'appendChild');
    const scriptAppendCount = () =>
      appendSpy.mock.calls.filter(([node]) => {
        const el = node as Partial<HTMLScriptElement> & { tagName?: string };
        return el?.tagName === 'SCRIPT' && String(el.src ?? '').includes('recaptcha');
      }).length;

    function Harness() {
      const { run, recaptchaModal } = useAuthRecaptcha(true, 'site-key');
      return (
        <>
          <button onClick={() => run(() => {})}>open</button>
          {recaptchaModal}
        </>
      );
    }

    try {
      await act(async () => {
        root!.render(<Harness />);
        await Promise.resolve();
      });

      // First attempt tries to install the loader <script> (which fails to load).
      await act(async () => {
        container
          .querySelector('button')!
          .dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(scriptAppendCount()).toBe(1);

      // Re-opening re-attempts the load rather than reusing the cached rejection,
      // proving the module singleton was released on failure.
      await act(async () => {
        container
          .querySelector('button')!
          .dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(scriptAppendCount()).toBe(2);
    } finally {
      appendSpy.mockRestore();
    }
  });
});
