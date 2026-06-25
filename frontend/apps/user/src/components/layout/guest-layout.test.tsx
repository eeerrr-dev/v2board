import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it, vi } from 'vitest';
import { GuestLayout } from './guest-layout';

const guestLayoutSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'guest-layout.tsx'),
  'utf8',
);

const mocks = vi.hoisted(() => ({
  backgroundUrl: 'https://cdn.example.test/bg.jpg',
}));

vi.mock('@/lib/legacy-settings', () => ({
  getLegacySettings: () => ({
    background_url: mocks.backgroundUrl,
  }),
}));

function renderGuest(path: string) {
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
  it('derives the bundled background_url image from a typed ternary (no unsound cast)', () => {
    expect(guestLayoutSource).toContain(
      'const legacyBackgroundImage = backgroundUrl ? `url(${backgroundUrl})` : undefined;',
    );
    expect(guestLayoutSource).not.toContain('`url(${backgroundUrl})`) as string');
  });

  describe('redesigned auth chrome (route-isolated 2026 reskin)', () => {
    it('renders the modern gradient backdrop and drops the legacy background + operator image', () => {
      mocks.backgroundUrl = 'https://cdn.example.test/bg.jpg';
      const html = renderGuest('/login');

      expect(html).toContain('id="page-container"');
      expect(html).toContain('id="main-container"');
      expect(html).toContain('tw:bg-gradient-to-br');
      expect(html).toContain('v2board-login-frame');
      expect(html).toContain('class="guest-probe"');
      // The 2026 presentation hooks: the surface scope (motion + scoped dark theme) and the two
      // ambient aurora blobs. See styles/user-login-surface.css.
      expect(html).toContain('v2board-login-surface');
      expect((html.match(/v2board-login-aurora/g) ?? []).length).toBe(2);
      // Route isolation: the redesigned surface does not use the packaged-oracle flat background
      // layer or the operator background image.
      expect(html).not.toContain('class="v2board-background"');
      expect(html).not.toContain('background-image');
    });

    it('uses the same 2026 presentation hooks for register and forgetpassword', () => {
      for (const path of ['/login', '/register', '/forgetpassword']) {
        const html = renderGuest(path);
        expect(html).toContain('v2board-login-surface');
        expect(html).toContain('v2board-login-frame');
        expect((html.match(/v2board-login-aurora/g) ?? []).length).toBe(2);
        expect(html).not.toContain('class="v2board-background"');
        expect(html).not.toContain('background-image');
      }
    });

    it('keeps exactly one auth box and adds no page-level button (behavior-gate contract)', () => {
      const html = renderGuest('/login');
      // The route chrome must contribute neither a second auth box nor its own controls; the login
      // component owns the redesigned buttons inside the auth box.
      expect((html.match(/v2board-auth-box/g) ?? []).length).toBe(1);
      expect(html).not.toContain('<button');
      expect(html).not.toContain('class="btn');
    });
  });
});
