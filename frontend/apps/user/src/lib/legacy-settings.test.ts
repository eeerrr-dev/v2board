import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { applyLegacySettings, copyText } from './legacy-settings';

describe('legacy settings bootstrap', () => {
  let appendedThemeLink: HTMLLinkElement | null;

  beforeEach(() => {
    appendedThemeLink = null;
    document.documentElement.removeAttribute('style');
    document.title = '';
    window.settings = undefined;

    // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
    const getElementsByTagName = document.getElementsByTagName.bind(document);
    vi.spyOn(document, 'getElementsByTagName').mockImplementation((name: string) => {
      if (name.toLowerCase() !== 'head') return getElementsByTagName(name);
      return [
        {
          appendChild: (element: Node) => {
            appendedThemeLink = element as HTMLLinkElement;
            return element;
          },
        },
      ] as unknown as HTMLCollectionOf<HTMLHeadElement>;
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = '';
    Object.defineProperty(window, 'isSecureContext', {
      configurable: true,
      value: false,
    });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: undefined,
    });
  });

  it('applies the selected theme from source variables without loading packaged css', () => {
    window.settings = {
      title: 'Legacy Title',
      theme: { color: 'green', sidebar: 'light', header: 'dark' },
    };

    applyLegacySettings();

    expect(appendedThemeLink).toBeNull();
    expect(document.documentElement.style.getPropertyValue('--legacy-ant-radio-focus-shadow')).toBe(
      'rgba(49, 151, 149, 0.08)',
    );
    expect(
      document.documentElement.style.getPropertyValue('--legacy-ant-radio-button-focus-shadow'),
    ).toBe('rgba(49, 151, 149, 0.06)');
    expect(document.title).toBe('Legacy Title');
  });

  it('does not fall back to packaged theme css when service host is configured', () => {
    window.settings = {
      title: 'Hosted',
      host: 'https://example.test',
      theme: { color: 'black', sidebar: 'dark', header: 'dark' },
    };

    applyLegacySettings();

    expect(appendedThemeLink).toBeNull();
    expect(document.documentElement.style.getPropertyValue('--legacy-ant-radio-focus-shadow')).toBe(
      'rgba(52, 58, 64, 0.08)',
    );
  });

  it('matches the legacy title assignment for a missing title', () => {
    window.settings = {
      theme: { color: 'default', sidebar: 'light', header: 'dark' },
    };

    applyLegacySettings();

    expect(document.title).toBe('undefined');
  });

  it('uses the modern Clipboard API', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });

    await expect(copyText('modern text')).resolves.toBe(true);

    expect(writeText).toHaveBeenCalledWith('modern text');
  });

  it('returns false when the Clipboard API is unavailable', async () => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: undefined,
    });

    await expect(copyText('no clipboard')).resolves.toBe(false);
  });

  it('returns false when Clipboard API write fails', async () => {
    const writeText = vi.fn().mockRejectedValue(new Error('blocked'));
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });

    await expect(copyText('blocked text')).resolves.toBe(false);

    expect(writeText).toHaveBeenCalledWith('blocked text');
  });
});
