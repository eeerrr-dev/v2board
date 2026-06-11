import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { LegacyButton } from './legacy-button';
import { LegacyPlusIcon } from './legacy-ant-icon';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyButton', () => {
  it('exposes the old antd static button marker', () => {
    expect((LegacyButton as typeof LegacyButton & { __ANT_BUTTON?: boolean }).__ANT_BUTTON).toBe(
      true,
    );
  });

  it('supports the old Button.Group static API', () => {
    const html = renderToStaticMarkup(
      <LegacyButton.Group
        className="orders-actions"
        data-testid="button-group"
        size="small"
        style={{ marginLeft: 8 }}
      >
        <LegacyButton type="primary">保存</LegacyButton>
        <LegacyButton>取消</LegacyButton>
      </LegacyButton.Group>,
    );

    expect(html).toContain('class="ant-btn-group ant-btn-group-sm orders-actions"');
    expect(html).toContain('data-testid="button-group"');
    expect(html).toContain('style="margin-left:8px"');
    expect(html).toContain('class="ant-btn ant-btn-primary"');
  });

  it('keeps Button.Group prefixCls behavior from the old ConfigConsumer component', () => {
    const html = renderToStaticMarkup(
      <LegacyButton.Group className="custom-actions" prefixCls="custom-btn-group" size="large">
        <LegacyButton>保存</LegacyButton>
      </LegacyButton.Group>,
    );

    expect(html).toContain('class="custom-btn-group custom-btn-group-lg custom-actions"');
  });

  it('renders the old Ant Design button shell without Ant Design 5 runtime classes', () => {
    const html = renderToStaticMarkup(
      <LegacyButton className="ant-btn">
        <LegacyPlusIcon />
      </LegacyButton>,
    );

    expect(html).toContain('<button type="button" class="ant-btn">');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).not.toContain('css-dev-only-do-not-override');
    expect(html).not.toContain('ant-btn-default');
    expect(html).not.toContain('ant-btn-color-default');
  });

  it('keeps ant-btn before dropdown trigger when rc-dropdown clones the child', () => {
    const html = renderToStaticMarkup(
      <LegacyButton className="ant-dropdown-trigger ant-btn">
        <LegacyPlusIcon />
      </LegacyButton>,
    );

    expect(html).toContain('<button type="button" class="ant-btn ant-dropdown-trigger">');
  });

  it('keeps the bundled auto space behavior for a single two-character Chinese label', () => {
    const html = renderToStaticMarkup(<LegacyButton className="ant-btn">提交</LegacyButton>);

    expect(html).toContain('<span>提 交</span>');
  });

  it('maps old Ant Design button props to generated classes and htmlType', () => {
    const html = renderToStaticMarkup(
      <LegacyButton
        block
        disabled
        ghost
        htmlType="submit"
        shape="round"
        size="large"
        type="primary"
      >
        提交
      </LegacyButton>,
    );

    expect(html).toContain('type="submit"');
    expect(html).toContain('disabled=""');
    expect(html).toContain(
      'class="ant-btn ant-btn-primary ant-btn-round ant-btn-lg ant-btn-background-ghost ant-btn-block"',
    );
    expect(html).toContain('<span>提 交</span>');
  });

  it('treats native button type values as html types for current rewrite call sites', () => {
    const html = renderToStaticMarkup(
      <LegacyButton className="ant-btn ant-btn-primary" type="button">
        确定
      </LegacyButton>,
    );

    expect(html).toContain('type="button"');
    expect(html).toContain('class="ant-btn ant-btn-primary"');
    expect(html).not.toContain('ant-btn-button');
  });

  it('renders old icon props before children and only auto-spaces iconless labels', () => {
    const iconOnlyHtml = renderToStaticMarkup(<LegacyButton icon="plus" />);
    const iconWithChildrenHtml = renderToStaticMarkup(
      <LegacyButton icon="plus">提交</LegacyButton>,
    );

    expect(iconOnlyHtml).toContain('class="ant-btn ant-btn-icon-only"');
    expect(iconOnlyHtml).toContain('aria-label="图标: plus"');
    expect(iconWithChildrenHtml).toContain('aria-label="图标: plus"');
    expect(iconWithChildrenHtml).toContain('<span>提交</span>');
    expect(iconWithChildrenHtml).not.toContain('提 交');
    expect(iconWithChildrenHtml).not.toContain('ant-btn-icon-only');
  });

  it('renders href buttons as anchors and keeps link buttons out of auto spacing', () => {
    const html = renderToStaticMarkup(
      <LegacyButton href="/docs" rel="noreferrer" target="_blank" type="link">
        文档
      </LegacyButton>,
    );

    expect(html).toContain(
      '<a href="/docs" rel="noreferrer" target="_blank" class="ant-btn ant-btn-link">',
    );
    expect(html).toContain('<span>文档</span>');
    expect(html).not.toContain('<button');
    expect(html).not.toContain('文 档');
  });

  it('keeps the old primary button attribute order with inline style', () => {
    const html = renderToStaticMarkup(
      <LegacyButton className="ant-btn ant-btn-primary" style={{ float: 'right' }}>
        编辑排序
      </LegacyButton>,
    );

    expect(html).toContain(
      '<button type="button" class="ant-btn ant-btn-primary" style="float:right"><span>编辑排序</span></button>',
    );
  });

  it('suppresses loading button clicks without preventing the original click event', async () => {
    const onClick = vi.fn();
    const container = document.createElement('div');
    document.body.appendChild(container);
    let root: Root | null = createRoot(container);

    await act(async () => {
      root!.render(
        <LegacyButton className="ant-btn ant-btn-primary ant-btn-loading" onClick={onClick}>
          提交
        </LegacyButton>,
      );
    });

    const button = container.querySelector<HTMLButtonElement>('.ant-btn')!;
    const event = new MouseEvent('click', { bubbles: true, cancelable: true });
    const notPrevented = button.dispatchEvent(event);

    expect(onClick).not.toHaveBeenCalled();
    expect(notPrevented).toBe(true);
    expect(event.defaultPrevented).toBe(false);

    await act(async () => {
      root?.unmount();
      root = null;
    });
    container.remove();
  });

  it('honors delayed loading objects before suppressing clicks', async () => {
    vi.useFakeTimers();
    const onClick = vi.fn();
    const container = document.createElement('div');
    document.body.appendChild(container);
    let root: Root | null = createRoot(container);

    await act(async () => {
      root!.render(
        <LegacyButton loading={false} onClick={onClick}>
          提交
        </LegacyButton>,
      );
    });

    const button = container.querySelector<HTMLButtonElement>('.ant-btn')!;
    button.click();
    expect(onClick).toHaveBeenCalledTimes(1);

    await act(async () => {
      root!.render(
        <LegacyButton loading={{ delay: 100 }} onClick={onClick}>
          提交
        </LegacyButton>,
      );
    });

    expect(button.className).not.toContain('ant-btn-loading');
    button.click();
    expect(onClick).toHaveBeenCalledTimes(2);

    await act(async () => {
      vi.advanceTimersByTime(100);
    });

    expect(button.className).toContain('ant-btn-loading');
    expect(button.outerHTML).toContain('aria-label="图标: loading"');
    button.click();
    expect(onClick).toHaveBeenCalledTimes(2);

    await act(async () => {
      root!.render(
        <LegacyButton loading={false} onClick={onClick}>
          提交
        </LegacyButton>,
      );
    });

    expect(button.className).not.toContain('ant-btn-loading');

    await act(async () => {
      root?.unmount();
      root = null;
    });
    container.remove();
    vi.useRealTimers();
  });
});
