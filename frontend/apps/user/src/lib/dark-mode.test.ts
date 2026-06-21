import { disable as disableDarkReader, enable as enableDarkReader } from 'darkreader';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { applyInitialDarkMode, isDarkModeEnabled, setDarkMode } from './dark-mode';

vi.mock('darkreader', () => ({
  disable: vi.fn(),
  enable: vi.fn(),
}));

const disableDarkReaderMock = vi.mocked(disableDarkReader);
const enableDarkReaderMock = vi.mocked(enableDarkReader);

describe('dark mode legacy storage', () => {
  beforeEach(() => {
    document.cookie = 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    document.documentElement.className = '';
    window.localStorage.clear();
    disableDarkReaderMock.mockClear();
    enableDarkReaderMock.mockClear();
  });

  it('reads dark mode from the legacy cookie', () => {
    document.cookie = 'dark_mode=1;path=/';
    window.localStorage.setItem('dark_mode', '0');

    expect(isDarkModeEnabled()).toBe(true);
  });

  it('ignores malformed legacy dark mode cookie encoding during startup', () => {
    document.cookie = 'dark_mode=%E0%A4%A;path=/';

    expect(isDarkModeEnabled()).toBe(false);
    expect(() => applyInitialDarkMode()).not.toThrow();
    expect(document.documentElement.classList.contains('v2board-dark-mode')).toBe(false);
    expect(enableDarkReaderMock).not.toHaveBeenCalled();
  });

  it('enables DarkReader with the original options and writes the legacy cookie', () => {
    setDarkMode(true);

    expect(document.cookie).toContain('dark_mode=1');
    expect(window.localStorage.getItem('dark_mode')).toBeNull();
    expect(enableDarkReaderMock).toHaveBeenCalledWith({ brightness: 100, contrast: 90, sepia: 10 });
  });

  it('disables DarkReader and keeps the original cookie key', () => {
    setDarkMode(false);

    expect(document.cookie).toContain('dark_mode=0');
    expect(disableDarkReaderMock).toHaveBeenCalledOnce();
  });

  it('matches legacy startup by not disabling DarkReader when the cookie is absent', () => {
    applyInitialDarkMode();

    expect(enableDarkReaderMock).not.toHaveBeenCalled();
    expect(disableDarkReaderMock).not.toHaveBeenCalled();
  });

  it('enables DarkReader on startup when the cookie is set', () => {
    document.cookie = 'dark_mode=1;path=/';

    applyInitialDarkMode();

    expect(enableDarkReaderMock).toHaveBeenCalledWith({ brightness: 100, contrast: 90, sepia: 10 });
    expect(disableDarkReaderMock).not.toHaveBeenCalled();
  });
});
