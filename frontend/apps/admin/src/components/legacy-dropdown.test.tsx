import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  LEGACY_DROPDOWN_CLICK_TRIGGER,
  LegacyDropdown,
  LegacyDropdownMenu,
  LegacyDropdownMenuItem,
} from './legacy-dropdown';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyDropdown', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
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

  const overlay = (
    <LegacyDropdownMenu>
      <LegacyDropdownMenuItem>导出CSV</LegacyDropdownMenuItem>
    </LegacyDropdownMenu>
  );

  function mockTriggerRect(element: HTMLElement) {
    element.getBoundingClientRect = () =>
      ({
        bottom: 32,
        height: 24,
        left: 12,
        right: 92,
        top: 8,
        width: 80,
        x: 12,
        y: 8,
        toJSON: () => undefined,
      }) as DOMRect;
  }

  it('keeps the old default hover trigger from opening on click', async () => {
    vi.useFakeTimers();

    await act(async () => {
      root.render(
        <LegacyDropdown overlay={overlay}>
          <button type="button">操作</button>
        </LegacyDropdown>,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('button')!;
    mockTriggerRect(trigger);
    expect(trigger.className).toBe('ant-dropdown-trigger');

    await act(async () => {
      trigger.click();
    });
    expect(document.body.querySelector('.ant-dropdown')).toBeNull();

    await act(async () => {
      trigger.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(149);
    });
    expect(document.body.querySelector('.ant-dropdown')).toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(1);
      await Promise.resolve();
    });
    expect(document.body.querySelector('.ant-dropdown')?.className).toBe(
      'ant-dropdown  ant-dropdown-placement-bottomLeft',
    );

    vi.useRealTimers();
  });

  it('opens and hides from click only when the legacy click trigger is requested', async () => {
    const onVisibleChange = vi.fn();
    await act(async () => {
      root.render(
        <LegacyDropdown
          trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}
          overlay={overlay}
          onVisibleChange={onVisibleChange}
        >
          <a href="/orders">操作</a>
        </LegacyDropdown>,
      );
    });

    const trigger = container.querySelector<HTMLAnchorElement>('a')!;
    mockTriggerRect(trigger);

    const openEvent = new MouseEvent('click', { bubbles: true, cancelable: true });
    await act(async () => {
      trigger.dispatchEvent(openEvent);
    });
    expect(openEvent.defaultPrevented).toBe(true);
    expect(onVisibleChange).toHaveBeenLastCalledWith(true);
    expect(trigger.className).toContain('ant-dropdown-open');
    expect(document.body.querySelector('.ant-dropdown')?.className).toBe(
      'ant-dropdown  ant-dropdown-placement-bottomLeft',
    );

    const closeEvent = new MouseEvent('click', { bubbles: true, cancelable: true });
    await act(async () => {
      trigger.dispatchEvent(closeEvent);
    });
    expect(closeEvent.defaultPrevented).toBe(true);
    expect(onVisibleChange).toHaveBeenLastCalledWith(false);
    expect(document.body.querySelector('.ant-dropdown')?.className).toContain('ant-dropdown-hidden');
  });

  it('matches old overlay props, function overlays, popup container, and click overlay hiding', async () => {
    const popupHost = document.createElement('div');
    document.body.appendChild(popupHost);
    const onOverlayClick = vi.fn();

    await act(async () => {
      root.render(
        <LegacyDropdown
          trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}
          overlay={() => overlay}
          overlayClassName="extra-menu"
          overlayStyle={{ backgroundColor: 'rgb(1, 2, 3)' }}
          getPopupContainer={() => popupHost}
          onOverlayClick={onOverlayClick}
        >
          <button type="button">操作</button>
        </LegacyDropdown>,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('button')!;
    mockTriggerRect(trigger);

    await act(async () => {
      trigger.click();
    });

    const dropdown = popupHost.querySelector<HTMLElement>('.ant-dropdown')!;
    expect(dropdown.parentElement).toBe(popupHost);
    expect(dropdown.className).toContain('extra-menu');
    expect(dropdown.style.backgroundColor).toBe('rgb(1, 2, 3)');

    await act(async () => {
      dropdown.querySelector<HTMLElement>('.ant-dropdown-menu-item')?.click();
    });

    expect(onOverlayClick).toHaveBeenCalledTimes(1);
    expect(dropdown.className).toContain('ant-dropdown-hidden');

    await act(async () => {
      document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
      await Promise.resolve();
    });

    expect(dropdown.className).toContain('ant-dropdown-hidden');
  });

  it('keeps default hover dropdown overlays visible after menu item clicks', async () => {
    vi.useFakeTimers();
    const onOverlayClick = vi.fn();

    await act(async () => {
      root.render(
        <LegacyDropdown overlay={overlay} onOverlayClick={onOverlayClick}>
          <button type="button">操作</button>
        </LegacyDropdown>,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('button')!;
    mockTriggerRect(trigger);

    await act(async () => {
      trigger.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(150);
      await Promise.resolve();
    });

    const dropdown = document.body.querySelector<HTMLElement>('.ant-dropdown')!;
    expect(dropdown.className).not.toContain('ant-dropdown-hidden');

    await act(async () => {
      dropdown.querySelector<HTMLElement>('.ant-dropdown-menu-item')?.click();
    });

    expect(onOverlayClick).toHaveBeenCalledTimes(1);
    expect(dropdown.className).not.toContain('ant-dropdown-hidden');

    await act(async () => {
      dropdown.dispatchEvent(
        new MouseEvent('mouseout', { bubbles: true, relatedTarget: document.body }),
      );
      vi.advanceTimersByTime(100);
    });
    expect(dropdown.className).not.toContain('ant-dropdown-hidden');

    const modal = document.createElement('div');
    modal.className = 'ant-modal';
    const confirmButtons = document.createElement('div');
    confirmButtons.className = 'ant-modal-confirm-btns';
    modal.appendChild(confirmButtons);
    document.body.appendChild(modal);

    await act(async () => {
      modal.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
      await Promise.resolve();
    });
    expect(dropdown.className).not.toContain('ant-dropdown-hidden');

    await act(async () => {
      confirmButtons.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(dropdown.className).toContain('ant-dropdown-hidden');
    vi.useRealTimers();
  });

  it('keeps the old trigger class on disabled dropdown children without opening', async () => {
    await act(async () => {
      root.render(
        <LegacyDropdown disabled trigger={LEGACY_DROPDOWN_CLICK_TRIGGER} overlay={overlay}>
          <button type="button">操作</button>
        </LegacyDropdown>,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('button')!;
    mockTriggerRect(trigger);
    expect(trigger.className).toBe('ant-dropdown-trigger');

    await act(async () => {
      trigger.click();
      trigger.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });

    expect(trigger.className).not.toContain('ant-dropdown-open');
    expect(document.body.querySelector('.ant-dropdown')).toBeNull();
  });

  it('uses the old 150ms hover enter and 100ms hover leave delays', async () => {
    vi.useFakeTimers();

    await act(async () => {
      root.render(
        <LegacyDropdown overlay={overlay}>
          <button type="button">操作</button>
        </LegacyDropdown>,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('button')!;
    mockTriggerRect(trigger);

    await act(async () => {
      trigger.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(149);
    });
    expect(document.body.querySelector('.ant-dropdown')).toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(1);
      await Promise.resolve();
    });
    expect(document.body.querySelector('.ant-dropdown')?.className).not.toContain(
      'ant-dropdown-hidden',
    );

    await act(async () => {
      trigger.dispatchEvent(
        new MouseEvent('mouseout', { bubbles: true, relatedTarget: document.body }),
      );
      vi.advanceTimersByTime(99);
    });
    expect(document.body.querySelector('.ant-dropdown')?.className).not.toContain(
      'ant-dropdown-hidden',
    );

    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(document.body.querySelector('.ant-dropdown')?.className).toContain(
      'ant-dropdown-hidden',
    );

    vi.useRealTimers();
  });

  it('renders old defaultVisible dropdowns on mount with the open trigger class', async () => {
    await act(async () => {
      root.render(
        <LegacyDropdown defaultVisible overlay={overlay}>
          <button type="button">操作</button>
        </LegacyDropdown>,
      );
      await Promise.resolve();
    });

    const trigger = container.querySelector<HTMLButtonElement>('button')!;
    mockTriggerRect(trigger);

    await act(async () => {
      await Promise.resolve();
    });

    expect(trigger.className).toContain('ant-dropdown-open');
    expect(document.body.querySelector('.ant-dropdown')?.className).toBe(
      'ant-dropdown  ant-dropdown-placement-bottomLeft',
    );
  });

  it('supports old contextMenu triggers with point alignment and click hiding', async () => {
    const onVisibleChange = vi.fn();

    await act(async () => {
      root.render(
        <LegacyDropdown trigger="contextMenu" overlay={overlay} onVisibleChange={onVisibleChange}>
          <button type="button">操作</button>
        </LegacyDropdown>,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('button')!;
    mockTriggerRect(trigger);

    const contextMenuEvent = new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: 120,
      clientY: 56,
    });
    await act(async () => {
      trigger.dispatchEvent(contextMenuEvent);
      await Promise.resolve();
    });

    const dropdown = document.body.querySelector<HTMLElement>('.ant-dropdown')!;
    expect(contextMenuEvent.defaultPrevented).toBe(true);
    expect(onVisibleChange).toHaveBeenLastCalledWith(true);
    expect(dropdown.className).toBe('ant-dropdown  ant-dropdown-placement-bottomLeft');
    expect(dropdown.style.position).toBe('absolute');
    expect(dropdown.style.left).toBe('120px');
    expect(dropdown.style.top).toBe('60px');

    await act(async () => {
      document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onVisibleChange).toHaveBeenLastCalledWith(false);
    expect(dropdown.className).toContain('ant-dropdown-hidden');
  });

  it('keeps old placement classes and passes disabled to the trigger child', async () => {
    await act(async () => {
      root.render(
        <LegacyDropdown
          disabled
          placement="bottomRight"
          trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}
          overlay={overlay}
        >
          <button type="button">操作</button>
        </LegacyDropdown>,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('button')!;
    expect(trigger.disabled).toBe(true);
    expect(trigger.className).toBe('ant-dropdown-trigger');

    await act(async () => {
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(document.body.querySelector('.ant-dropdown')).toBeNull();
  });
});
