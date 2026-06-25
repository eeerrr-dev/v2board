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

describe('GuestLayout bundled-theme auth shell', () => {
  it('derives the bundled background_url image from a typed ternary (no unsound cast)', () => {
    expect(guestLayoutSource).toContain(
      'const legacyBackgroundImage = backgroundUrl ? `url(${backgroundUrl})` : undefined;',
    );
    expect(guestLayoutSource).not.toContain('`url(${backgroundUrl})`) as string');
  });

  it('renders the old auth page container, background, centered box, and child outlet', () => {
    mocks.backgroundUrl = 'https://cdn.example.test/bg.jpg';
    const html = renderGuest('/register');

    expect(html).toContain('id="page-container"');
    expect(html).toContain('id="main-container"');
    expect(html).toContain('class="v2board-background"');
    expect(html).toContain('style="background-image:url(https://cdn.example.test/bg.jpg)"');
    expect(html).toContain('class="no-gutters v2board-auth-box"');
    expect(html).toContain('style="max-width:450px;width:100%;margin:auto"');
    expect(html).toContain('class="mx-2 mx-sm-0"');
    expect(html).toContain('class="guest-probe"');
  });

  it('keeps the old empty class attribute for register and forgetpassword auth boxes only', () => {
    mocks.backgroundUrl = 'https://cdn.example.test/bg.jpg';
    expect(renderGuest('/register')).toContain('<div class="" style="max-width:450px;width:100%;margin:auto">');
    expect(renderGuest('/forgetpassword')).toContain('<div class="" style="max-width:450px;width:100%;margin:auto">');
    expect(renderGuest('/login')).not.toContain('<div class="" style="max-width:450px;width:100%;margin:auto">');
  });

  describe('redesigned /login chrome (route-isolated 2026 reskin)', () => {
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

    it('keeps the 2026 presentation hooks off the still-pixel-gated register/forget chrome', () => {
      expect(renderGuest('/register')).not.toContain('v2board-login-surface');
      expect(renderGuest('/register')).not.toContain('v2board-login-aurora');
      expect(renderGuest('/forgetpassword')).not.toContain('v2board-login-surface');
      expect(renderGuest('/forgetpassword')).not.toContain('v2board-login-aurora');
    });

    it('keeps exactly one auth box and adds no page-level button (behavior-gate contract)', () => {
      const html = renderGuest('/login');
      // user-home-root-page-state asserts authBoxCount === 1 and compares the page-wide button set
      // to the oracle; the redesigned chrome must contribute neither a second auth box nor a button.
      expect((html.match(/v2board-auth-box/g) ?? []).length).toBe(1);
      expect(html).not.toContain('<button');
      expect(html).not.toContain('class="btn');
    });
  });
});
