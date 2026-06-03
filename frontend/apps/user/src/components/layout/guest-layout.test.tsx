import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import { GuestLayout } from './guest-layout';

vi.mock('@/lib/legacy-settings', () => ({
  getLegacySettings: () => ({
    background_url: 'https://cdn.example.test/bg.jpg',
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
  it('renders the old auth page container, background, centered box, and child outlet', () => {
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
    expect(renderGuest('/register')).toContain('<div class="" style="max-width:450px;width:100%;margin:auto">');
    expect(renderGuest('/forgetpassword')).toContain('<div class="" style="max-width:450px;width:100%;margin:auto">');
    expect(renderGuest('/login')).not.toContain('<div class="" style="max-width:450px;width:100%;margin:auto">');
  });
});
