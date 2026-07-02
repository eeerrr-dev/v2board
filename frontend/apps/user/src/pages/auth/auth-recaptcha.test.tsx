import { screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { useAuthRecaptcha } from './auth-recaptcha';

function Harness({ action = () => {} }: { action?: (recaptchaData?: string) => void }) {
  const { run, recaptchaModal } = useAuthRecaptcha(true, 'site-key');
  return (
    <>
      <button type="button" onClick={() => run(action)}>
        open
      </button>
      {recaptchaModal}
    </>
  );
}

describe('useAuthRecaptcha', () => {
  beforeEach(() => {
    window.grecaptcha = {
      render: vi.fn(() => 1),
      reset: vi.fn(),
    };
  });

  afterEach(() => {
    delete window.grecaptcha;
    delete window.onloadcallback;
  });

  it('opens an accessible dialog and renders the grecaptcha challenge inside it with the site key', async () => {
    const { user } = renderWithProviders(<Harness />);

    await user.click(screen.getByRole('button', { name: 'open' }));

    const dialog = await screen.findByRole('dialog', { name: 'reCAPTCHA' });
    await waitFor(() => {
      expect(window.grecaptcha!.render).toHaveBeenCalledTimes(1);
    });

    const [target, options] = vi.mocked(window.grecaptcha!.render).mock.calls[0]!;
    expect(dialog.contains(target)).toBe(true);
    expect(options.sitekey).toBe('site-key');
    expect(options.callback).toBeTypeOf('function');
  });

  it('does not run the gated action when the surface unmounts during the token hold', async () => {
    const action = vi.fn();
    let solve: ((token: string) => void) | undefined;
    window.grecaptcha = {
      render: vi.fn((_target: HTMLElement, options: { callback: (token: string) => void }) => {
        solve = options.callback;
        return 1;
      }),
      reset: vi.fn(),
    };

    const { user, unmount } = renderWithProviders(<Harness action={action} />);

    await user.click(screen.getByRole('button', { name: 'open' }));
    await waitFor(() => {
      expect(solve).toBeTypeOf('function');
    });

    // grecaptcha solves (schedules the 500ms hold), then the surface unmounts before
    // the hold elapses. The legacy timer would fire the captured mutation; the
    // cancelable timer must be cleared on unmount instead.
    solve!('token-value');
    unmount();
    await new Promise((resolve) => setTimeout(resolve, 600));

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

    try {
      const { user } = renderWithProviders(<Harness />);

      // First attempt tries to install the loader <script> (which fails to load).
      await user.click(screen.getByRole('button', { name: 'open' }));
      await waitFor(() => {
        expect(scriptAppendCount()).toBe(1);
      });

      // The user closes the blank dialog and retries: the second attempt must
      // re-append a fresh loader rather than reuse the cached rejection, proving
      // the module singleton was released on failure.
      await user.keyboard('{Escape}');
      await waitFor(() => {
        expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      });

      await user.click(screen.getByRole('button', { name: 'open' }));
      await waitFor(() => {
        expect(scriptAppendCount()).toBe(2);
      });
    } finally {
      appendSpy.mockRestore();
    }
  });
});
