import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { RequireAuth } from './require-auth';
import { logout, setAuthData } from '@/lib/auth';

function LocationProbe() {
  const location = useLocation();
  return <div data-testid="loc">{`${location.pathname}${location.search}`}</div>;
}

function renderGuarded(root: Root, entry: string) {
  act(() => {
    root.render(
      <MemoryRouter initialEntries={[entry]}>
        <Routes>
          <Route
            path="/dashboard"
            element={
              <RequireAuth>
                <div data-testid="guarded">dashboard</div>
              </RequireAuth>
            }
          />
          <Route path="/login" element={<LocationProbe />} />
        </Routes>
      </MemoryRouter>,
    );
  });
}

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  setAuthData(null);
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  setAuthData(null);
});

describe('RequireAuth', () => {
  it('renders the guarded children while a token is present', () => {
    setAuthData('token-123');

    renderGuarded(root, '/dashboard');

    expect(container.querySelector('[data-testid="guarded"]')?.textContent).toBe('dashboard');
  });

  it('redirects an unauthenticated visit to /login with the encoded return path', () => {
    renderGuarded(root, '/dashboard?tab=orders');

    expect(container.querySelector('[data-testid="guarded"]')).toBeNull();
    expect(container.querySelector('[data-testid="loc"]')?.textContent).toBe(
      `/login?redirect=${encodeURIComponent('/dashboard?tab=orders')}`,
    );
  });

  it('redirects to /login when the session is cleared mid-render (live logout)', () => {
    setAuthData('token-123');

    renderGuarded(root, '/dashboard?tab=orders');
    expect(container.querySelector('[data-testid="guarded"]')).not.toBeNull();

    act(() => {
      logout();
    });

    expect(container.querySelector('[data-testid="guarded"]')).toBeNull();
    expect(container.querySelector('[data-testid="loc"]')?.textContent).toBe(
      `/login?redirect=${encodeURIComponent('/dashboard?tab=orders')}`,
    );
  });
});
