import { screen, within } from '@testing-library/react';
import { Route, Routes } from 'react-router';
import { describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { GuestLayout } from './guest-layout';

// The operator background_url is always configured in these tests: the
// redesigned auth shell must ignore it, so every render doubles as proof the
// legacy backdrop never reaches the DOM.
vi.mock('@/lib/legacy-settings', () => ({
  getLegacySettings: () => ({
    background_url: 'https://cdn.example.test/bg.jpg',
  }),
  getLegacyTitle: () => 'V2Board',
}));

function renderGuest(path: string) {
  window.settings = {
    i18n: ['en-US', 'zh-CN'] as string[] & Record<string, Record<string, string>>,
  };

  return renderWithProviders(
    <Routes>
      <Route element={<GuestLayout />}>
        <Route path="/login" element={<div>login probe</div>} />
        <Route path="/register" element={<div>register probe</div>} />
        <Route path="/forgetpassword" element={<div>forget probe</div>} />
      </Route>
    </Routes>,
    { i18n: true, routerEntries: [path] },
  );
}

function hasInlineBackground(container: HTMLElement): boolean {
  return Array.from(container.querySelectorAll<HTMLElement>('*')).some(
    (element) => element.style.backgroundImage || element.style.background,
  );
}

describe('GuestLayout auth shell (route-isolated 2026 reskin)', () => {
  it('renders the shadcn auth shell around the routed page and ignores the operator background', () => {
    const { container } = renderGuest('/login');

    // #page-container / #main-container are visual-parity ready-selectors.
    expect(container.querySelector('#page-container')).not.toBeNull();
    const main = container.querySelector('#main-container');
    expect(main).not.toBeNull();
    // The v2board-auth-surface/-frame hooks mark AuthLayout's island shell —
    // their presence is the behavior twin of the retired "GuestLayout
    // delegates to AuthLayout" source pin.
    expect(main).toHaveClass('v2board-auth-surface');
    const frame = container.querySelector<HTMLElement>('.v2board-auth-frame');
    expect(frame).not.toBeNull();
    expect(within(frame!).getByText('login probe')).toBeInTheDocument();

    // Shell chrome: brand wordmark plus the language menu trigger
    // (visual-parity clicks .v2board-auth-language-trigger).
    expect(screen.getByText('V2Board')).toBeInTheDocument();
    expect(container.querySelector('.v2board-auth-language-trigger')).not.toBeNull();

    // The configured background_url never renders: no inline background style
    // anywhere in the shell.
    expect(hasInlineBackground(container)).toBe(false);
  });

  it('uses the same auth shell for register and forgetpassword', () => {
    const cases: Array<[string, string]> = [
      ['/register', 'register probe'],
      ['/forgetpassword', 'forget probe'],
    ];

    for (const [path, probe] of cases) {
      const { container, unmount } = renderGuest(path);

      expect(container.querySelector('#main-container')).toHaveClass('v2board-auth-surface');
      const frame = container.querySelector<HTMLElement>('.v2board-auth-frame');
      expect(frame).not.toBeNull();
      expect(within(frame!).getByText(probe)).toBeInTheDocument();
      expect(hasInlineBackground(container)).toBe(false);

      unmount();
    }
  });
});
