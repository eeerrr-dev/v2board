import { act } from 'react';
import type { ReactNode } from 'react';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const source = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'legacy-recaptcha.tsx'),
  'utf8',
);

vi.mock('@/components/ui/dialog', () => ({
  Dialog: ({ open, children }: { open: boolean; children: ReactNode }) =>
    open ? <>{children}</> : null,
  DialogContent: ({ children }: { children: ReactNode }) => (
    <div className="ant-modal-body">{children}</div>
  ),
}));

describe('useLegacyRecaptcha legacy DOM', () => {
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

  it('renders the reCAPTCHA widget in the single legacy container div', async () => {
    const { useLegacyRecaptcha } = await import('./legacy-recaptcha');

    function Harness() {
      const { run, recaptchaModal } = useLegacyRecaptcha(true, 'site-key');
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

    const body = container.querySelector('.ant-modal-body')!;
    expect(body.children).toHaveLength(1);
    expect(body.firstElementChild?.tagName).toBe('DIV');
    expect(body.firstElementChild?.children).toHaveLength(1);
    expect(body.querySelector('.grecaptcha-render-target')).toBeTruthy();
  });

  it('keeps the bundled modal props for the captcha challenge', () => {
    expect(source).toContain(
      '<DialogContent key={widgetKey} closable={false} footer={false} centered ariaLabel="reCAPTCHA">',
    );
    expect(source).not.toContain('footer={null}');
  });
});
