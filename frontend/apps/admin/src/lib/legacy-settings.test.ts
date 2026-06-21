import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  applyAdminLegacySettings,
  getAdminApiBaseUrl,
  getAdminBackgroundUrl,
  getAdminLogo,
  getAdminSecurePath,
  getAdminTitle,
} from './legacy-settings';

const legacySettingsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'legacy-settings.ts'),
  'utf8',
);

describe('admin legacy settings', () => {
  let appendedThemeLink: HTMLLinkElement | null;

  beforeEach(() => {
    appendedThemeLink = null;
    vi.spyOn(HTMLHeadElement.prototype, 'appendChild').mockImplementation(function appendChild(node) {
      if (node instanceof HTMLLinkElement) appendedThemeLink = node;
      return node;
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    window.settings = undefined;
    document.title = '';
  });

  it('reads title, logo, background and secure path from window.settings', () => {
    window.settings = {
      title: 'Panel',
      host: 'https://api.example.com',
      logo: '/logo.png',
      background_url: '/bg.jpg',
      secure_path: '/admin',
    };

    expect(getAdminTitle()).toBe('Panel');
    expect(getAdminLogo()).toBe('/logo.png');
    expect(getAdminBackgroundUrl()).toBe('/bg.jpg');
    expect(getAdminApiBaseUrl()).toBe('https://api.example.com/api/v1');
    expect(getAdminSecurePath()).toBe('admin');
  });

  it('falls back to location.origin for the admin API base URL', () => {
    window.settings = { host: '', secure_path: 'admin' };

    expect(getAdminApiBaseUrl()).toBe(`${new URL(window.location.href).origin}/api/v1`);
  });

  it('uses the same single slash replacement behavior as the original admin bundle', () => {
    window.settings = { secure_path: '/secure/path' };

    expect(getAdminSecurePath()).toBe('secure/path');
  });

  it('applies the original admin bootstrap side effects', () => {
    window.settings = { title: 'Legacy Admin', secure_path: '/admin', theme: { color: 'green' } };

    applyAdminLegacySettings();

    expect(window.settings.secure_path).toBe('admin');
    expect(document.title).toBe('Legacy Admin');
    expect(appendedThemeLink?.rel).toBe('stylesheet');
    expect(appendedThemeLink?.getAttribute('data-v2board-admin-theme-color')).toBe('green');
    expect(appendedThemeLink?.getAttribute('href')).toContain('/src/styles/themes/green.css');
    expect(appendedThemeLink?.getAttribute('href')).not.toContain('/assets/admin/theme/');
  });

  it('stringifies a missing title the same way as the packaged admin script', () => {
    window.settings = { secure_path: '/admin', theme: { color: 'default' } };

    applyAdminLegacySettings();

    expect(document.title).toBe('undefined');
  });

  it('does not load packaged theme css when window.settings.host is set', () => {
    window.settings = {
      title: 'Legacy Admin',
      host: 'https://api.example.com',
      secure_path: 'admin',
      theme: { color: 'darkblue' },
    };

    applyAdminLegacySettings();

    expect(appendedThemeLink?.getAttribute('data-v2board-admin-theme-color')).toBe('darkblue');
    expect(appendedThemeLink?.getAttribute('href')).toContain('/src/styles/themes/darkblue.css');
    expect(appendedThemeLink?.getAttribute('href')).not.toContain('/assets/admin/theme/');
    expect(document.head.innerHTML).not.toContain('/assets/admin/theme/');
    expect(document.head.innerHTML).not.toContain('./theme/');
  });

  it('uses stable source and deploy theme paths instead of bundled css url imports', () => {
    expect(legacySettingsSource).toContain("'/src/styles/themes'");
    expect(legacySettingsSource).toContain("'/assets/admin/themes'");
    expect(legacySettingsSource).toContain("['black', 'darkblue', 'default', 'green']");
    expect(legacySettingsSource).not.toContain('.css?url');
    expect(legacySettingsSource).not.toContain("new URL('../styles/themes/");
    expect(legacySettingsSource).not.toContain('/assets/admin/theme/');
  });
});
