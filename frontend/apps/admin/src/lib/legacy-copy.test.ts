import { afterEach, describe, expect, it, vi } from 'vitest';
import { legacyCopyText } from './legacy-copy';

describe('legacyCopyText', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = '';
  });

  it('uses the old hidden span selection before any fallback path', () => {
    const execCommand = vi.fn(() => {
      const mark = document.body.querySelector('span') as HTMLSpanElement | null;
      expect(mark?.textContent).toBe('legacy text');
      expect(mark?.ariaHidden).toBe('true');
      expect(mark?.style.position).toBe('fixed');
      expect(mark?.style.clip).toBe('rect(0, 0, 0, 0)');
      expect(mark?.style.whiteSpace).toBe('pre');
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
    const clipboardWrite = vi.fn();
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: clipboardWrite },
    });

    expect(legacyCopyText('legacy text')).toBe(true);

    expect(execCommand).toHaveBeenCalledWith('copy');
    expect(clipboardWrite).not.toHaveBeenCalled();
    expect(document.querySelector('span')).toBeNull();
  });

  it('falls back to the old clipboardData path when execCommand cannot copy', () => {
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
