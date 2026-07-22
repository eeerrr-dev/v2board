import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { AppShellBoundary } from './app-shell-boundary';

const reportBoundaryError = vi.hoisted(() => vi.fn());
vi.mock('./error-reporting', () => ({ reportBoundaryError }));

function Crash(): never {
  throw new Error('shell crashed');
}

describe('AppShellBoundary last-resort guard', () => {
  it('renders a provider-free fallback and reports the crash', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => {});
    const getSentryDsn = vi.fn<() => string | undefined>();

    // Rendered without i18n/router/query providers on purpose: the shell
    // boundary must work when those are exactly what crashed.
    render(
      <AppShellBoundary getSentryDsn={getSentryDsn}>
        <Crash />
      </AppShellBoundary>,
    );

    expect(screen.getByTestId('app-shell-error')).toBeInTheDocument();
    expect(reportBoundaryError).toHaveBeenCalledWith(
      getSentryDsn,
      expect.objectContaining({ message: 'shell crashed' }),
      expect.any(String),
    );

    await userEvent.click(screen.getByRole('button'));
    expect(reload).toHaveBeenCalledTimes(1);

    reload.mockRestore();
    consoleError.mockRestore();
  });
});
