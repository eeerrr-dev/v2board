import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { applyLegacySettings, legacyCopyText } from './legacy-settings';

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

  it('uses the legacy copy marker selection styles', () => {
    const execCommand = vi.fn(() => {
      const mark = document.body.querySelector('span') as HTMLSpanElement | null;
      // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
      expect(mark?.style.webkitUserSelect).toBe('text');
      expect((mark?.style as CSSStyleDeclaration & { MozUserSelect?: string }).MozUserSelect).toBe(
        'text',
      );
      expect((mark?.style as CSSStyleDeclaration & { msUserSelect?: string }).msUserSelect).toBe(
        'text',
      );
      expect(mark?.style.userSelect).toBe('text');
      return true;
    });
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    });

    expect(legacyCopyText('legacy text')).toBe(true);

    expect(execCommand).toHaveBeenCalledWith('copy');
    expect(document.querySelector('span')).toBeNull();
  });

  it('falls back to the old clipboardData copy path when execCommand fails', () => {
    const execCommand = vi.fn(() => false);
    const setData = vi.fn();
    const prompt = vi.fn();
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    });
    Object.defineProperty(window, 'clipboardData', {
      configurable: true,
      value: { setData },
    });
    Object.defineProperty(window, 'prompt', {
      configurable: true,
      value: prompt,
    });

    expect(legacyCopyText('fallback text')).toBe(true);

    expect(execCommand).toHaveBeenCalledWith('copy');
    expect(setData).toHaveBeenCalledWith('text', 'fallback text');
    expect(prompt).not.toHaveBeenCalled();
    expect(document.querySelector('span')).toBeNull();
  });

  it('falls back to the original copy prompt when clipboardData is unavailable', () => {
    const execCommand = vi.fn(() => false);
    const prompt = vi.fn();
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    });
    Object.defineProperty(window, 'clipboardData', {
      configurable: true,
      value: undefined,
    });
    Object.defineProperty(window, 'prompt', {
      configurable: true,
      value: prompt,
    });

    expect(legacyCopyText('manual text')).toBe(false);

    expect(prompt).toHaveBeenCalledWith(expect.stringContaining('Copy to clipboard:'), 'manual text');
    expect(document.querySelector('span')).toBeNull();
  });
});
