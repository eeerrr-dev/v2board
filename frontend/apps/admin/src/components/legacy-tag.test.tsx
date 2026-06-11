import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacyTag } from './legacy-tag';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyTag', () => {
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

  it('keeps the plain old Ant Design tag shell', () => {
    const html = renderToStaticMarkup(<LegacyTag>基础</LegacyTag>);

    expect(html).toContain('<span class="ant-tag">基础</span>');
    expect(html).not.toContain('css-dev-only-do-not-override');
  });

  it('matches old preset and custom color classes', () => {
    const preset = renderToStaticMarkup(
      <LegacyTag className="extra" color="green" data-kind="status" id="tag-node" prefixCls="custom-tag">
        正常
      </LegacyTag>,
    );
    const custom = renderToStaticMarkup(<LegacyTag color="#415A94">节点</LegacyTag>);
    const nonPreset = renderToStaticMarkup(<LegacyTag color="green-inverse-extra">节点</LegacyTag>);

    expect(preset).toContain('class="custom-tag custom-tag-green extra"');
    expect(preset).toContain('data-kind="status"');
    expect(preset).toContain('id="tag-node"');
    expect(preset).not.toContain('background-color');
    expect(custom).toContain('class="ant-tag ant-tag-has-color"');
    expect(custom).toContain('style="background-color:#415A94"');
    expect(nonPreset).toContain('class="ant-tag ant-tag-has-color"');
    expect(nonPreset).toContain('style="background-color:green-inverse-extra"');
  });

  it('renders the old hidden class when controlled invisible', () => {
    const html = renderToStaticMarkup(<LegacyTag visible={false}>隐藏</LegacyTag>);

    expect(html).toContain('class="ant-tag ant-tag-hidden"');
  });

  it('treats explicit undefined visible as controlled hidden', () => {
    const html = renderToStaticMarkup(<LegacyTag visible={undefined}>隐藏</LegacyTag>);

    expect(html).toContain('class="ant-tag ant-tag-hidden"');
  });

  it('keeps the last controlled visible state when visible is removed', async () => {
    await act(async () => {
      root.render(<LegacyTag visible={false}>隐藏</LegacyTag>);
    });

    await act(async () => {
      root.render(<LegacyTag>隐藏</LegacyTag>);
    });

    expect(container.querySelector('.ant-tag')?.className).toContain('ant-tag-hidden');
  });

  it('handles the legacy closable tag close event', async () => {
    const onParentClick = vi.fn();
    const onClose = vi.fn();
    const afterClose = vi.fn();

    await act(async () => {
      root.render(
        <div onClick={onParentClick}>
          <LegacyTag afterClose={afterClose} closable onClose={onClose}>
            可关闭
          </LegacyTag>
        </div>,
      );
    });

    await act(async () => {
      container.querySelector<HTMLElement>('.anticon-close')!.click();
    });

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(afterClose).not.toHaveBeenCalled();
    expect(onParentClick).not.toHaveBeenCalled();
    expect(container.querySelector('.ant-tag')?.className).toContain('ant-tag-hidden');
  });

  it('calls afterClose for closable tags without onClose', async () => {
    const afterClose = vi.fn();

    await act(async () => {
      root.render(
        <LegacyTag afterClose={afterClose} closable>
          可关闭
        </LegacyTag>,
      );
    });

    await act(async () => {
      container.querySelector<HTMLElement>('.anticon-close')!.click();
    });

    expect(afterClose).toHaveBeenCalledTimes(1);
    expect(container.querySelector('.ant-tag')?.className).toContain('ant-tag-hidden');
  });

  it('keeps a closable tag visible when onClose prevents default', async () => {
    await act(async () => {
      root.render(
        <LegacyTag
          closable
          onClose={(event) => {
            event.preventDefault();
          }}
        >
          保留
        </LegacyTag>,
      );
    });

    await act(async () => {
      container.querySelector<HTMLElement>('.anticon-close')!.click();
    });

    expect(container.querySelector('.ant-tag')?.className).not.toContain('ant-tag-hidden');
  });

  it('runs the old wave animation for clickable tags', async () => {
    const onClick = vi.fn();
    await act(async () => {
      root.render(
        <LegacyTag color="blue" onClick={onClick}>
          复制
        </LegacyTag>,
      );
    });

    const tag = container.querySelector<HTMLElement>('.ant-tag')!;

    await act(async () => {
      tag.click();
    });

    expect(onClick).toHaveBeenCalledTimes(1);
    expect(tag.getAttribute('ant-click-animating-without-extra-node')).toBe('true');

    await act(async () => {
      tag.dispatchEvent(new AnimationEvent('animationend', { animationName: 'fadeEffect' }));
    });

    expect(tag.hasAttribute('ant-click-animating-without-extra-node')).toBe(false);
  });

  it('runs the old wave animation when the only child is an anchor', async () => {
    await act(async () => {
      root.render(
        <LegacyTag>
          <a href="#node">节点</a>
        </LegacyTag>,
      );
    });

    const tag = container.querySelector<HTMLElement>('.ant-tag')!;

    await act(async () => {
      container.querySelector<HTMLAnchorElement>('a')!.click();
    });

    expect(tag.getAttribute('ant-click-animating-without-extra-node')).toBe('true');
  });

  it('renders and toggles the old CheckableTag subcomponent', async () => {
    const onChange = vi.fn();

    await act(async () => {
      root.render(
        <LegacyTag.CheckableTag
          checked
          className="extra"
          data-kind="filter"
          onChange={onChange}
          prefixCls="custom-tag"
        >
          有效
        </LegacyTag.CheckableTag>,
      );
    });

    const tag = container.querySelector<HTMLElement>('.custom-tag')!;
    expect(tag.className).toBe('custom-tag custom-tag-checkable custom-tag-checkable-checked extra');
    expect(tag.getAttribute('data-kind')).toBe('filter');

    await act(async () => {
      tag.click();
    });

    expect(onChange).toHaveBeenCalledWith(false);

    await act(async () => {
      root.render(
        <LegacyTag.CheckableTag checked={false} onChange={onChange}>
          无效
        </LegacyTag.CheckableTag>,
      );
    });

    const unchecked = container.querySelector<HTMLElement>('.ant-tag')!;
    expect(unchecked.className).toBe('ant-tag ant-tag-checkable');

    await act(async () => {
      unchecked.click();
    });

    expect(onChange).toHaveBeenLastCalledWith(true);
  });
});
