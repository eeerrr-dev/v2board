import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { applyLegacySettings, legacyCopyText } from './legacy-settings';

describe('legacy settings bootstrap', () => {
  let appendedThemeLink: HTMLLinkElement | null;

  beforeEach(() => {
    appendedThemeLink = null;
    document.documentElement.removeAttribute('style');
    document.title = '';
    window.settings = undefined;

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
  });

  it('uses the packaged theme css path when no service host is configured', () => {
    window.settings = {
      title: 'Legacy Title',
      theme: { color: 'green', sidebar: 'light', header: 'dark' },
    };

    applyLegacySettings();

    const link = appendedThemeLink;
    expect(link?.rel).toBe('stylesheet');
    expect(link?.hasAttribute('data-v2board-theme-color')).toBe(false);
    expect(link?.getAttribute('href')).toBe('/theme/default/assets/theme/green.css');
    expect(document.documentElement.style.getPropertyValue('--legacy-ant-radio-focus-shadow')).toBe(
      'rgba(49, 151, 149, 0.08)',
    );
    expect(
      document.documentElement.style.getPropertyValue('--legacy-ant-radio-button-focus-shadow'),
    ).toBe('rgba(49, 151, 149, 0.06)');
    expect(document.title).toBe('Legacy Title');
  });

  it('uses the relative theme css path when service host is configured', () => {
    window.settings = {
      title: 'Hosted',
      host: 'https://example.test',
      theme: { color: 'black', sidebar: 'dark', header: 'dark' },
    };

    applyLegacySettings();

    const link = appendedThemeLink;
    expect(link?.getAttribute('href')).toBe('./theme/black.css');
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

    legacyCopyText('legacy text');

    expect(execCommand).toHaveBeenCalledWith('copy');
  });
});
