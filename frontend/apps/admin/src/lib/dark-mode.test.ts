import { beforeEach, describe, expect, it } from 'vitest';
import { applyInitialDarkMode, isDarkModeEnabled, setDarkMode } from './dark-mode';

describe('admin dark mode legacy behavior', () => {
  beforeEach(() => {
    document.cookie = 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    document.documentElement.className = '';
    window.localStorage.clear();
    window.webpackJsonp = undefined;
  });

  it('reads dark mode from the original cookie key', () => {
    document.cookie = 'dark_mode=1;path=/';
    window.localStorage.setItem('dark_mode', '0');

    expect(isDarkModeEnabled()).toBe(true);
  });

  it('writes the legacy cookie without using localStorage', () => {
    setDarkMode(true);

    expect(document.cookie).toContain('dark_mode=1');
    expect(window.localStorage.getItem('dark_mode')).toBeNull();
    expect(document.documentElement.classList.contains('v2board-dark-mode')).toBe(true);
  });

  it('uses the legacy bundled DarkReader module when available', () => {
    let enabledWith: unknown;
    let disabled = false;
    window.webpackJsonp = [
      [
        [2],
        {
          nDCI: (_module: { exports: unknown }, exports: Record<string, unknown>) => {
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

  it('matches legacy startup by only applying DarkReader when the cookie is set', () => {
    let enabled = false;
    let disabled = false;
    window.webpackJsonp = [
      [
        [2],
        {
          nDCI: (_module: { exports: unknown }, exports: Record<string, unknown>) => {
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

    document.cookie = 'dark_mode=1;path=/';
    applyInitialDarkMode();
    expect(enabled).toBe(true);
    expect(disabled).toBe(false);
  });
});
