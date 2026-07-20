import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { createTestTranslation } from '../test/i18next-selector';
import { renderWithUser } from '../test/render';
import { ConfirmDialogProvider, confirmDialog } from './confirm-dialog';

// The provider reads the canonical common translations for its default buttons.
vi.mock('react-i18next', () => ({
  useTranslation: () =>
    createTestTranslation({
      'common.confirm': '确定',
      'common.cancel': '取消',
    }),
}));

// Behavior tests for the queued confirm-dialog store + provider. These pin the
// OBSERVABLE contract that any future refactor of the external store must
// preserve: queue advance, promise resolution, the actionLoading reset between
// requests, and the closingRef guard that stops a programmatic close from
// firing onCancel. The naive `open = Boolean(request) && !closingRef` rewrite
// breaks these, so this suite must stay green before any migration lands.
describe('confirm dialog queued behavior', () => {
  it('advances to the next queued request after the current one resolves', async () => {
    const { user } = renderWithUser(<ConfirmDialogProvider />);

    let resolvedA: boolean | undefined;
    let resolvedB: boolean | undefined;
    act(() => {
      void confirmDialog({ title: 'First' }).then((value) => {
        resolvedA = value;
      });
      void confirmDialog({ title: 'Second' }).then((value) => {
        resolvedB = value;
      });
    });

    expect(await screen.findByRole('alertdialog', { name: 'First' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '确定' }));

    // First resolved true; the queue advanced to the second request without
    // resolving it yet, and its primary button is interactive again (the
    // actionLoading reset fired on the request id change).
    await waitFor(() => expect(resolvedA).toBe(true));
    expect(resolvedB).toBeUndefined();
    const second = screen.getByRole('alertdialog', { name: 'Second' });
    const secondPrimary = within(second).getByRole('button', { name: '确定' });
    expect(secondPrimary).toBeEnabled();

    await user.click(secondPrimary);

    await waitFor(() => expect(resolvedB).toBe(true));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  });

  it('resolves false and runs onCancel when the cancel button is clicked', async () => {
    const onCancel = vi.fn();
    const { user } = renderWithUser(<ConfirmDialogProvider />);

    let resolved: boolean | undefined;
    act(() => {
      void confirmDialog({ title: 'Cancel me', onCancel, cancelText: 'Nope' }).then((value) => {
        resolved = value;
      });
    });

    await user.click(await screen.findByRole('button', { name: 'Nope' }));

    await waitFor(() => expect(resolved).toBe(false));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('focuses the safe cancel action so Enter cannot confirm a destructive request', async () => {
    const { user } = renderWithUser(<ConfirmDialogProvider />);

    let resolved: boolean | undefined;
    act(() => {
      void confirmDialog({ title: 'Delete item?' }).then((value) => {
        resolved = value;
      });
    });

    const dialog = await screen.findByRole('alertdialog', { name: 'Delete item?' });
    const cancel = within(dialog).getByRole('button', { name: '取消' });
    await waitFor(() => expect(cancel).toHaveFocus());
    expect(cancel).toHaveAttribute('data-alert-dialog-cancel');

    await user.keyboard('{Enter}');

    await waitFor(() => expect(resolved).toBe(false));
  });

  it('does not invoke onCancel when a confirm programmatically closes the dialog', async () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    const { user } = renderWithUser(<ConfirmDialogProvider />);

    let resolved: boolean | undefined;
    act(() => {
      void confirmDialog({ title: 'Confirm me', onConfirm, onCancel }).then((value) => {
        resolved = value;
      });
    });

    await user.click(await screen.findByRole('button', { name: '确定' }));

    // The programmatic close flips Radix open -> false; the closingRef guard must
    // stop that transition from being treated as a user cancel.
    await waitFor(() => expect(resolved).toBe(true));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('renders an accessible alert dialog carrying the parity selector hooks and localized defaults', async () => {
    const { user } = renderWithUser(<ConfirmDialogProvider />);

    let resolved: boolean | undefined;
    act(() => {
      void confirmDialog({ title: 'Hooked', description: 'Body copy' }).then((value) => {
        resolved = value;
      });
    });

    // The shell is a real alert dialog labelled by its title (shadcn/Radix
    // alert-dialog semantics rather than an Ant modal shim).
    const dialog = await screen.findByRole('alertdialog', { name: 'Hooked' });
    expect(dialog).toHaveAttribute('data-slot', 'alert-dialog-content');
    expect(within(dialog).getByText('Hooked')).toHaveAttribute('data-slot', 'alert-dialog-title');
    expect(within(dialog).getByText('Body copy')).toHaveAttribute(
      'data-slot',
      'alert-dialog-description',
    );
    const confirm = within(dialog).getByRole('button', { name: '确定' });
    expect(confirm).toHaveAttribute('data-alert-dialog-action');
    expect(confirm.closest('[data-slot="alert-dialog-footer"]')).not.toBeNull();
    // Default button text comes from the canonical common translations.
    expect(within(dialog).getByRole('button', { name: '取消' })).toBeInTheDocument();

    await user.click(confirm);
    await waitFor(() => expect(resolved).toBe(true));
  });
});
