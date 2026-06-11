import { act, createRef } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacySwitch, type LegacySwitchRef } from './legacy-switch';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacySwitch', () => {
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
  });

  it('exposes the old antd static switch marker', () => {
    expect((LegacySwitch as typeof LegacySwitch & { __ANT_SWITCH?: boolean }).__ANT_SWITCH).toBe(
      true,
    );
  });

  it('renders the old Ant Design switch classes without v5 hashes', () => {
    const checked = renderToStaticMarkup(<LegacySwitch checked={1} size="small" />);

    expect(checked).toContain(
      '<button type="button" role="switch" aria-checked="true" class="ant-switch-small ant-switch ant-switch-checked">',
    );
    expect(checked).toContain('<span class="ant-switch-inner"></span>');
    expect(checked).not.toContain('css-dev-only-do-not-override');

    const unchecked = renderToStaticMarkup(<LegacySwitch checked={0} />);
    expect(unchecked).toContain(
      '<button type="button" role="switch" aria-checked="false" class="ant-switch">',
    );
    expect(unchecked).not.toContain('ant-switch-small');
    expect(unchecked).not.toContain('ant-switch-checked');
  });

  it('keeps the legacy disabled switch class and native disabled attribute', () => {
    const html = renderToStaticMarkup(<LegacySwitch checked={1} disabled />);

    expect(html).toContain(
      '<button type="button" role="switch" aria-checked="true" disabled="" class="ant-switch ant-switch-checked ant-switch-disabled">',
    );
  });

  it('renders the original checked and unchecked inner labels', () => {
    const checked = renderToStaticMarkup(
      <LegacySwitch checked={1} checkedChildren="亮" unCheckedChildren="暗" />,
    );
    const unchecked = renderToStaticMarkup(
      <LegacySwitch checked={0} checkedChildren="亮" unCheckedChildren="暗" />,
    );

    expect(checked).toContain('<span class="ant-switch-inner">亮</span>');
    expect(unchecked).toContain('<span class="ant-switch-inner">暗</span>');
  });

  it('keeps old className ordering and passes button props through', () => {
    const html = renderToStaticMarkup(
      <LegacySwitch
        checked={1}
        className="theme-switch"
        id="theme-mode"
        name="theme"
        tabIndex={2}
      />,
    );

    expect(html).toContain('id="theme-mode"');
    expect(html).toContain('tabindex="2"');
    expect(html).toContain('name="theme"');
    expect(html).toContain('class="theme-switch ant-switch ant-switch-checked"');
    expect(html).not.toContain(' checked=""');
  });

  it('renders the old loading switch classes, icon, and disabled state', () => {
    const html = renderToStaticMarkup(<LegacySwitch checked={1} loading />);

    expect(html).toContain(
      '<button type="button" role="switch" aria-checked="true" disabled="" class="ant-switch-loading ant-switch ant-switch-checked ant-switch-disabled">',
    );
    expect(html).toContain('ant-switch-loading-icon');
    expect(html).toContain('anticon-loading');
    expect(html.indexOf('ant-switch-loading-icon')).toBeLessThan(
      html.indexOf('ant-switch-inner'),
    );
  });

  it('calls the legacy click callback with the next checked value', async () => {
    const onChange = vi.fn();
    const onClick = vi.fn();

    await act(async () => {
      root.render(<LegacySwitch checked={0} onChange={onChange} onClick={onClick} />);
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('.ant-switch')!.click();
    });

    expect(onChange.mock.calls[0]?.[0]).toBe(true);
    expect(onClick.mock.calls[0]?.[0]).toBe(true);
  });

  it('supports the old defaultChecked uncontrolled state and checked prop presence', async () => {
    const onChange = vi.fn();

    await act(async () => {
      root.render(<LegacySwitch defaultChecked onChange={onChange} />);
    });

    const button = container.querySelector<HTMLButtonElement>('.ant-switch')!;
    expect(button.className).toBe('ant-switch ant-switch-checked');

    await act(async () => {
      button.click();
    });
    expect(onChange.mock.calls.at(-1)?.[0]).toBe(false);
    expect(button.className).toBe('ant-switch');

    await act(async () => {
      root.render(<LegacySwitch checked={undefined} defaultChecked onChange={onChange} />);
    });
    expect(button.className).toBe('ant-switch');
  });

  it('exposes the old focus and blur methods and blurs on mouseup', async () => {
    const switchRef = createRef<LegacySwitchRef>();
    const onMouseUp = vi.fn();

    await act(async () => {
      root.render(<LegacySwitch ref={switchRef} autoFocus checked={0} onMouseUp={onMouseUp} />);
    });

    const button = container.querySelector<HTMLButtonElement>('.ant-switch')!;
    expect(document.activeElement).toBe(button);

    await act(async () => {
      switchRef.current?.blur();
    });
    expect(document.activeElement).not.toBe(button);

    await act(async () => {
      switchRef.current?.focus();
    });
    expect(document.activeElement).toBe(button);

    await act(async () => {
      button.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
    });
    expect(onMouseUp).toHaveBeenCalledTimes(1);
    expect(document.activeElement).not.toBe(button);

    await act(async () => {
      switchRef.current?.focus();
      switchRef.current?.blur();
    });
    expect(document.activeElement).not.toBe(button);
  });

  it('handles legacy left and right arrow keyboard changes', async () => {
    const onChange = vi.fn();

    await act(async () => {
      root.render(<LegacySwitch checked={0} onChange={onChange} />);
    });

    const press = async (key: string, keyCode: number) => {
      const event = new KeyboardEvent('keydown', { bubbles: true, key });
      Object.defineProperty(event, 'keyCode', { value: keyCode });
      await act(async () => {
        container.querySelector<HTMLButtonElement>('.ant-switch')!.dispatchEvent(event);
      });
    };

    await press('ArrowRight', 39);
    expect(onChange.mock.calls.at(-1)?.[0]).toBe(true);

    await act(async () => {
      root.render(<LegacySwitch checked={1} onChange={onChange} />);
    });
    await press('ArrowLeft', 37);
    expect(onChange.mock.calls.at(-1)?.[0]).toBe(false);

    await act(async () => {
      root.render(<LegacySwitch checked={1} disabled onChange={onChange} />);
    });
    const callCount = onChange.mock.calls.length;
    await press('ArrowLeft', 37);
    await act(async () => {
      container.querySelector<HTMLButtonElement>('.ant-switch')!.click();
    });
    expect(onChange).toHaveBeenCalledTimes(callCount);
  });
});
