import { beforeEach, describe, expect, it } from 'vitest';
import { applyInitialDarkMode, isDarkModeEnabled, setDarkMode } from './dark-mode';

describe('dark mode legacy storage', () => {
  beforeEach(() => {
    document.cookie = 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    document.documentElement.className = '';
    window.localStorage.clear();
    window.webpackJsonp = undefined;
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
  });

  it('writes the legacy cookie without using localStorage', () => {
    setDarkMode(true);

    expect(document.cookie).toContain('dark_mode=1');
    expect(window.localStorage.getItem('dark_mode')).toBeNull();
    expect(document.documentElement.classList.contains('v2board-dark-mode')).toBe(true);
  });

  it('uses the legacy bundled DarkReader module when it is loaded', () => {
    let enabledWith: unknown;
    let disabled = false;
    window.webpackJsonp = [
      [
        [2],
        {
          nDCI: (
            _module: { exports: unknown },
            exports: Record<string, unknown>,
          ) => {
            exports.enable = (options: unknown) => {
              enabledWith = options;
            };
            exports.disable = () => {
              disabled = true;
            };
          },
        },
      ],
    ];

    setDarkMode(true);
    expect(enabledWith).toEqual({ brightness: 100, contrast: 90, sepia: 10 });
    expect(document.documentElement.classList.contains('v2board-dark-mode')).toBe(false);

    setDarkMode(false);
    expect(disabled).toBe(true);
  });

  it('matches the legacy startup by not disabling DarkReader when the cookie is absent', () => {
    let enabled = false;
    let disabled = false;
    window.webpackJsonp = [
      [
        [2],
        {
          nDCI: (
            _module: { exports: unknown },
            exports: Record<string, unknown>,
          ) => {
            exports.enable = () => {
              enabled = true;
            };
            exports.disable = () => {
              disabled = true;
            };
          },
        },
      ],
    ];

    applyInitialDarkMode();

    expect(enabled).toBe(false);
    expect(disabled).toBe(false);
    expect(document.documentElement.classList.contains('v2board-dark-mode')).toBe(false);
  });

  it('enables the legacy bundled DarkReader on startup when the cookie is set', () => {
    let enabledWith: unknown;
    let disabled = false;
    document.cookie = 'dark_mode=1;path=/';
    window.webpackJsonp = [
      [
        [2],
        {
          nDCI: (
            _module: { exports: unknown },
            exports: Record<string, unknown>,
          ) => {
            exports.enable = (options: unknown) => {
              enabledWith = options;
            };
            exports.disable = () => {
              disabled = true;
            };
          },
        },
      ],
    ];

    applyInitialDarkMode();

    expect(enabledWith).toEqual({ brightness: 100, contrast: 90, sepia: 10 });
    expect(disabled).toBe(false);
  });
});
