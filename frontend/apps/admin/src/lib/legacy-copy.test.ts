import { afterEach, describe, expect, it, vi } from 'vitest';
import { legacyCopyText } from './legacy-copy';

describe('legacyCopyText', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('uses the old execCommand copy path before the Clipboard API', () => {
    const execCommand = vi.fn(() => true);
    const clipboardWrite = vi.fn();
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: clipboardWrite },
    });

    legacyCopyText('legacy text');

    expect(execCommand).toHaveBeenCalledWith('copy');
    expect(clipboardWrite).not.toHaveBeenCalled();
    expect(document.querySelector('textarea')).toBeNull();
  });

  it('falls back to Clipboard API when execCommand cannot copy', () => {
    const execCommand = vi.fn(() => false);
    const clipboardWrite = vi.fn();
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: clipboardWrite },
    });

    legacyCopyText('fallback text');

    expect(execCommand).toHaveBeenCalledWith('copy');
    expect(clipboardWrite).toHaveBeenCalledWith('fallback text');
    expect(document.querySelector('textarea')).toBeNull();
  });
});
