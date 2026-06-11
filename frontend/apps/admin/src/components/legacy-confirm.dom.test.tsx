import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  LegacyConfirmProvider,
  legacyConfirm,
  legacyDestroyAll,
  legacyError,
  legacyInfo,
  legacySuccess,
  legacyWarning,
} from './legacy-confirm';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyConfirmProvider runtime DOM', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(async () => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    await act(async () => {
      root.render(<LegacyConfirmProvider />);
    });
  });

  afterEach(() => {
    act(() => {
      legacyDestroyAll();
      root.unmount();
    });
    container.remove();
    document.body.innerHTML = '';
    document.body.className = '';
    vi.useRealTimers();
  });

  it('uses the original Modal.info justOkText default', async () => {
    let result: Promise<boolean>;

    await act(async () => {
      result = legacyInfo({ title: '提示' });
      await Promise.resolve();
    });

    const buttons = Array.from(
      document.body.querySelectorAll<HTMLButtonElement>('.ant-modal-confirm-btns .ant-btn'),
    );
    expect(document.body.querySelector('.ant-modal-confirm-info')).not.toBeNull();
    expect(buttons).toHaveLength(1);
    expect(buttons[0]?.textContent).toBe('知道了');

    await act(async () => {
      buttons[0]?.click();
      await result;
    });

    await expect(result!).resolves.toBe(true);
  });

  it('keeps the original Modal.confirm cancel and ok defaults', async () => {
    let result: Promise<boolean>;

    await act(async () => {
      result = legacyConfirm({ title: '删除节点?' });
      await Promise.resolve();
    });

    const buttons = Array.from(
      document.body.querySelectorAll<HTMLButtonElement>('.ant-modal-confirm-btns .ant-btn'),
    );
    expect(document.body.querySelector('.ant-modal-confirm-confirm')).not.toBeNull();
    expect(buttons.map((button) => button.textContent)).toEqual(['取 消', '确 定']);

    await act(async () => {
      buttons[1]?.click();
      await result;
    });

    await expect(result!).resolves.toBe(true);
  });

  it('renders separate old static confirms instead of queueing them', async () => {
    let first: Promise<boolean>;
    let second: Promise<boolean>;

    await act(async () => {
      first = legacyConfirm({ title: '第一个确认' });
      second = legacyConfirm({ title: '第二个确认' });
      await Promise.resolve();
    });

    let modals = Array.from(document.body.querySelectorAll<HTMLElement>('.ant-modal-confirm'));
    expect(modals).toHaveLength(2);
    expect(modals.map((modal) => modal.querySelector('.ant-modal-confirm-title')?.textContent)).toEqual([
      '第一个确认',
      '第二个确认',
    ]);

    await act(async () => {
      modals[0]?.querySelector<HTMLButtonElement>('.ant-modal-confirm-btns .ant-btn')?.click();
      await first!;
    });

    await expect(first!).resolves.toBe(false);
    modals = Array.from(document.body.querySelectorAll<HTMLElement>('.ant-modal-confirm'));
    expect(modals).toHaveLength(1);
    expect(modals[0]?.querySelector('.ant-modal-confirm-title')?.textContent).toBe('第二个确认');

    await act(async () => {
      modals[0]?.querySelector<HTMLButtonElement>('.ant-btn-primary')?.click();
      await second!;
    });

    await expect(second!).resolves.toBe(true);
  });

  it('keeps the old rc-dialog focus sentinels for confirm modals', async () => {
    vi.useFakeTimers();
    let result: Promise<boolean>;

    await act(async () => {
      result = legacyConfirm({ title: '删除节点?' });
      await Promise.resolve();
    });

    const wrap = document.body.querySelector<HTMLElement>('.ant-modal-wrap');
    const sentinels = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-modal-confirm > [aria-hidden="true"]'),
    );
    expect(sentinels).toHaveLength(2);

    await act(async () => {
      vi.runOnlyPendingTimers();
    });

    sentinels[1]!.focus();
    await act(async () => {
      wrap?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
      await Promise.resolve();
    });
    expect(document.activeElement).toBe(sentinels[0]);

    await act(async () => {
      sentinels[0]!.focus();
      wrap?.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true }),
      );
      await Promise.resolve();
    });
    expect(document.activeElement).toBe(sentinels[1]);

    await act(async () => {
      document.body.querySelector<HTMLButtonElement>('.ant-btn-primary')?.click();
      await result;
    });
  });

  it('ignores mask clicks during the original rc-dialog opening guard', async () => {
    vi.useFakeTimers();
    let result: Promise<boolean>;

    await act(async () => {
      result = legacyConfirm({ title: '删除节点?', maskClosable: true });
      await Promise.resolve();
    });

    const wrap = document.body.querySelector<HTMLElement>('.ant-modal-wrap')!;
    await act(async () => {
      wrap.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(document.body.querySelector('.ant-modal-confirm')).not.toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(301);
      wrap.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    await expect(result!).resolves.toBe(false);
  });

  it('keeps the original Modal.confirm maskClosable default disabled', async () => {
    vi.useFakeTimers();
    let result: Promise<boolean>;

    await act(async () => {
      result = legacyConfirm({ title: '删除节点?' });
      await Promise.resolve();
    });

    const wrap = document.body.querySelector<HTMLElement>('.ant-modal-wrap')!;
    await act(async () => {
      vi.advanceTimersByTime(301);
      wrap.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(document.body.querySelector('.ant-modal-confirm')).not.toBeNull();

    await act(async () => {
      document.body.querySelector<HTMLButtonElement>('.ant-modal-confirm-btns .ant-btn')?.click();
      await result!;
    });

    await expect(result!).resolves.toBe(false);
  });

  it('renders the old static success, error and warning confirm variants', async () => {
    const cases = [
      [legacySuccess, 'success', 'check-circle'],
      [legacyError, 'error', 'close-circle'],
      [legacyWarning, 'warning', 'exclamation-circle'],
    ] as const;

    for (const [openConfirm, type, icon] of cases) {
      let result: Promise<boolean>;

      await act(async () => {
        result = openConfirm({ title: `${type} title` });
        await Promise.resolve();
      });

      expect(document.body.querySelector(`.ant-modal-confirm-${type}`)).not.toBeNull();
      expect(document.body.querySelector(`[data-icon="${icon}"]`)).not.toBeNull();
      const buttons = Array.from(
        document.body.querySelectorAll<HTMLButtonElement>('.ant-modal-confirm-btns .ant-btn'),
      );
      expect(buttons).toHaveLength(1);
      expect(buttons[0]?.textContent).toBe('知道了');

      await act(async () => {
        buttons[0]?.click();
        await result!;
      });

      await expect(result!).resolves.toBe(true);
    }
  });

  it('honors the old centered classes, custom styles and getContainer false', async () => {
    let result: Promise<boolean>;

    await act(async () => {
      result = legacyConfirm({
        title: '内联确认',
        centered: true,
        className: 'confirm-extra',
        getContainer: false,
        maskStyle: { opacity: 0.25 },
        style: { top: 24 },
        width: 480,
        zIndex: 1100,
      });
      await Promise.resolve();
    });

    expect(container.querySelector('.ant-modal-root')).not.toBeNull();
    expect(container.querySelector('.ant-modal-wrap')?.className).toBe(
      'ant-modal-wrap ant-modal-centered ant-modal-confirm-centered',
    );
    expect(container.querySelector('.ant-modal')?.className).toBe(
      'ant-modal ant-modal-confirm ant-modal-confirm-confirm confirm-extra',
    );
    expect(container.querySelector('.ant-modal-mask')?.getAttribute('style')).toBe(
      'z-index: 1100; opacity: 0.25;',
    );
    expect(container.querySelector('.ant-modal-wrap')?.getAttribute('style')).toBe(
      'z-index: 1100;',
    );
    expect(container.querySelector('.ant-modal')?.getAttribute('style')).toBe(
      'top: 24px; width: 480px;',
    );

    await act(async () => {
      container.querySelector<HTMLButtonElement>('.ant-btn-primary')?.click();
      await result!;
    });

    await expect(result!).resolves.toBe(true);
  });

  it('uses the old autoFocusButton behavior', async () => {
    vi.useFakeTimers();
    let result: Promise<boolean>;

    await act(async () => {
      result = legacyConfirm({ title: '聚焦取消', autoFocusButton: 'cancel' });
      await Promise.resolve();
    });

    await act(async () => {
      vi.runOnlyPendingTimers();
    });

    const buttons = Array.from(
      document.body.querySelectorAll<HTMLButtonElement>('.ant-modal-confirm-btns .ant-btn'),
    );
    expect(document.activeElement).toBe(buttons[0]);

    await act(async () => {
      buttons[1]?.click();
      await result!;
    });

    await expect(result!).resolves.toBe(true);
  });

  it('keeps ActionButton promise loading and reject behavior', async () => {
    let resolveOk: () => void = () => {};
    const onOk = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveOk = resolve;
        }),
    );
    let result: Promise<boolean>;

    await act(async () => {
      result = legacyConfirm({ title: '异步确认', onOk });
      await Promise.resolve();
    });

    const ok = document.body.querySelector<HTMLButtonElement>('.ant-btn-primary')!;
    await act(async () => {
      ok.click();
      await Promise.resolve();
    });

    expect(onOk).toHaveBeenCalledTimes(1);
    expect(document.body.querySelector('.ant-modal-confirm')).not.toBeNull();
    expect(ok.className).toContain('ant-btn-loading');

    await act(async () => {
      resolveOk();
      await Promise.resolve();
    });

    await expect(result!).resolves.toBe(true);
    expect(document.body.querySelector('.ant-modal-confirm')).toBeNull();

    const error = new Error('nope');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    await act(async () => {
      result = legacyConfirm({ title: '拒绝确认', onOk: () => Promise.reject(error) });
      await Promise.resolve();
    });

    const rejectedOk = document.body.querySelector<HTMLButtonElement>('.ant-btn-primary')!;
    await act(async () => {
      rejectedOk.click();
      await Promise.resolve();
    });

    expect(consoleError).toHaveBeenCalledWith(error);
    expect(document.body.querySelector('.ant-modal-confirm')).not.toBeNull();
    expect(rejectedOk.className).not.toContain('ant-btn-loading');

    await act(async () => {
      document.body.querySelector<HTMLButtonElement>('.ant-modal-confirm-btns .ant-btn')?.click();
      await result!;
    });

    await expect(result!).resolves.toBe(false);
    consoleError.mockRestore();
  });

  it('supports old update and destroy handles on the promise facade', async () => {
    let result: ReturnType<typeof legacyConfirm>;

    await act(async () => {
      result = legacyConfirm({ title: '旧标题', okText: '确认' });
      await Promise.resolve();
    });

    expect(document.body.querySelector('.ant-modal-confirm-title')?.textContent).toBe('旧标题');
    expect(document.body.querySelector('.ant-btn-primary')?.textContent).toBe('确 认');

    await act(async () => {
      result.update({ title: '新标题', okText: '保存' });
      await Promise.resolve();
    });

    expect(document.body.querySelector('.ant-modal-confirm-title')?.textContent).toBe('新标题');
    expect(document.body.querySelector('.ant-btn-primary')?.textContent).toBe('保 存');

    await act(async () => {
      result.destroy();
      await result;
    });

    await expect(result!).resolves.toBe(false);
    expect(document.body.querySelector('.ant-modal-confirm')).toBeNull();
  });
});
