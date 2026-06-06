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
});
