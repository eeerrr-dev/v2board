import { getLegacyCookie, setLegacyCookie } from './legacy-cookie';

const DARK_MODE_KEY = 'dark_mode';
const DARK_MODE_CLASS = 'v2board-dark-mode';
const LEGACY_DARK_READER_MODULE = 'nDCI';
const LEGACY_DARK_READER_OPTIONS = {
  brightness: 100,
  contrast: 90,
  sepia: 10,
};

interface LegacyDarkReader {
  enable(options: typeof LEGACY_DARK_READER_OPTIONS): void;
  disable(): void;
}

type WebpackModuleFactory = (
  module: { exports: unknown },
  exports: Record<string, unknown>,
  require: (id: string) => unknown,
) => void;

declare global {
  interface Window {
    webpackJsonp?: unknown[];
  }
}

let darkReaderCache:
  | { factory: WebpackModuleFactory; reader: LegacyDarkReader | null }
  | null = null;

export function isDarkModeEnabled(): boolean {
  return getLegacyCookie(DARK_MODE_KEY) === '1';
}

export function applyDarkMode(enabled = isDarkModeEnabled()): void {
  const darkReader = getBundledDarkReader();
  if (darkReader) {
    document.documentElement.classList.remove(DARK_MODE_CLASS);
    if (enabled) {
      darkReader.enable(LEGACY_DARK_READER_OPTIONS);
    } else {
      darkReader.disable();
    }
    return;
  }

  document.documentElement.classList.toggle(DARK_MODE_CLASS, enabled);
}

export function setDarkMode(enabled: boolean): void {
  setLegacyCookie(DARK_MODE_KEY, enabled ? 1 : 0);
  applyDarkMode(enabled);
}

function getBundledDarkReader(): LegacyDarkReader | null {
  const factory = findLegacyDarkReaderFactory();
  if (!factory) return null;
  if (darkReaderCache?.factory === factory) return darkReaderCache.reader;

  const module = { exports: {} as unknown };
  const exports: Record<string, unknown> = {};
  factory(module, exports, () => ({}));

  const reader = isLegacyDarkReader(exports)
    ? {
        enable: exports.enable.bind(exports),
        disable: exports.disable.bind(exports),
      }
    : null;
  darkReaderCache = { factory, reader };
  return reader;
}

function findLegacyDarkReaderFactory(): WebpackModuleFactory | null {
  const chunks = window.webpackJsonp;
  if (!Array.isArray(chunks)) return null;

  for (const chunk of chunks) {
    if (!Array.isArray(chunk)) continue;
    const factories = chunk[1];
    if (!factories || typeof factories !== 'object') continue;
    const factory = (factories as Record<string, unknown>)[LEGACY_DARK_READER_MODULE];
    if (typeof factory === 'function') return factory as WebpackModuleFactory;
  }

  return null;
}

function isLegacyDarkReader(value: Record<string, unknown>): value is Record<string, unknown> & LegacyDarkReader {
  return typeof value.enable === 'function' && typeof value.disable === 'function';
}
