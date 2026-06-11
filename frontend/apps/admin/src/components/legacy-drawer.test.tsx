import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacyDrawer } from './legacy-drawer';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyDrawer', () => {
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
    document.body.removeAttribute('style');
  });

  it('renders the old Ant Design drawer shell in a body portal', async () => {
    await act(async () => {
      root.render(
        <LegacyDrawer id="knowledge" open title="新增知识" width="80%" onClose={vi.fn()}>
          <div>表单</div>
        </LegacyDrawer>,
      );
    });

    const drawer = document.querySelector('#knowledge')!;
    expect(drawer.outerHTML).toContain(
      '<div id="knowledge" tabindex="-1" class="ant-drawer ant-drawer-right ant-drawer-open"><div class="ant-drawer-mask"></div><div class="ant-drawer-content-wrapper" style="width: 80%;">',
    );
    expect(drawer.querySelector('.ant-drawer-header')?.outerHTML).toContain(
      '<div class="ant-drawer-title">新增知识</div><button aria-label="Close" class="ant-drawer-close"><i aria-label="图标: close" class="anticon anticon-close">',
    );
    expect(drawer.querySelector('.ant-drawer-body')?.outerHTML).toBe(
      '<div class="ant-drawer-body"><div>表单</div></div>',
    );
    expect(document.body.getAttribute('style')).toBe('overflow: hidden; touch-action: none;');
    expect(container.children).toHaveLength(0);
  });

  it('can preserve legacy Drawer passthrough attributes used by the user editor', async () => {
    await act(async () => {
      root.render(
        <LegacyDrawer
          id="user"
          cancelText="取消"
          open
          title="用户管理"
          width="80%"
          onClose={vi.fn()}
        >
          <div>表单</div>
        </LegacyDrawer>,
      );
    });

    expect(document.querySelector('#user')?.outerHTML).toContain(
      '<div id="user" canceltext="取消" tabindex="-1" class="ant-drawer ant-drawer-right ant-drawer-open">',
    );
  });

  it('can preserve the old filter drawer root attributes and default width', async () => {
    await act(async () => {
      root.render(
        <LegacyDrawer
          className="v2board-filter-drawer"
          footer={<></>}
          open
          title="过滤器"
          width={256}
          onClose={vi.fn()}
        >
          <div>表单</div>
        </LegacyDrawer>,
      );
    });

    expect(document.querySelector('.ant-drawer')?.outerHTML).toContain(
      '<div footer="[object Object]" tabindex="-1" class="ant-drawer ant-drawer-right ant-drawer-open v2board-filter-drawer">',
    );
    expect(document.querySelector('.ant-drawer-content-wrapper')?.getAttribute('style')).toBe(
      'width: 256px;',
    );
  });

  it('can hide the close button for legacy child drawers', async () => {
    await act(async () => {
      root.render(
        <LegacyDrawer
          closable={false}
          id="server"
          open
          title="编辑安全性配置"
          width="80%"
          onClose={vi.fn()}
        >
          <div>子抽屉</div>
        </LegacyDrawer>,
      );
    });

    expect(document.querySelector('#server .ant-drawer-title')?.textContent).toBe('编辑安全性配置');
    expect(document.querySelector('#server .ant-drawer-close')).toBeNull();
  });

  it('supports old visible alias, placement, height and style props', async () => {
    const afterVisibleChange = vi.fn();
    await act(async () => {
      root.render(
        <LegacyDrawer
          bodyStyle={{ padding: 0 }}
          drawerStyle={{ backgroundColor: 'rgb(250, 250, 250)' }}
          headerStyle={{ borderBottom: 0 }}
          height={320}
          maskStyle={{ opacity: 0.25 }}
          placement="top"
          style={{ position: 'absolute' }}
          title="顶部抽屉"
          visible
          zIndex={1200}
          afterVisibleChange={afterVisibleChange}
          onClose={vi.fn()}
        >
          <div>内容</div>
        </LegacyDrawer>,
      );
    });

    const drawer = document.querySelector<HTMLElement>('.ant-drawer')!;
    expect(drawer.className).toBe('ant-drawer ant-drawer-top ant-drawer-open');
    expect(drawer.getAttribute('style')).toBe('z-index: 1200; position: absolute;');
    expect(document.querySelector('.ant-drawer-mask')?.getAttribute('style')).toBe(
      'opacity: 0.25;',
    );
    expect(document.querySelector('.ant-drawer-content-wrapper')?.getAttribute('style')).toBe(
      'height: 320px;',
    );
    expect(document.querySelector('.ant-drawer-wrapper-body')?.getAttribute('style')).toBe(
      'background-color: rgb(250, 250, 250);',
    );
    expect(
      document.querySelector<HTMLElement>('.ant-drawer-header')?.style.borderBottomWidth,
    ).toBe('0px');
    expect(document.querySelector('.ant-drawer-body')?.getAttribute('style')).toBe('padding: 0px;');
    expect(afterVisibleChange).toHaveBeenCalledWith(true);
  });

  it('supports old no-mask and no-title header classes', async () => {
    await act(async () => {
      root.render(
        <LegacyDrawer mask={false} open onClose={vi.fn()}>
          <div>无标题</div>
        </LegacyDrawer>,
      );
    });

    expect(document.querySelector('.ant-drawer')?.className).toBe(
      'ant-drawer ant-drawer-right ant-drawer-open no-mask',
    );
    expect(document.querySelector('.ant-drawer-mask')).toBeNull();
    expect(document.querySelector('.ant-drawer-header-no-title')?.outerHTML).toContain(
      '<button aria-label="Close" class="ant-drawer-close">',
    );
    expect(document.body.getAttribute('style')).toBeNull();
  });

  it('renders inline when getContainer is false and keeps custom prefix classes', async () => {
    await act(async () => {
      root.render(
        <LegacyDrawer
          className="panel"
          getContainer={false}
          open
          prefixCls="legacy-drawer"
          title="内联"
          wrapClassName="wrapped"
          onClose={vi.fn()}
        >
          <div>内容</div>
        </LegacyDrawer>,
      );
    });

    expect(container.querySelector('.legacy-drawer')).not.toBeNull();
    expect(document.body.querySelector('.legacy-drawer')).not.toBeNull();
    expect(container.querySelector('.legacy-drawer')?.className).toBe(
      'legacy-drawer legacy-drawer-right legacy-drawer-open wrapped panel',
    );
    expect(document.body.querySelector('.legacy-drawer-title')?.textContent).toBe('内联');
  });

  it('pushes and pulls the parent drawer when an old nested drawer opens and closes', async () => {
    const renderNested = async (childOpen: boolean) => {
      await act(async () => {
        root.render(
          <LegacyDrawer id="parent" open title="父抽屉" width="80%" onClose={vi.fn()}>
            <LegacyDrawer id="child" open={childOpen} title="子抽屉" width="60%" onClose={vi.fn()}>
              <div>子内容</div>
            </LegacyDrawer>
            <div>父内容</div>
          </LegacyDrawer>,
        );
      });
    };

    await renderNested(true);

    expect(document.querySelector<HTMLElement>('#parent')?.style.transform).toBe(
      'translateX(-180px)',
    );
    expect(document.querySelector('#child')).not.toBeNull();

    await renderNested(false);

    expect(document.querySelector<HTMLElement>('#parent')?.style.transform).toBe('');
    expect(document.querySelector('#child')).toBeNull();
  });

  it('keeps old forceRender closed drawer transform without locking body scroll', async () => {
    await act(async () => {
      root.render(
        <LegacyDrawer forceRender open={false} title="关闭" onClose={vi.fn()}>
          <div>仍渲染</div>
        </LegacyDrawer>,
      );
    });

    const drawer = document.querySelector<HTMLElement>('.ant-drawer')!;
    expect(drawer.className).toBe('ant-drawer ant-drawer-right');
    expect(document.querySelector('.ant-drawer-content-wrapper')?.getAttribute('style')).toContain(
      'transform: translateX(100%)',
    );
    expect(document.querySelector('.ant-drawer-body')?.textContent).toBe('仍渲染');
    expect(document.body.getAttribute('style')).toBeNull();
  });

  it('closes from the old mask and close button interactions', async () => {
    const onClose = vi.fn();
    await act(async () => {
      root.render(
        <LegacyDrawer id="knowledge" open title="新增知识" width="80%" onClose={onClose}>
          <div>表单</div>
        </LegacyDrawer>,
      );
    });

    document.querySelector<HTMLElement>('.ant-drawer-mask')!.click();
    document.querySelector<HTMLElement>('.ant-drawer-close')!.click();

    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('respects old maskClosable and keyboard switches', async () => {
    const onClose = vi.fn();
    await act(async () => {
      root.render(
        <LegacyDrawer
          keyboard={false}
          maskClosable={false}
          open
          title="不可通过遮罩关闭"
          onClose={onClose}
        >
          <div>表单</div>
        </LegacyDrawer>,
      );
    });

    document.querySelector<HTMLElement>('.ant-drawer-mask')!.click();
    document
      .querySelector<HTMLElement>('.ant-drawer')!
      .dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
    expect(onClose).not.toHaveBeenCalled();

    document.querySelector<HTMLElement>('.ant-drawer-close')!.click();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('keeps the old rc-drawer focused root keyboard close behavior', async () => {
    const onClose = vi.fn();
    await act(async () => {
      root.render(
        <LegacyDrawer id="knowledge" open title="新增知识" width="80%" onClose={onClose}>
          <div>表单</div>
        </LegacyDrawer>,
      );
    });

    const drawer = document.querySelector<HTMLElement>('#knowledge')!;
    expect(document.activeElement).toBe(drawer);

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect(onClose).not.toHaveBeenCalled();

    drawer.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('keeps body scroll locked until all open drawers are closed', async () => {
    await act(async () => {
      root.render(
        <>
          <LegacyDrawer id="one" open title="一" onClose={vi.fn()}>
            <div>一</div>
          </LegacyDrawer>
          <LegacyDrawer id="two" open title="二" onClose={vi.fn()}>
            <div>二</div>
          </LegacyDrawer>
        </>,
      );
    });

    expect(document.body.getAttribute('style')).toBe('overflow: hidden; touch-action: none;');

    await act(async () => {
      root.render(
        <LegacyDrawer id="two" open title="二" onClose={vi.fn()}>
          <div>二</div>
        </LegacyDrawer>,
      );
    });

    expect(document.body.getAttribute('style')).toBe('overflow: hidden; touch-action: none;');

    await act(async () => {
      root.render(<></>);
    });

    expect(document.body.getAttribute('style')).toBeNull();
  });
});
