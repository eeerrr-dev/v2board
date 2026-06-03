import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RouteErrorBoundary } from './route-error-boundary';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function BrokenPage(): never {
  throw new Error('broken route');
}

describe('RouteErrorBoundary', () => {
  let container: HTMLDivElement;
  let root: Root;
  let consoleError: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    window.history.replaceState(null, '', '/');
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    consoleError.mockRestore();
  });

  it('renders a recovery panel instead of clearing the page when a route throws', async () => {
    await act(async () => {
      root.render(
        <MemoryRouter initialEntries={['/profile']}>
          <RouteErrorBoundary>
            <BrokenPage />
          </RouteErrorBoundary>
        </MemoryRouter>,
      );
    });

    expect(container.textContent).toContain('页面加载失败');
    expect(container.textContent).toContain('已阻止整站白屏');

    await act(async () => {
      container.querySelector<HTMLButtonElement>('.btn-primary')!.click();
    });

    expect(window.location.hash).toBe('#/dashboard');
  });
});
