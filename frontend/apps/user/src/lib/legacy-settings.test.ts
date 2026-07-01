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
    Object.defineProperty(document, 'execCommand', {
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

    const style = document.documentElement.style;
    expect(appendedThemeLink).toBeNull();
    expect(style.getPropertyValue('--color-brand-500')).toBe('#319795');
    expect(style.getPropertyValue('--color-page')).toBe('#f5f5f5');
    expect(style.getPropertyValue('--legacy-link')).toBe('#319795');
    expect(style.getPropertyValue('--legacy-link-hover')).toBe('#184a49');
    expect(style.getPropertyValue('--legacy-link-active')).toBe('#1e6f70');
    // Framework-only legacy variables are no longer written at runtime; they
    // were read solely by the deleted Bootstrap/OneUI CSS.
    expect(style.getPropertyValue('--legacy-ant-radio-focus-shadow')).toBe('');
    expect(style.getPropertyValue('--legacy-nav-link')).toBe('');
    expect(document.title).toBe('Legacy Title');
  });

  it('does not fall back to packaged theme css when service host is configured', () => {
    window.settings = {
      title: 'Hosted',
      host: 'https://example.test',
      theme: { color: 'black', sidebar: 'dark', header: 'dark' },
    };

    applyLegacySettings();

    const style = document.documentElement.style;
    expect(appendedThemeLink).toBeNull();
    expect(style.getPropertyValue('--color-brand-500')).toBe('#343a40');
    expect(style.getPropertyValue('--color-page')).toBe('#f5f5f5');
    expect(style.getPropertyValue('--legacy-link-active')).toBe('#13161a');
  });

  it('falls back to the product name instead of "undefined" for a missing title', () => {
    window.settings = {
      theme: { color: 'default', sidebar: 'light', header: 'dark' },
    };

    applyLegacySettings();

    expect(document.title).toBe('V2Board');
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

  it('falls back to execCommand copy when Clipboard API is unavailable', async () => {
    const execCommand = vi.fn().mockReturnValue(true);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: undefined,
    });
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    });

    await expect(copyText('fallback text')).resolves.toBe(true);

    expect(execCommand).toHaveBeenCalledWith('copy');
    expect(document.querySelector('textarea')).toBeNull();
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

  it('falls back to execCommand copy when Clipboard API write fails', async () => {
    const writeText = vi.fn().mockRejectedValue(new Error('blocked'));
    const execCommand = vi.fn().mockReturnValue(true);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    });

    await expect(copyText('blocked fallback')).resolves.toBe(true);

    expect(writeText).toHaveBeenCalledWith('blocked fallback');
    expect(execCommand).toHaveBeenCalledWith('copy');
  });
});
