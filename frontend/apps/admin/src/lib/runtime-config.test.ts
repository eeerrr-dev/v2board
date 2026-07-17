import { afterEach, describe, expect, it, vi } from 'vitest';
import { setAdminRuntimeConfig } from '@/test/runtime-config';
import {
  applyAdminRuntimeConfig,
  getAdminApiBaseUrl,
  getAdminBackgroundUrl,
  getAdminBasename,
  getAdminLogo,
  getAdminSecurePath,
  getAdminTitle,
  getLegacyHashRedirectEnabled,
} from './runtime-config';

describe('admin runtime config', () => {
  afterEach(() => {
    setAdminRuntimeConfig();
    document.documentElement.classList.remove('dark');
    document.documentElement.removeAttribute('data-theme-color');
    document.title = '';
    vi.unstubAllEnvs();
  });

  it('reads title, logo, background and secure path from the JSON bootstrap', () => {
    setAdminRuntimeConfig({
      title: 'Panel',
      logo: '/logo.png',
      background_url: '/bg.jpg',
      secure_path: '/admin',
    });

    expect(getAdminTitle()).toBe('Panel');
    expect(getAdminLogo()).toBe('/logo.png');
    expect(getAdminBackgroundUrl()).toBe('/bg.jpg');
    expect(getAdminApiBaseUrl()).toBe(`${new URL(window.location.href).origin}/api/v1`);
    expect(getAdminSecurePath()).toBe('admin');
    // The history-router basename derives from the same injected secure_path
    // (docs/api-dialect.md §10.1).
    expect(getAdminBasename()).toBe('/admin');
  });

  it('reads the injected legacy-hash-redirect toggle (docs/api-dialect.md §10.3)', () => {
    // Mirrors the Rust config default (default ON) when the key is absent.
    setAdminRuntimeConfig({});
    expect(getLegacyHashRedirectEnabled()).toBe(true);

    setAdminRuntimeConfig({ legacy_hash_redirect_enable: false });
    expect(getLegacyHashRedirectEnabled()).toBe(false);
  });

  it('falls back safely when the backend token is not replaced', async () => {
    // The absent-bootstrap secure_path fallback follows VITE_DEV_ADMIN_PATH
    // (captured at module load), so pin the plumbing with a stubbed env and a
    // fresh module instance instead of the invoking environment's value.
    vi.resetModules();
    vi.stubEnv('VITE_DEV_ADMIN_PATH', 'stubbed-admin');
    const element = document.createElement('script');
    element.id = 'v2board-runtime-config';
    element.type = 'application/json';
    element.textContent = '__V2BOARD_RUNTIME_CONFIG__';
    document.head.append(element);

    const fresh = await import('./runtime-config');
    expect(fresh.getAdminRuntimeConfig()).toMatchObject({
      title: 'V2Board',
      secure_path: 'stubbed-admin',
    });
    expect(fresh.getAdminApiBaseUrl()).toBe(`${new URL(window.location.href).origin}/api/v1`);
  });

  it('keeps bootstrap config immutable and applies a token theme without a stylesheet link', () => {
    setAdminRuntimeConfig({
      description: 'Native administration',
      title: 'Admin',
      secure_path: '/admin',
      theme: { color: 'green' },
    });

    applyAdminRuntimeConfig();

    expect(getAdminSecurePath()).toBe('admin');
    expect(document.title).toBe('Admin');
    expect(document.documentElement.dataset.themeColor).toBe('green');
    expect(document.querySelector('link[data-v2board-admin-theme-color]')).toBeNull();
    expect(document.querySelector('meta[name="description"]')).toHaveAttribute(
      'content',
      'Native administration',
    );
    expect(document.querySelector('meta[name="theme-color"]')).toHaveAttribute(
      'content',
      '#319795',
    );

    document.documentElement.classList.add('dark');
    applyAdminRuntimeConfig();
    expect(document.querySelector('meta[name="theme-color"]')).toHaveAttribute(
      'content',
      '#171717',
    );
  });

  it('rejects active URL schemes in operator-provided images', () => {
    setAdminRuntimeConfig({
      background_url: 'javascript:alert(1)',
      logo: 'data:image/svg+xml,<svg/>',
    });

    expect(getAdminBackgroundUrl()).toBe('');
    expect(getAdminLogo()).toBe('');
  });

  it('uses the default token palette for an unknown theme', () => {
    setAdminRuntimeConfig({
      title: 'Admin',
      secure_path: 'admin',
      theme: { color: 'unsupported' },
    });

    applyAdminRuntimeConfig();

    expect(document.documentElement.dataset.themeColor).toBe('default');
  });
});
