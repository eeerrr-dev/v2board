import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ConfirmDialogProvider, confirmDialog } from './confirm-dialog';

// The provider only reads i18n.language for the localized default button text.
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ i18n: { language: 'zh-CN' } }),
}));

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

function clickPrimary() {
  document.body
    .querySelector<HTMLButtonElement>('.v2board-confirm-primary')!
    .dispatchEvent(new MouseEvent('click', { bubbles: true }));
}

function currentTitle() {
  return document.body.querySelector('.v2board-confirm-title')?.textContent ?? null;
}

// Characterization tests for the queued confirm-dialog store + provider. These
// pin the OBSERVABLE contract that any future refactor (e.g. moving the store to
// useSyncExternalStore) must preserve: queue advance, promise resolution, the
// actionLoading reset between requests, and the closingRef guard that stops a
// programmatic close from firing onCancel. The naive `open = Boolean(request) &&
// !closingRef` rewrite breaks these, so this suite must stay green before any
// migration lands.
describe('confirm dialog queued behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  it('advances to the next queued request after the current one resolves', async () => {
    await act(async () => {
      root.render(<ConfirmDialogProvider />);
    });

    let resolvedA: boolean | undefined;
    let resolvedB: boolean | undefined;
    await act(async () => {
      void confirmDialog({ title: 'First' }).then((value) => {
        resolvedA = value;
      });
      void confirmDialog({ title: 'Second' }).then((value) => {
        resolvedB = value;
      });
      await Promise.resolve();
    });

    expect(currentTitle()).toBe('First');

    await act(async () => {
      clickPrimary();
      await Promise.resolve();
    });
    await flushPromises();

    // First resolved true; the queue advanced to the second request without
    // resolving it yet, and its primary button is interactive again (the
    // actionLoading reset fired on the request id change).
    expect(resolvedA).toBe(true);
    expect(resolvedB).toBeUndefined();
    expect(currentTitle()).toBe('Second');

    await act(async () => {
      clickPrimary();
      await Promise.resolve();
    });
    await flushPromises();

    expect(resolvedB).toBe(true);
    expect(currentTitle()).toBeNull();
  });

  it('resolves false and runs onCancel when the cancel button is clicked', async () => {
    const onCancel = vi.fn();
    await act(async () => {
      root.render(<ConfirmDialogProvider />);
    });

    let resolved: boolean | undefined;
    await act(async () => {
      void confirmDialog({ title: 'Cancel me', onCancel, cancelText: 'Nope' }).then((value) => {
        resolved = value;
      });
      await Promise.resolve();
    });

    await act(async () => {
      const cancelButton = Array.from(
        document.body.querySelectorAll<HTMLButtonElement>('.v2board-confirm-footer button'),
      ).find((button) => button.textContent?.includes('Nope'))!;
      cancelButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(resolved).toBe(false);
  });

  it('does not invoke onCancel when a confirm programmatically closes the dialog', async () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    await act(async () => {
      root.render(<ConfirmDialogProvider />);
    });

    let resolved: boolean | undefined;
    await act(async () => {
      void confirmDialog({ title: 'Confirm me', onConfirm, onCancel }).then((value) => {
        resolved = value;
      });
      await Promise.resolve();
    });

    await act(async () => {
      clickPrimary();
      await Promise.resolve();
    });
    await flushPromises();

    // The programmatic close flips Radix open -> false; the closingRef guard must
    // stop that transition from being treated as a user cancel.
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onCancel).not.toHaveBeenCalled();
    expect(resolved).toBe(true);
  });
});
