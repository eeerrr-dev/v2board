import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter, Route, Routes } from 'react-router';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it, vi } from 'vitest';
import { GuestLayout } from './guest-layout';

const guestLayoutSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'guest-layout.tsx'),
  'utf8',
);
const authLayoutSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../pages/auth/auth-layout.tsx'),
  'utf8',
);

const mocks = vi.hoisted(() => ({
  backgroundUrl: 'https://cdn.example.test/bg.jpg',
}));

vi.mock('@/lib/legacy-settings', () => ({
  getLegacySettings: () => ({
    background_url: mocks.backgroundUrl,
  }),
  getLegacyTitle: () => 'V2Board',
}));

function renderGuest(path: string) {
  window.g_lang = 'zh-CN';
  window.settings = {
    i18n: ['en-US', 'zh-CN'] as string[] & Record<string, Record<string, string>>,
  };

  return renderToStaticMarkup(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route element={<GuestLayout />}>
          <Route path="/login" element={<div className="guest-probe">login</div>} />
          <Route path="/register" element={<div className="guest-probe">register</div>} />
          <Route path="/forgetpassword" element={<div className="guest-probe">forget</div>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe('GuestLayout auth shell', () => {
  it('delegates the background-free auth shell to AuthLayout', () => {
    expect(guestLayoutSource).toContain("import { AuthLayout } from '@/pages/auth/auth-layout';");
    expect(guestLayoutSource).toContain('return <AuthLayout />;');
    expect(guestLayoutSource).not.toContain('backgroundUrl');
    expect(guestLayoutSource).not.toContain('background_url');
    expect(authLayoutSource).not.toContain('backgroundUrl');
    expect(authLayoutSource).not.toContain('background_url');
  });

  describe('redesigned auth chrome (route-isolated 2026 reskin)', () => {
    it('renders the shadcn auth surface and drops the legacy background + operator image', () => {
      mocks.backgroundUrl = 'https://cdn.example.test/bg.jpg';
      const html = renderGuest('/login');

      expect(html).toContain('id="page-container"');
      expect(html).toContain('id="main-container"');
      expect(html).toContain('bg-muted');
      expect(html).toContain('v2board-auth-frame');
      expect(html).toContain('class="guest-probe"');
      expect(html).toContain('v2board-auth-surface');
      expect(html).not.toContain('v2board-auth-backdrop');
      expect(html).not.toContain('v2board-auth-box');
      expect(html).not.toContain('class="v2board-background"');
      expect(html).not.toContain('background-image');
      expect(guestLayoutSource).not.toContain('tw:fixed tw:inset-0');
      expect(guestLayoutSource).not.toContain('tw:p-4');
    });

    it('uses the same 2026 presentation hooks for register and forgetpassword', () => {
      for (const path of ['/login', '/register', '/forgetpassword']) {
        const html = renderGuest(path);
        expect(html).toContain('v2board-auth-surface');
        expect(html).toContain('v2board-auth-frame');
        expect(html).not.toContain('v2board-auth-backdrop');
        expect(html).not.toContain('v2board-auth-box');
        expect(html).not.toContain('class="v2board-background"');
        expect(html).not.toContain('background-image');
      }
    });

    it('keeps the centered auth shell free of page-level legacy chrome', () => {
      const html = renderGuest('/login');
      expect(html).toContain('v2board-auth-shell-brand');
      expect(html).toContain('>V2Board</div>');
      expect(html).toContain('v2board-auth-language-trigger');
      expect(html).not.toContain('v2board-auth-chrome');
      expect(html).not.toContain('v2board-auth-box');
      expect(html).not.toContain('class="btn');
      expect(html).not.toContain('href="#/login"');
    });
  });
});
