import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { copyText } from '@v2board/config/clipboard';
import { setRuntimeConfig } from '@/test/runtime-config';
import {
  applyRuntimeConfig,
  getBackgroundUrl,
  getLegacyHashRedirectEnabled,
  getLogoUrl,
  getRuntimeConfig,
} from './runtime-config';

describe('runtime config bootstrap', () => {
  beforeEach(() => {
    document.documentElement.classList.remove('dark');
    document.documentElement.removeAttribute('data-theme-color');
    document.title = '';
    setRuntimeConfig();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    setRuntimeConfig();
    document.body.innerHTML = '';
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: undefined });
  });

  it('maps the operator theme to shadcn tokens without loading a stylesheet', () => {
    setRuntimeConfig({
      description: 'Native user application',
      title: 'Panel',
      theme: { color: 'green' },
    });

    applyRuntimeConfig();

    expect(document.documentElement.dataset.themeColor).toBe('green');
    expect(document.querySelector('link[data-v2board-theme-color]')).toBeNull();
    expect(document.title).toBe('Panel');
    expect(document.querySelector('meta[name="description"]')).toHaveAttribute(
      'content',
      'Native user application',
    );
    expect(document.querySelector('meta[name="theme-color"]')).toHaveAttribute(
      'content',
      '#319795',
    );

    document.documentElement.classList.add('dark');
    applyRuntimeConfig();
    expect(document.querySelector('meta[name="theme-color"]')).toHaveAttribute(
      'content',
      '#171717',
    );
  });

  it('falls back to a complete default config when the backend token is not replaced', () => {
    const element = document.createElement('script');
    element.id = 'v2board-runtime-config';
    element.type = 'application/json';
    element.textContent = '__V2BOARD_RUNTIME_CONFIG__';
    document.head.append(element);

    applyRuntimeConfig();

    expect(document.documentElement.dataset.themeColor).toBe('default');
    expect(document.title).toBe('V2Board');
    expect(getRuntimeConfig().i18n).toContain('zh-CN');
    // Mirrors the Rust config default (docs/api-dialect.md §10.3: default ON).
    expect(getLegacyHashRedirectEnabled()).toBe(true);
  });

  it('reads the injected legacy-hash-redirect toggle (docs/api-dialect.md §10.3)', () => {
    setRuntimeConfig({ legacy_hash_redirect_enable: false });
    expect(getLegacyHashRedirectEnabled()).toBe(false);

    setRuntimeConfig({ legacy_hash_redirect_enable: true });
    expect(getLegacyHashRedirectEnabled()).toBe(true);
  });

  it('accepts web and relative operator images but rejects active URL schemes', () => {
    setRuntimeConfig({
      background_url: 'https://cdn.example.test/background.jpg',
      logo: '/brand.svg',
    });
    expect(getBackgroundUrl()).toBe('https://cdn.example.test/background.jpg');
    expect(getLogoUrl()).toBe('/brand.svg');

    setRuntimeConfig({
      background_url: 'javascript:alert(1)',
      logo: 'data:image/svg+xml,<svg/>',
    });
    expect(getBackgroundUrl()).toBe('');
    expect(getLogoUrl()).toBe('');
  });

  it('uses the Clipboard API', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });

    await expect(copyText('modern text')).resolves.toBe(true);
    expect(writeText).toHaveBeenCalledWith('modern text');
  });

  it('returns false when no copy API is available', async () => {
    await expect(copyText('no clipboard')).resolves.toBe(false);
  });

  it('reports denied Clipboard access without a deprecated fallback', async () => {
    const writeText = vi.fn().mockRejectedValue(new Error('blocked'));
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });

    await expect(copyText('blocked text')).resolves.toBe(false);
    expect(writeText).toHaveBeenCalledWith('blocked text');
  });
});
