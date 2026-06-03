import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Dialog, DialogContent } from './dialog';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ i18n: { language: 'en-US' } }),
}));

describe('Dialog legacy modal scroll lock', () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    document.body.className = 'app-body';
    document.body.removeAttribute('style');
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

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
    if (root) act(() => root?.unmount());
    container.remove();
    document.body.className = '';
    document.body.removeAttribute('style');
  });

  it('uses rc-dialog scrolling effect instead of ant-modal-open', () => {
    act(() => {
      root!.render(
        <Dialog open onOpenChange={() => {}}>
          <DialogContent footer={null}>content</DialogContent>
        </Dialog>,
      );
    });

    expect(document.body.className).toBe('app-body ant-scrolling-effect');
    expect(document.body.classList.contains('ant-modal-open')).toBe(false);
    expect(document.body.style.width).toBe('calc(100% - 16px)');
    expect(document.body.style.overflow).toBe('hidden');

    act(() => root?.unmount());
    root = null;

    expect(document.body.className).toBe('app-body');
    expect(document.body.getAttribute('style') ?? '').toBe('');
  });
});
