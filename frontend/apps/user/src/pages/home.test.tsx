// @vitest-environment jsdom
import { screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import HomePage from './home';

const navigate = vi.hoisted(() => vi.fn());

vi.mock('react-router', () => ({
  useNavigate: () => navigate,
}));

describe('HomePage legacy root entry', () => {
  beforeEach(() => {
    navigate.mockReset();
    window.settings = undefined;
  });

  afterEach(() => {
    window.settings = undefined;
  });

  it('renders the bundled fallback shell and redirects to login', () => {
    renderWithProviders(<HomePage />);

    expect(screen.getByRole('link', { name: 'v2board' })).toHaveAttribute(
      'href',
      'https://github.com/wyx2685/v2board',
    );
    expect(screen.getByText(/is best\./)).toBeInTheDocument();
    expect(navigate).toHaveBeenCalledWith('/login');
  });

  it('decodes and renders the configured legacy homepage html', () => {
    window.settings = {
      homepage: window.btoa(encodeURI('<section class="hero">欢迎回来</section>')),
    };

    renderWithProviders(<HomePage />);

    const section = screen.getByText('欢迎回来');
    expect(section.tagName).toBe('SECTION');
    expect(section).toHaveClass('hero');
    expect(navigate).not.toHaveBeenCalled();
  });

  it('sanitizes configured legacy homepage html before rendering', () => {
    window.settings = {
      homepage: window.btoa(encodeURI('<section onclick="alert(1)">欢迎回来</section>')),
    };

    renderWithProviders(<HomePage />);

    const section = screen.getByText('欢迎回来');
    expect(section.tagName).toBe('SECTION');
    expect(section).not.toHaveAttribute('onclick');
    expect(navigate).not.toHaveBeenCalled();
  });
});
