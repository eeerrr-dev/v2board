import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacySwitch } from './legacy-switch';
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

  function renderTooltip(title: string, placement: 'top' | 'topRight' | 'left' | 'right' = 'top') {
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
      vi.advanceTimersByTime(100);
    });

    const tooltip = document.body.querySelector('.ant-tooltip') as HTMLElement;
    expect(tooltip.parentElement).toBe(document.body);
    expect(tooltip.className).toContain('ant-tooltip-placement-top');
    expect(tooltip.querySelector('.ant-tooltip-arrow')).not.toBeNull();
    expect(tooltip.querySelector('.ant-tooltip-inner')?.getAttribute('role')).toBe('tooltip');
    expect(tooltip.querySelector('.ant-tooltip-inner')?.textContent).toBe('Tip');
  });

  it('opens after the old default hover delay and uses the legacy zoom-big-fast motion classes', () => {
    const target = renderTooltip('Tip');

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });

    expect(document.body.querySelector('.ant-tooltip')).toBeNull();

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(document.body.querySelector('.ant-tooltip')?.className).toContain(
      'zoom-big-fast-enter',
    );

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

    const hiddenTooltip = document.body.querySelector('.ant-tooltip') as HTMLElement;
    expect(hiddenTooltip).not.toBeNull();
    expect(hiddenTooltip.className).toContain('ant-tooltip-hidden');
    expect(target.className).not.toContain('ant-tooltip-open');
  });

  it('removes the popup after close when destroyTooltipOnHide is enabled', () => {
    act(() => {
      root.render(
        <LegacyTooltip destroyTooltipOnHide title="Tip">
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

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(100);
    });
    expect(document.body.querySelector('.ant-tooltip')).not.toBeNull();

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseout', { bubbles: true }));
      vi.advanceTimersByTime(100);
    });
    act(() => {
      vi.advanceTimersByTime(130);
    });

    expect(document.body.querySelector('.ant-tooltip')).toBeNull();
  });

  it('honors a zero mouseEnterDelay like old Tooltip props', () => {
    act(() => {
      root.render(
        <LegacyTooltip mouseEnterDelay={0} title="Tip">
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

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });

    expect(document.body.querySelector('.ant-tooltip-inner')?.textContent).toBe('Tip');
  });

  it('keeps open when the pointer moves from trigger into the tooltip popup', () => {
    const target = renderTooltip('Tip');

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(100);
      vi.advanceTimersByTime(130);
    });

    const tooltip = document.body.querySelector('.ant-tooltip') as HTMLElement;
    expect(tooltip).not.toBeNull();

    act(() => {
      target.dispatchEvent(
        new MouseEvent('mouseout', { bubbles: true, relatedTarget: tooltip }),
      );
      vi.advanceTimersByTime(50);
      tooltip.dispatchEvent(
        new MouseEvent('mouseover', { bubbles: true, relatedTarget: target }),
      );
      vi.advanceTimersByTime(150);
    });

    expect(document.body.querySelector('.ant-tooltip-inner')?.textContent).toBe('Tip');
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

  it('does not notify visibility changes when title and overlay are empty', () => {
    const onVisibleChange = vi.fn();
    act(() => {
      root.render(
        <LegacyTooltip title="" onVisibleChange={onVisibleChange}>
          <span className="target">info</span>
        </LegacyTooltip>,
      );
    });
    const target = container.querySelector('.target') as HTMLElement;

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(100);
    });

    expect(onVisibleChange).not.toHaveBeenCalled();
    expect(document.body.querySelector('.ant-tooltip')).toBeNull();
  });

  it('supports overlay functions and the rc-tooltip popup customization props', () => {
    const popupRoot = document.createElement('div');
    document.body.appendChild(popupRoot);
    act(() => {
      root.render(
        <LegacyTooltip
          getTooltipContainer={() => popupRoot}
          mouseEnterDelay={0}
          openClassName="is-tooltip-open"
          overlay={() => <strong>Overlay tip</strong>}
          overlayClassName="custom-tooltip"
          overlayStyle={{ maxWidth: 120 }}
        >
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

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });

    const tooltip = popupRoot.querySelector('.ant-tooltip') as HTMLElement;
    expect(tooltip.parentElement).toBe(popupRoot);
    expect(tooltip.className).toContain('custom-tooltip');
    expect(tooltip.style.maxWidth).toBe('120px');
    expect(tooltip.querySelector('.ant-tooltip-inner')?.textContent).toBe('Overlay tip');
    expect(target.className).toContain('is-tooltip-open');
  });

  it('opens from defaultVisible like old antd Tooltip state', () => {
    act(() => {
      root.render(
        <LegacyTooltip defaultVisible title="Initial tip">
          <span className="target">info</span>
        </LegacyTooltip>,
      );
    });

    const target = container.querySelector('.target') as HTMLElement;
    expect(target.className).toContain('ant-tooltip-open');
    expect(document.body.querySelector('.ant-tooltip-inner')?.textContent).toBe('Initial tip');
  });

  it('supports the old click trigger and onVisibleChange callback', () => {
    const onVisibleChange = vi.fn();
    act(() => {
      root.render(
        <LegacyTooltip
          mouseEnterDelay={0}
          onVisibleChange={onVisibleChange}
          title="Click tip"
          trigger={['click']}
        >
          <button className="target" type="button">
            info
          </button>
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

    act(() => {
      target.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    });

    expect(onVisibleChange).toHaveBeenLastCalledWith(true);
    expect(document.body.querySelector('.ant-tooltip-inner')?.textContent).toBe('Click tip');

    act(() => {
      target.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    });

    expect(onVisibleChange).toHaveBeenLastCalledWith(false);
  });

  it('wraps primitive children the way antd Tooltip allows', () => {
    act(() => {
      root.render(<LegacyTooltip title="Tip">plain text</LegacyTooltip>);
    });

    const target = container.querySelector('span') as HTMLElement;
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

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(100);
    });

    expect(target.textContent).toBe('plain text');
    expect(target.className).toContain('ant-tooltip-open');
    expect(document.body.querySelector('.ant-tooltip-inner')?.textContent).toBe('Tip');
  });

  it('preserves the wrapped child hover handlers like rc-trigger', () => {
    const onMouseEnter = vi.fn();
    const onMouseLeave = vi.fn();
    act(() => {
      root.render(
        <LegacyTooltip title="Tip">
          <span className="target" onMouseEnter={onMouseEnter} onMouseLeave={onMouseLeave}>
            info
          </span>
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

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(100);
    });
    act(() => {
      target.dispatchEvent(new MouseEvent('mouseout', { bubbles: true }));
    });

    expect(onMouseEnter).toHaveBeenCalledTimes(1);
    expect(onMouseLeave).toHaveBeenCalledTimes(1);
    expect(document.body.querySelector('.ant-tooltip-inner')?.textContent).toBe('Tip');
  });

  it('wraps disabled native controls so the tooltip can still trigger', () => {
    act(() => {
      root.render(
        <LegacyTooltip mouseEnterDelay={0} title="Disabled tip">
          <button className="target" disabled type="button">
            info
          </button>
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

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });

    expect(target.tagName).toBe('SPAN');
    expect(target.style.cursor).toBe('not-allowed');
    expect(target.querySelector('button')?.style.pointerEvents).toBe('none');
    expect(document.body.querySelector('.ant-tooltip-inner')?.textContent).toBe('Disabled tip');
  });

  it('wraps disabled old antd-marked controls so the tooltip can still trigger', () => {
    act(() => {
      root.render(
        <LegacyTooltip mouseEnterDelay={0} title="Disabled switch">
          <LegacySwitch className="switch-target" disabled />
        </LegacyTooltip>,
      );
    });
    const target = container.querySelector('.switch-target') as HTMLElement;
    target.getBoundingClientRect = () =>
      ({
        top: 20,
        right: 80,
        bottom: 42,
        left: 30,
        width: 50,
        height: 22,
        x: 30,
        y: 20,
        toJSON: () => {},
      }) as DOMRect;

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });

    const switchButton = target.querySelector('.ant-switch') as HTMLButtonElement;
    expect(target.tagName).toBe('SPAN');
    expect(target.style.cursor).toBe('not-allowed');
    expect(switchButton.disabled).toBe(true);
    expect(switchButton.className).toContain('ant-switch-disabled');
    expect(switchButton.className).not.toContain('switch-target');
    expect(switchButton.style.pointerEvents).toBe('none');
    expect(document.body.querySelector('.ant-tooltip-inner')?.textContent).toBe(
      'Disabled switch',
    );
  });

  it('uses the legacy topRight transform origin at the arrow corner', () => {
    const target = renderTooltip('Tip', 'topRight');

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(100);
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
      vi.advanceTimersByTime(100);
    });

    const tooltip = document.body.querySelector('.ant-tooltip') as HTMLElement;
    expect(tooltip.className).toContain('ant-tooltip-placement-left');
    expect(tooltip.style.top).toBe('30px');
    expect(tooltip.style.left).toBe('26px');
    expect(tooltip.style.transformOrigin).toBe('calc(100% + 4px) 50%');
    expect(tooltip.style.translate).toBe('-100% -50%');
  });

  it('uses the legacy right transform origin at the arrow edge', () => {
    const target = renderTooltip('Tip', 'right');

    act(() => {
      target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(100);
    });

    const tooltip = document.body.querySelector('.ant-tooltip') as HTMLElement;
    expect(tooltip.className).toContain('ant-tooltip-placement-right');
    expect(tooltip.style.top).toBe('30px');
    expect(tooltip.style.left).toBe('74px');
    expect(tooltip.style.transformOrigin).toBe('-4px 50%');
    expect(tooltip.style.translate).toBe('0 -50%');
  });
});
