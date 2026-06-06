import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacyModal } from './legacy-modal';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyModal', () => {
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
    document.body.className = '';
  });

  it('renders the old Ant Design modal shell in a body portal', async () => {
    await act(async () => {
      root.render(
        <LegacyModal visible title="配置默认主题" onCancel={vi.fn()} onOk={vi.fn()}>
          <div className="form-group">字段</div>
        </LegacyModal>,
      );
    });

    const modalRoot = document.querySelector('.ant-modal-root')!;
    expect(modalRoot.outerHTML).toContain(
      '<div class="ant-modal-root"><div class="ant-modal-mask"></div><div tabindex="-1" class="ant-modal-wrap" role="dialog">',
    );
    expect(modalRoot.querySelector('.ant-modal')?.outerHTML).toContain(
      '<div class="ant-modal" role="document" style="width: 520px;">',
    );
    expect(modalRoot.querySelector('.ant-modal-header')?.outerHTML).toBe(
      '<div class="ant-modal-header"><div class="ant-modal-title">配置默认主题</div></div>',
    );
    expect(modalRoot.querySelector('.ant-modal-close')?.outerHTML).toContain(
      '<button type="button" aria-label="Close" class="ant-modal-close"><span class="ant-modal-close-x"><i aria-label="图标: close" class="anticon anticon-close">',
    );
    expect(modalRoot.querySelector('.ant-modal-body')?.outerHTML).toBe(
      '<div class="ant-modal-body"><div class="form-group">字段</div></div>',
    );
    expect(modalRoot.querySelector('.ant-modal-footer')?.outerHTML).toContain(
      '<button type="button" class="ant-btn"><span>取 消</span></button><button type="button" class="ant-btn ant-btn-primary"><span>确 定</span></button>',
    );
    expect(document.body.classList.contains('ant-modal-open')).toBe(true);
    expect(container.children).toHaveLength(0);
  });

  it('keeps the old primary button loading icon and disables clicks through LegacyButton', async () => {
    const onOk = vi.fn();
    await act(async () => {
      root.render(
        <LegacyModal
          visible
          title="配置默认主题"
          okButtonProps={{ loading: true }}
          onCancel={vi.fn()}
          onOk={onOk}
        >
          <div />
        </LegacyModal>,
      );
    });

    const ok = document.querySelector<HTMLElement>('.ant-modal-footer .ant-btn-primary')!;
    expect(ok.outerHTML).toContain('class="ant-btn ant-btn-primary ant-btn-loading"');
    expect(ok.outerHTML).toContain('aria-label="图标: loading"');
    ok.click();
    expect(onOk).not.toHaveBeenCalled();
  });

  it('supports the old okText and cancelText footer labels', async () => {
    await act(async () => {
      root.render(
        <LegacyModal
          visible
          title="添加支付方式"
          okText="添加"
          cancelText="取消"
          onCancel={vi.fn()}
          onOk={vi.fn()}
        >
          <div />
        </LegacyModal>,
      );
    });

    expect(document.querySelector('.ant-modal-footer')?.outerHTML).toContain(
      '<button type="button" class="ant-btn"><span>取 消</span></button><button type="button" class="ant-btn ant-btn-primary"><span>添 加</span></button>',
    );
  });

  it('supports the old bodyStyle, style, width and hidden footer props', async () => {
    await act(async () => {
      root.render(
        <LegacyModal
          visible
          width="100%"
          style={{ maxWidth: 1000, padding: '0 10px', top: 20 }}
          bodyStyle={{ padding: 0 }}
          footer={false}
          title="流量记录"
          onCancel={vi.fn()}
        >
          <div>表格</div>
        </LegacyModal>,
      );
    });

    expect(document.querySelector('.ant-modal')?.getAttribute('style')).toBe(
      'width: 100%; max-width: 1000px; padding: 0px 10px; top: 20px;',
    );
    expect(document.querySelector('.ant-modal-body')?.getAttribute('style')).toBe('padding: 0px;');
    expect(document.querySelector('.ant-modal-footer')).toBeNull();
  });

  it('also accepts the Ant Design 5 styles.body alias while rendering old DOM', async () => {
    await act(async () => {
      root.render(
        <LegacyModal open title="流量记录" styles={{ body: { padding: 0 } }} onCancel={vi.fn()}>
          <div>表格</div>
        </LegacyModal>,
      );
    });

    expect(document.querySelector('.ant-modal-body')?.getAttribute('style')).toBe('padding: 0px;');
  });

  it('closes from the old mask, close button and cancel button interactions', async () => {
    const onCancel = vi.fn();
    await act(async () => {
      root.render(
        <LegacyModal visible title="配置默认主题" onCancel={onCancel} onOk={vi.fn()}>
          <div />
        </LegacyModal>,
      );
    });

    document.querySelector<HTMLElement>('.ant-modal-wrap')!.click();
    document.querySelector<HTMLElement>('.ant-modal-close')!.click();
    document.querySelector<HTMLElement>('.ant-modal-footer .ant-btn')!.click();

    expect(onCancel).toHaveBeenCalledTimes(3);
  });

  it('renders nothing while closed', async () => {
    await act(async () => {
      root.render(
        <LegacyModal visible={false} title="配置默认主题" onCancel={vi.fn()} onOk={vi.fn()}>
          <div />
        </LegacyModal>,
      );
    });

    expect(document.querySelector('.ant-modal-root')).toBeNull();
  });
});
