import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacyTooltip } from './legacy-tooltip';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyTooltip antd behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
    vi.useRealTimers();
  });

  function renderTooltip(title: string, placement: 'top' | 'topRight' | 'left' = 'top') {
    act(() => {
      root.render(
        <LegacyTooltip title={title} placement={placement}>
          <span className="target">info</span>
        </LegacyTooltip>,
      );
    });
    const target = container.querySelector('.target') as HTMLElement;
    target.getBoundingClientRect = () =>
      ({
        top: 20,
        right: 70,
        bottom: 40,
        left: 30,
        width: 40,
        height: 20,
        x: 30,
        y: 20,
        toJSON: () => {},
      }) as DOMRect;
    return target;
  }

  it('portals the legacy tooltip shell to document.body', () => {
    const target = renderTooltip('Tip');

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(130);
    });

    const tooltip = document.body.querySelector('.ant-tooltip') as HTMLElement;
    expect(tooltip.parentElement).toBe(document.body);
    expect(tooltip.className).toContain('ant-tooltip-placement-top');
    expect(tooltip.querySelector('.ant-tooltip-arrow')).not.toBeNull();
    expect(tooltip.querySelector('.ant-tooltip-inner')?.getAttribute('role')).toBe('tooltip');
    expect(tooltip.querySelector('.ant-tooltip-inner')?.textContent).toBe('Tip');
  });

  it('uses the legacy zoom-big-fast motion classes', () => {
    const target = renderTooltip('Tip');

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(100);
    });

    act(() => {
      vi.advanceTimersByTime(30);
    });

    const tooltip = document.body.querySelector('.ant-tooltip') as HTMLElement;
    expect(tooltip.className).toContain('zoom-big-fast-enter');
    expect(tooltip.className).toContain('zoom-big-fast-enter-active');
    expect(tooltip.style.transformOrigin).toBe('50% calc(100% + 4px)');
    expect(tooltip.style.translate).toBe('-50% -100%');
    expect(target.className).toContain('ant-tooltip-open');

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(tooltip.className).not.toContain('zoom-big-fast-enter');
    expect(tooltip.className).not.toContain('zoom-big-fast-enter-active');

    act(() => {
      target.dispatchEvent(
        new MouseEvent('mouseout', { bubbles: true, relatedTarget: document.body }),
      );
      vi.advanceTimersByTime(100);
    });

    act(() => {
      vi.advanceTimersByTime(30);
    });

    expect(document.body.querySelector('.ant-tooltip')?.className).toContain(
      'zoom-big-fast-leave-active',
    );

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(document.body.querySelector('.ant-tooltip')).toBeNull();
  });

  it('does not show an overlay when title is empty', () => {
    const target = renderTooltip('');

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(200);
    });

    expect(document.body.querySelector('.ant-tooltip')).toBeNull();
    expect(target.className).not.toContain('ant-tooltip-open');
  });

  it('uses the legacy topRight transform origin at the arrow corner', () => {
    const target = renderTooltip('Tip', 'topRight');

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(130);
    });

    const tooltip = document.body.querySelector('.ant-tooltip') as HTMLElement;
    expect(tooltip.className).toContain('ant-tooltip-placement-topRight');
    expect(tooltip.style.transformOrigin).toBe('100% calc(100% + 4px)');
    expect(tooltip.style.translate).toBe('-100% -100%');
  });

  it('uses the legacy left transform origin at the arrow edge', () => {
    const target = renderTooltip('Tip', 'left');

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(130);
    });

    const tooltip = document.body.querySelector('.ant-tooltip') as HTMLElement;
    expect(tooltip.className).toContain('ant-tooltip-placement-left');
    expect(tooltip.style.top).toBe('30px');
    expect(tooltip.style.left).toBe('26px');
    expect(tooltip.style.transformOrigin).toBe('calc(100% + 4px) 50%');
    expect(tooltip.style.translate).toBe('-100% -50%');
  });
});
