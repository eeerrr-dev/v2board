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
      '<div class="ant-modal-root"><div class="ant-modal-mask"></div><div tabindex="-1" class="ant-modal-wrap" role="dialog" aria-labelledby="rcDialogTitle',
    );
    expect(modalRoot.querySelector('.ant-modal')?.outerHTML).toContain(
      '<div class="ant-modal" role="document" style="width: 520px;">',
    );
    expect(modalRoot.querySelectorAll('.ant-modal > div[aria-hidden="true"]')).toHaveLength(2);
    const title = modalRoot.querySelector<HTMLElement>('.ant-modal-title')!;
    expect(title.id).toMatch(/^rcDialogTitle\d+$/);
    expect(modalRoot.querySelector('.ant-modal-wrap')?.getAttribute('aria-labelledby')).toBe(
      title.id,
    );
    expect(title.textContent).toBe('配置默认主题');
    expect(modalRoot.querySelector('.ant-modal-close')?.outerHTML).toContain(
      '<button type="button" aria-label="Close" class="ant-modal-close"><span class="ant-modal-close-x"><i aria-label="图标: close" class="anticon anticon-close ant-modal-close-icon">',
    );
    expect(modalRoot.querySelector('.ant-modal-body')?.outerHTML).toBe(
      '<div class="ant-modal-body"><div class="form-group">字段</div></div>',
    );
    expect(modalRoot.querySelector('.ant-modal-footer')?.outerHTML).toContain(
      '<div><button type="button" class="ant-btn ant-btn-two-chinese-chars"><span>取 消</span></button><button type="button" class="ant-btn ant-btn-primary ant-btn-two-chinese-chars"><span>确 定</span></button></div>',
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
    expect(ok.className).toContain('ant-btn-loading');
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
      '<div><button type="button" class="ant-btn ant-btn-two-chinese-chars"><span>取 消</span></button><button type="button" class="ant-btn ant-btn-primary ant-btn-two-chinese-chars"><span>添 加</span></button></div>',
    );
  });

  it('does not create an old rc-dialog aria title link without a title', async () => {
    await act(async () => {
      root.render(
        <LegacyModal visible onCancel={vi.fn()} onOk={vi.fn()}>
          <div />
        </LegacyModal>,
      );
    });

    expect(document.querySelector('.ant-modal-header')).toBeNull();
    expect(document.querySelector('.ant-modal-wrap')?.hasAttribute('aria-labelledby')).toBe(
      false,
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
      'max-width: 1000px; padding: 0px 10px; top: 20px; width: 100%;',
    );
    expect(document.querySelector('.ant-modal-body')?.getAttribute('style')).toBe('padding: 0px;');
    expect(document.querySelector('.ant-modal-footer')).toBeNull();
  });

  it('supports the old centered, class, zIndex, maskStyle and closable props', async () => {
    await act(async () => {
      root.render(
        <LegacyModal
          visible
          centered
          closable={false}
          className="modal-extra"
          wrapClassName="wrap-extra"
          maskStyle={{ opacity: 0.5 }}
          zIndex={1001}
          title="配置默认主题"
          onCancel={vi.fn()}
        >
          <div />
        </LegacyModal>,
      );
    });

    expect(document.querySelector('.ant-modal-wrap')?.className).toBe(
      'ant-modal-wrap ant-modal-centered wrap-extra',
    );
    expect(document.querySelector('.ant-modal')?.className).toBe('ant-modal modal-extra');
    expect(document.querySelector('.ant-modal-mask')?.getAttribute('style')).toBe(
      'z-index: 1001; opacity: 0.5;',
    );
    expect(document.querySelector('.ant-modal-wrap')?.getAttribute('style')).toBe(
      'z-index: 1001;',
    );
    expect(document.querySelector('.ant-modal-close')).toBeNull();
  });

  it('respects the old mask, maskClosable, keyboard and closeIcon controls', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);
    const onCancel = vi.fn();
    await act(async () => {
      root.render(
        <LegacyModal
          visible
          mask={false}
          keyboard={false}
          closeIcon={<span className="custom-close">x</span>}
          title="配置默认主题"
          onCancel={onCancel}
        >
          <div />
        </LegacyModal>,
      );
    });

    vi.setSystemTime(1301);
    expect(document.querySelector('.ant-modal-mask')).toBeNull();
    expect(document.querySelector('.custom-close')?.outerHTML).toBe(
      '<span class="custom-close">x</span>',
    );
    document.querySelector<HTMLElement>('.ant-modal-wrap')!.click();
    document
      .querySelector<HTMLElement>('.ant-modal-wrap')!
      .dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
    expect(onCancel).not.toHaveBeenCalled();

    document.querySelector<HTMLElement>('.ant-modal-close')!.click();
    expect(onCancel).toHaveBeenCalledTimes(1);

    await act(async () => {
      root.render(
        <LegacyModal visible maskClosable={false} title="配置默认主题" onCancel={onCancel}>
          <div />
        </LegacyModal>,
      );
    });

    document.querySelector<HTMLElement>('.ant-modal-wrap')!.click();
    expect(onCancel).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });

  it('passes old footer button props, okType and confirmLoading through LegacyButton', async () => {
    const onOk = vi.fn();
    await act(async () => {
      root.render(
        <LegacyModal
          visible
          title="删除配置"
          okText="删除"
          okType="danger"
          confirmLoading
          cancelButtonProps={{ className: 'cancel-extra', disabled: true }}
          okButtonProps={{ className: 'ok-extra', disabled: true, style: { marginLeft: 4 } }}
          onCancel={vi.fn()}
          onOk={onOk}
        >
          <div />
        </LegacyModal>,
      );
    });

    const cancel = document.querySelector<HTMLButtonElement>('.ant-modal-footer .cancel-extra')!;
    const ok = document.querySelector<HTMLButtonElement>('.ant-modal-footer .ok-extra')!;
    expect(cancel.disabled).toBe(true);
    expect(cancel.className).toContain('ant-btn');
    expect(ok.disabled).toBe(true);
    expect(ok.className).toContain('ant-btn-danger');
    expect(ok.className).toContain('ant-btn-loading');
    expect(ok.getAttribute('style')).toBe('margin-left: 4px;');
    expect(ok.outerHTML).toContain('aria-label="图标: loading"');
    ok.click();
    expect(onOk).not.toHaveBeenCalled();
  });

  it('renders inline with getContainer false and calls afterClose after a visible close', async () => {
    const afterClose = vi.fn();
    await act(async () => {
      root.render(
        <LegacyModal
          visible
          getContainer={false}
          title="配置默认主题"
          afterClose={afterClose}
          onCancel={vi.fn()}
        >
          <div />
        </LegacyModal>,
      );
    });

    expect(container.querySelector('.ant-modal-root')).not.toBeNull();

    await act(async () => {
      root.render(
        <LegacyModal
          visible={false}
          getContainer={false}
          title="配置默认主题"
          afterClose={afterClose}
          onCancel={vi.fn()}
        >
          <div />
        </LegacyModal>,
      );
    });

    expect(container.querySelector('.ant-modal-root')).toBeNull();
    expect(afterClose).toHaveBeenCalledTimes(1);
  });

  it('keeps force-rendered closed modals hidden and honors destroyOnClose content removal', async () => {
    await act(async () => {
      root.render(
        <LegacyModal visible={false} forceRender title="配置默认主题" onCancel={vi.fn()}>
          <div className="persisted-body">字段</div>
        </LegacyModal>,
      );
    });

    expect(document.querySelector('.ant-modal-root')).not.toBeNull();
    expect(document.querySelector('.ant-modal-wrap')?.getAttribute('style')).toBe(
      'display: none;',
    );
    expect(document.querySelector('.persisted-body')).not.toBeNull();

    await act(async () => {
      root.render(
        <LegacyModal
          visible={false}
          forceRender
          destroyOnClose
          title="配置默认主题"
          onCancel={vi.fn()}
        >
          <div className="destroyed-body">字段</div>
        </LegacyModal>,
      );
    });

    expect(document.querySelector('.ant-modal-root')).not.toBeNull();
    expect(document.querySelector('.destroyed-body')).toBeNull();
  });

  it('keeps the old rc-dialog hidden mask and pass-through element props', async () => {
    await act(async () => {
      root.render(
        <LegacyModal
          visible={false}
          forceRender
          title="配置默认主题"
          zIndex={1003}
          maskStyle={{ opacity: 0.25 }}
          maskProps={{ className: 'mask-extra', style: { pointerEvents: 'none' } }}
          wrapStyle={{ top: 24 }}
          wrapProps={{ className: 'wrap-prop' }}
          bodyProps={{ className: 'body-extra', style: { minHeight: 60 } }}
          onCancel={vi.fn()}
        >
          <div className="persisted-body">字段</div>
        </LegacyModal>,
      );
    });

    const mask = document.querySelector<HTMLElement>('.ant-modal-mask')!;
    const wrap = document.querySelector<HTMLElement>('.ant-modal-wrap')!;
    const body = document.querySelector<HTMLElement>('.ant-modal-body')!;

    expect(mask.className).toBe('ant-modal-mask ant-modal-mask-hidden mask-extra');
    expect(mask.getAttribute('style')).toBe(
      'z-index: 1003; opacity: 0.25; pointer-events: none;',
    );
    expect(wrap.className).toBe('ant-modal-wrap wrap-prop');
    expect(wrap.getAttribute('style')).toBe('z-index: 1003; top: 24px; display: none;');
    expect(body.className).toBe('ant-modal-body body-extra');
    expect(body.getAttribute('style')).toBe('min-height: 60px;');
  });

  it('uses the old rc-dialog mousePosition origin and height prop', async () => {
    await act(async () => {
      root.render(
        <LegacyModal
          visible
          title="配置默认主题"
          height={300}
          mousePosition={{ x: 32, y: 48 }}
          onCancel={vi.fn()}
        >
          <div />
        </LegacyModal>,
      );
      await Promise.resolve();
    });

    const modal = document.querySelector<HTMLElement>('.ant-modal')!;
    expect(modal.getAttribute('style')).toBe(
      'width: 520px; height: 300px; transform-origin: 32px 48px;',
    );
  });

  it('keeps the body open class until all visible modals have closed', async () => {
    const renderModals = async (firstVisible: boolean, secondVisible: boolean) => {
      await act(async () => {
        root.render(
          <>
            <LegacyModal visible={firstVisible} title="第一个" onCancel={vi.fn()}>
              <div />
            </LegacyModal>
            <LegacyModal visible={secondVisible} title="第二个" onCancel={vi.fn()}>
              <div />
            </LegacyModal>
          </>,
        );
      });
    };

    await renderModals(true, true);
    expect(document.body.classList.contains('ant-modal-open')).toBe(true);

    await renderModals(false, true);
    expect(document.body.classList.contains('ant-modal-open')).toBe(true);

    await renderModals(false, false);
    expect(document.body.classList.contains('ant-modal-open')).toBe(false);
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

  it('closes from the old mask, close button, keyboard and cancel button interactions', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);
    const onCancel = vi.fn();
    await act(async () => {
      root.render(
        <LegacyModal visible title="配置默认主题" onCancel={onCancel} onOk={vi.fn()}>
          <div />
        </LegacyModal>,
      );
    });

    document.querySelector<HTMLElement>('.ant-modal-wrap')!.click();
    expect(onCancel).not.toHaveBeenCalled();

    vi.setSystemTime(1301);
    document.querySelector<HTMLElement>('.ant-modal-wrap')!.click();
    document.querySelector<HTMLElement>('.ant-modal-close')!.click();
    document
      .querySelector<HTMLElement>('.ant-modal-wrap')!
      .dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
    document.querySelector<HTMLElement>('.ant-modal-footer .ant-btn')!.click();

    expect(onCancel).toHaveBeenCalledTimes(4);
    vi.useRealTimers();
  });

  it('keeps the old rc-dialog guard against mask closes after dialog mouse down', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);
    const onCancel = vi.fn();
    await act(async () => {
      root.render(
        <LegacyModal visible title="配置默认主题" onCancel={onCancel} onOk={vi.fn()}>
          <div />
        </LegacyModal>,
      );
    });

    vi.setSystemTime(1301);
    const modal = document.querySelector<HTMLElement>('.ant-modal')!;
    const wrap = document.querySelector<HTMLElement>('.ant-modal-wrap')!;

    await act(async () => {
      modal.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
      wrap.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
      wrap.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onCancel).not.toHaveBeenCalled();

    await act(async () => {
      vi.runOnlyPendingTimers();
      wrap.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onCancel).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });

  it('keeps the old focus sentinels cycling tab focus inside the modal', async () => {
    await act(async () => {
      root.render(
        <LegacyModal visible title="配置默认主题" onCancel={vi.fn()} onOk={vi.fn()}>
          <div />
        </LegacyModal>,
      );
    });

    const sentinels = document.querySelectorAll<HTMLDivElement>('.ant-modal > div[aria-hidden]');
    sentinels[1]!.focus();
    await act(async () => {
      sentinels[1]!.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Tab' }));
    });
    expect(document.activeElement).toBe(sentinels[0]);

    await act(async () => {
      sentinels[0]!.dispatchEvent(
        new KeyboardEvent('keydown', { bubbles: true, key: 'Tab', shiftKey: true }),
      );
    });
    expect(document.activeElement).toBe(sentinels[1]);
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
