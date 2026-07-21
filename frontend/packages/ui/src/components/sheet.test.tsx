import { useState } from 'react';
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { createTestTranslation } from '../test/i18next-selector';
import { renderWithUser } from '../test/render';
import { Sheet, SheetContent, SheetDescription, SheetTitle } from './sheet';

vi.mock('react-i18next', () => ({
  useTranslation: () =>
    createTestTranslation({
      'common.close_dialog': 'Close sheet',
    }),
}));

function ControlledSheet({ onOpenChange }: { onOpenChange: (open: boolean) => void }) {
  const [open, setOpen] = useState(true);
  return (
    <Sheet
      open={open}
      onOpenChange={(next) => {
        onOpenChange(next);
        setOpen(next);
      }}
    >
      <SheetContent>
        <SheetTitle>Node editor</SheetTitle>
        <SheetDescription>Edit a server node.</SheetDescription>
      </SheetContent>
    </Sheet>
  );
}

describe('Sheet', () => {
  it('exposes a stable close hook and closes through the visible control', async () => {
    const onOpenChange = vi.fn();
    const { user } = renderWithUser(<ControlledSheet onOpenChange={onOpenChange} />);
    const dialog = await screen.findByRole('dialog', { name: 'Node editor' });
    const close = within(dialog).getByRole('button', { name: 'Close sheet' });

    expect(dialog).toHaveAttribute('data-slot', 'sheet-content');
    expect(dialog).toHaveAttribute('data-state', 'open');
    expect(close).toHaveAttribute('data-slot', 'sheet-close');

    await user.click(close);

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
    expect(screen.queryByRole('dialog', { name: 'Node editor' })).not.toBeInTheDocument();
  });

  it('preserves Radix Escape dismissal for keyboard users', async () => {
    const onOpenChange = vi.fn();
    const { user } = renderWithUser(<ControlledSheet onOpenChange={onOpenChange} />);

    expect(await screen.findByRole('dialog', { name: 'Node editor' })).toBeInTheDocument();
    await user.keyboard('{Escape}');

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
    expect(screen.queryByRole('dialog', { name: 'Node editor' })).not.toBeInTheDocument();
  });
});
