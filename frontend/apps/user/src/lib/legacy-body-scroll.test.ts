import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { lockLegacyDrawerBodyScroll, lockLegacyModalBodyScroll } from './legacy-body-scroll';

describe('legacy drawer body scroll lock', () => {
  beforeEach(() => {
    document.body.className = 'existing-body-class';
    document.body.removeAttribute('style');

    Object.defineProperty(document.body, 'scrollHeight', {
      configurable: true,
      value: 1000,
    });
    Object.defineProperty(document.body, 'offsetWidth', {
      configurable: true,
      value: 1008,
    });
    Object.defineProperty(document.documentElement, 'clientHeight', {
      configurable: true,
      value: 500,
    });
    Object.defineProperty(document.documentElement, 'clientWidth', {
      configurable: true,
      value: 1008,
    });
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      value: 1024,
    });
  });

  afterEach(() => {
    document.body.className = '';
    document.body.removeAttribute('style');
  });

  it('matches rc-dialog body scrolling effect and restores previous state', () => {
    const unlock = lockLegacyModalBodyScroll();

    expect(document.body.className).toBe('existing-body-class ant-scrolling-effect');
    expect(document.body.style.position).toBe('relative');
    expect(document.body.style.width).toBe('calc(100% - 16px)');
    expect(document.body.style.overflow).toBe('hidden');
    expect(document.body.style.overflowX).toBe('hidden');
    expect(document.body.style.overflowY).toBe('hidden');

    unlock();

    expect(document.body.className).toBe('existing-body-class');
    expect(document.body.getAttribute('style') ?? '').toBe('');
  });

  it('adds the rc-drawer touch-action lock on top of the shared scrolling effect', () => {
    const unlock = lockLegacyDrawerBodyScroll();

    expect(document.body.className).toBe('existing-body-class ant-scrolling-effect');
    expect(document.body.style.touchAction).toBe('none');

    unlock();

    expect(document.body.className).toBe('existing-body-class');
    expect(document.body.getAttribute('style') ?? '').toBe('');
  });
});
