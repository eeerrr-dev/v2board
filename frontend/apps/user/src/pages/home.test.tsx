import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import HomePage from './home';

const navigate = vi.hoisted(() => vi.fn());

vi.mock('react-router-dom', () => ({
  useNavigate: () => navigate,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('HomePage legacy root entry', () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    navigate.mockReset();
    window.settings = undefined;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    if (root) {
      act(() => root?.unmount());
      root = null;
    }
    container.remove();
    document.body.innerHTML = '';
    window.settings = undefined;
  });

  it('renders the bundled fallback shell before redirecting to login', async () => {
    const html = renderToStaticMarkup(<HomePage />);

    expect(html).toContain('padding-top:50px');
    expect(html).toContain('href="https://github.com/wyx2685/v2board"');
    expect(html).toContain('v2board');
    expect(html).toContain(' is best.');

    await act(async () => {
      root!.render(<HomePage />);
      await Promise.resolve();
    });

    expect(navigate).toHaveBeenCalledWith('/login');
  });

  it('decodes and renders the configured legacy homepage html', () => {
    window.settings = {
      homepage: window.btoa(encodeURI('<section class="hero">欢迎回来</section>')),
    };

    const html = renderToStaticMarkup(<HomePage />);

    expect(html).toContain('<section class="hero">欢迎回来</section>');
    expect(navigate).not.toHaveBeenCalled();
  });
});
