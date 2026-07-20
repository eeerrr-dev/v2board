import { screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { RouteErrorBoundary } from './route-error-boundary';

const reportBoundaryError = vi.hoisted(() => vi.fn());
vi.mock('@/lib/error-reporting', () => ({ reportBoundaryError }));

function Crash(): never {
  throw new Error('route crashed');
}

describe('RouteErrorBoundary white-screen guard', () => {
  it('renders a route-local fallback and recovers after the reset key changes', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});

    const { i18n, rerender } = renderWithProviders(
      <RouteErrorBoundary resetKey="/plan">
        <Crash />
      </RouteErrorBoundary>,
      { i18n: true },
    );

    const failedLabel = i18n!.t(($) => $.common.route_load_failed);
    expect(screen.getByRole('alert')).toHaveTextContent(failedLabel);
    expect(reportBoundaryError).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'route crashed' }),
      expect.any(String),
    );
    expect(screen.getByRole('alert')).toHaveTextContent(
      i18n!.t(($) => $.common.route_refresh_hint),
    );
    expect(
      screen.getByRole('button', { name: i18n!.t(($) => $.common.refresh_page) }),
    ).toBeInTheDocument();

    rerender(
      <RouteErrorBoundary resetKey="/dashboard">
        <div>Recovered route</div>
      </RouteErrorBoundary>,
    );

    expect(screen.getByText('Recovered route')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByText(failedLabel)).not.toBeInTheDocument();

    consoleError.mockRestore();
  });

  it('reloads the page from the fallback button', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => {});

    const { i18n, user } = renderWithProviders(
      <RouteErrorBoundary resetKey="/plan">
        <Crash />
      </RouteErrorBoundary>,
      { i18n: true },
    );

    await user.click(screen.getByRole('button', { name: i18n!.t(($) => $.common.refresh_page) }));

    expect(reload).toHaveBeenCalledTimes(1);

    reload.mockRestore();
    consoleError.mockRestore();
  });
});
