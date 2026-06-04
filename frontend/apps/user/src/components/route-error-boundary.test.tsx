import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RouteErrorBoundary } from './route-error-boundary';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function Crash() {
  throw new Error('route crashed');
  return null;
}

describe('RouteErrorBoundary white-screen guard', () => {
  let container: HTMLDivElement;
  let root: Root;
  let consoleError: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    consoleError.mockRestore();
  });

  it('renders a route-local fallback and recovers after the reset key changes', async () => {
    await act(async () => {
      root.render(
        <RouteErrorBoundary resetKey="/plan">
          <Crash />
        </RouteErrorBoundary>,
      );
      await Promise.resolve();
    });

    expect(container.textContent).toContain('页面加载失败');
    expect(container.textContent).toContain('刷新页面');

    await act(async () => {
      root.render(
        <RouteErrorBoundary resetKey="/dashboard">
          <div>Recovered route</div>
        </RouteErrorBoundary>,
      );
      await Promise.resolve();
    });

    expect(container.textContent).toContain('Recovered route');
    expect(container.textContent).not.toContain('页面加载失败');
  });
});
