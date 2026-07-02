import { act, cleanup, screen } from '@testing-library/react';
import { Route, Routes, useLocation } from 'react-router';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { RequireAuth } from './require-auth';
import { logout, setAuthData } from '@/lib/auth';

function LocationProbe() {
  const location = useLocation();
  return <div data-testid="loc">{`${location.pathname}${location.search}`}</div>;
}

function renderGuarded(entry: string) {
  return renderWithProviders(
    <Routes>
      <Route
        path="/dashboard"
        element={
          <RequireAuth>
            <div>guarded dashboard</div>
          </RequireAuth>
        }
      />
      <Route path="/login" element={<LocationProbe />} />
    </Routes>,
    { routerEntries: [entry] },
  );
}

// The localStorage stub persists per test file, so clear the token around
// every test to keep them order-independent. Unmount before clearing: a
// mounted RequireAuth would react to the store change outside act().
beforeEach(() => setAuthData(null));
afterEach(() => {
  cleanup();
  setAuthData(null);
});

describe('RequireAuth', () => {
  it('renders the guarded children while a token is present', () => {
    setAuthData('token-123');

    renderGuarded('/dashboard');

    expect(screen.getByText('guarded dashboard')).toBeInTheDocument();
  });

  it('redirects an unauthenticated visit to /login with the encoded return path', () => {
    renderGuarded('/dashboard?tab=orders');

    expect(screen.queryByText('guarded dashboard')).not.toBeInTheDocument();
    expect(screen.getByTestId('loc').textContent).toBe(
      `/login?redirect=${encodeURIComponent('/dashboard?tab=orders')}`,
    );
  });

  it('redirects to /login when the session is cleared mid-render (live logout)', () => {
    setAuthData('token-123');

    renderGuarded('/dashboard?tab=orders');
    expect(screen.getByText('guarded dashboard')).toBeInTheDocument();

    // logout() flips the external auth store outside a React event handler.
    act(() => {
      logout();
    });

    expect(screen.queryByText('guarded dashboard')).not.toBeInTheDocument();
    expect(screen.getByTestId('loc').textContent).toBe(
      `/login?redirect=${encodeURIComponent('/dashboard?tab=orders')}`,
    );
  });
});
