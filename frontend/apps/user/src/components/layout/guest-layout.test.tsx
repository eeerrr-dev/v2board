import { screen, within } from '@testing-library/react';
import { Route, Routes } from 'react-router';
import { describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { setRuntimeConfig } from '@/test/runtime-config';
import { GuestLayout } from './guest-layout';
import type * as RuntimeConfigModule from '@/lib/runtime-config';

// The operator brand assets are configured in these tests so the pure shadcn
// shell also covers its modern, tokenized customization layer.
vi.mock('@/lib/runtime-config', async (importOriginal) => ({
  ...(await importOriginal<typeof RuntimeConfigModule>()),
  getBackgroundUrl: () => 'https://cdn.example.test/bg.jpg',
  getLogoUrl: () => 'https://cdn.example.test/logo.svg',
  getRuntimeConfig: () => ({ i18n: ['en-US', 'zh-CN'] }),
  getSiteTitle: () => 'V2Board',
}));

function renderGuest(path: string) {
  setRuntimeConfig({
    i18n: ['en-US', 'zh-CN'],
  });

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
  it('renders the shadcn auth shell with tokenized operator brand assets', () => {
    const { container } = renderGuest('/login');

    // #page-container / #main-container are visual-parity ready-selectors.
    expect(container.querySelector('#page-container')).not.toBeNull();
    const main = container.querySelector('#main-container');
    expect(main).not.toBeNull();
    expect(main).toHaveAttribute('data-testid', 'auth-surface');
    const frame = container.querySelector<HTMLElement>('[data-slot="auth-route-frame"]');
    expect(frame).not.toBeNull();
    expect(within(frame!).getByText('login probe')).toBeInTheDocument();

    // Shell chrome: brand wordmark plus the language menu trigger.
    expect(screen.getByText('V2Board')).toBeInTheDocument();
    expect(screen.getByTestId('auth-language-trigger')).toBeInTheDocument();

    const background = container.querySelector('img[src="https://cdn.example.test/bg.jpg"]');
    expect(background).toHaveAttribute('decoding', 'async');
    expect(background).toHaveAttribute('fetchpriority', 'high');
    expect(container.querySelector('img[src="https://cdn.example.test/logo.svg"]')).toHaveAttribute(
      'decoding',
      'async',
    );
    // URLs are assigned through image src attributes, never interpolated into
    // an inline CSS declaration.
    expect(hasInlineBackground(container)).toBe(false);
  });

  it('uses the same auth shell for register and forgetpassword', () => {
    const cases: Array<[string, string]> = [
      ['/register', 'register probe'],
      ['/forgetpassword', 'forget probe'],
    ];

    for (const [path, probe] of cases) {
      const { container, unmount } = renderGuest(path);

      expect(container.querySelector('#main-container')).toHaveAttribute(
        'data-testid',
        'auth-surface',
      );
      const frame = container.querySelector<HTMLElement>('[data-slot="auth-route-frame"]');
      expect(frame).not.toBeNull();
      expect(within(frame!).getByText(probe)).toBeInTheDocument();
      expect(container.querySelector('img[src="https://cdn.example.test/bg.jpg"]')).not.toBeNull();
      expect(hasInlineBackground(container)).toBe(false);

      unmount();
    }
  });
});
