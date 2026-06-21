import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacyTabs } from './legacy-tabs';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyTabs', () => {
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

  it('renders the old Ant Design 3 line tab DOM without v5 operations', () => {
    const html = renderToStaticMarkup(
      <LegacyTabs defaultActiveKey="site" size="large">
        <LegacyTabs.TabPane tab="站点" key="site">
          站点内容
        </LegacyTabs.TabPane>
        <LegacyTabs.TabPane tab="安全" key="safe">
          安全内容
        </LegacyTabs.TabPane>
      </LegacyTabs>,
    );

    expect(html).toContain('class="ant-tabs ant-tabs-top ant-tabs-large ant-tabs-line"');
    expect(html).toContain(
      'role="tablist" class="ant-tabs-bar ant-tabs-top-bar ant-tabs-large-bar" tabindex="0"',
    );
    expect(html).toContain('class="ant-tabs-nav-container"');
    expect(html).toContain(
      '<span unselectable="unselectable" class="ant-tabs-tab-prev ant-tabs-tab-btn-disabled">',
    );
    expect(html).toContain(
      '<span unselectable="unselectable" class="ant-tabs-tab-next ant-tabs-tab-btn-disabled">',
    );
    expect(html).toContain('class="ant-tabs-nav-scroll"');
    expect(html).toContain('class="ant-tabs-nav ant-tabs-nav-animated"');
    expect(html).toContain(
      'role="tab" aria-disabled="false" aria-selected="true" class="ant-tabs-tab-active ant-tabs-tab"',
    );
    expect(html).toContain(
      'role="tab" aria-disabled="false" aria-selected="false" class=" ant-tabs-tab"',
    );
    expect(html).toContain(
      'class="ant-tabs-content ant-tabs-content-animated ant-tabs-top-content" style="margin-left:0%"',
    );
    expect(html).toContain('class="ant-tabs-tabpane ant-tabs-tabpane-active"');
    expect(html).toContain('class="ant-tabs-tabpane ant-tabs-tabpane-inactive"');
    expect(html).toContain('站点内容');
    expect(html).not.toContain('安全内容');
    expect(html).not.toContain('ant-tabs-nav-operations');
    expect(html).not.toContain('css-dev-only-do-not-override');
  });

  it('switches active panes with the original animated content offset', async () => {
    const onChange = vi.fn();
    await act(async () => {
      root.render(
        <LegacyTabs defaultActiveKey="site" onChange={onChange} size="large">
          <LegacyTabs.TabPane tab="站点" key="site">
            站点内容
          </LegacyTabs.TabPane>
          <LegacyTabs.TabPane tab="安全" key="safe">
            安全内容
          </LegacyTabs.TabPane>
        </LegacyTabs>,
      );
    });

    await act(async () => {
      container.querySelectorAll<HTMLElement>('.ant-tabs-tab')[1]!.click();
    });

    expect(onChange).toHaveBeenCalledWith('safe');
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('安全');
    expect(container.querySelector('.ant-tabs-content')?.getAttribute('style')).toContain(
      'margin-left: -100%;',
    );
    expect(container.querySelectorAll('.ant-tabs-tabpane')[0]!.textContent).toContain('站点内容');
    expect(container.querySelectorAll('.ant-tabs-tabpane')[1]!.textContent).toContain('安全内容');
  });

  it('matches old controlled activeKey behavior and passes wrapper props through', async () => {
    const onChange = vi.fn();
    await act(async () => {
      root.render(
        <LegacyTabs
          activeKey="safe"
          className="config-tabs"
          onChange={onChange}
          style={{ marginTop: 4 }}
        >
          <LegacyTabs.TabPane tab="站点" key="site">
            站点内容
          </LegacyTabs.TabPane>
          <LegacyTabs.TabPane tab="安全" key="safe">
            安全内容
          </LegacyTabs.TabPane>
        </LegacyTabs>,
      );
    });

    expect(container.querySelector('.ant-tabs')?.className).toBe(
      'ant-tabs ant-tabs-top ant-tabs-line config-tabs',
    );
    expect(container.querySelector<HTMLElement>('.ant-tabs')?.style.marginTop).toBe('4px');
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('安全');

    await act(async () => {
      container.querySelectorAll<HTMLElement>('.ant-tabs-tab')[0]!.click();
    });
    expect(onChange).toHaveBeenCalledWith('site');
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('安全');
  });

  it('honors the old prefixCls across nav and arrow icon target classes', () => {
    const html = renderToStaticMarkup(
      <LegacyTabs prefixCls="legacy-tabs">
        <LegacyTabs.TabPane tab="站点" key="site">
          站点内容
        </LegacyTabs.TabPane>
      </LegacyTabs>,
    );

    expect(html).toContain('class="legacy-tabs legacy-tabs-top legacy-tabs-line"');
    expect(html).toContain('class="legacy-tabs-nav-container"');
    expect(html).toContain('legacy-tabs-tab-prev-icon-target');
    expect(html).toContain('legacy-tabs-tab-next-icon-target');
  });

  it('supports the old tabBarStyle and tabBarExtraContent props', async () => {
    await act(async () => {
      root.render(
        <LegacyTabs
          tabBarExtraContent={<button type="button">操作</button>}
          tabBarStyle={{ marginBottom: 8 }}
        >
          <LegacyTabs.TabPane tab="站点" key="site">
            站点内容
          </LegacyTabs.TabPane>
        </LegacyTabs>,
      );
    });

    const tabBar = container.querySelector<HTMLElement>('.ant-tabs-bar')!;
    const extra = container.querySelector<HTMLElement>('.ant-tabs-extra-content')!;
    expect(tabBar.style.marginBottom).toBe('8px');
    expect(tabBar.firstElementChild).toBe(extra);
    expect(extra.style.cssFloat).toBe('right');
    expect(extra.textContent).toBe('操作');
    expect(tabBar.querySelector('.ant-tabs-nav-container')).not.toBeNull();
  });

  it('shows old Ant Design 3 tab arrows when horizontal tabs overflow', async () => {
    const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
    const rect = (width: number, height = 48) =>
      ({
        bottom: height,
        height,
        left: 0,
        right: width,
        top: 0,
        width,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      }) as DOMRect;

    HTMLElement.prototype.getBoundingClientRect = function getBoundingClientRect() {
      const element = this as HTMLElement;
      if (element.classList.contains('ant-tabs-nav-container')) return rect(390);
      if (element.classList.contains('ant-tabs-nav-wrap')) return rect(326);
      if (element.classList.contains('ant-tabs-nav')) return rect(720);
      return originalGetBoundingClientRect.call(this);
    };

    try {
      await act(async () => {
        root.render(
          <LegacyTabs defaultActiveKey="site">
            <LegacyTabs.TabPane tab="站点" key="site">
              站点内容
            </LegacyTabs.TabPane>
            <LegacyTabs.TabPane tab="邮件" key="mail">
              邮件内容
            </LegacyTabs.TabPane>
            <LegacyTabs.TabPane tab="安全" key="safe">
              安全内容
            </LegacyTabs.TabPane>
            <LegacyTabs.TabPane tab="支付" key="payment">
              支付内容
            </LegacyTabs.TabPane>
            <LegacyTabs.TabPane tab="订阅" key="subscribe">
              订阅内容
            </LegacyTabs.TabPane>
            <LegacyTabs.TabPane tab="审计" key="audit">
              审计内容
            </LegacyTabs.TabPane>
          </LegacyTabs>,
        );
      });

      const navContainer = container.querySelector<HTMLElement>('.ant-tabs-nav-container')!;
      const previous = container.querySelector<HTMLElement>('.ant-tabs-tab-prev')!;
      const next = container.querySelector<HTMLElement>('.ant-tabs-tab-next')!;

      expect(navContainer.classList.contains('ant-tabs-nav-container-scrolling')).toBe(true);
      expect(previous.classList.contains('ant-tabs-tab-arrow-show')).toBe(true);
      expect(previous.classList.contains('ant-tabs-tab-btn-disabled')).toBe(true);
      expect(next.classList.contains('ant-tabs-tab-arrow-show')).toBe(true);
      expect(next.classList.contains('ant-tabs-tab-btn-disabled')).toBe(false);

      await act(async () => {
        next.click();
      });

      expect(
        container
          .querySelector<HTMLElement>('.ant-tabs-nav')
          ?.style.transform,
      ).toBe('translate3d(-326px, 0px, 0px)');
      expect(previous.classList.contains('ant-tabs-tab-btn-disabled')).toBe(false);
    } finally {
      HTMLElement.prototype.getBoundingClientRect = originalGetBoundingClientRect;
    }
  });

  it('matches old animated=false classes and keeps active content unshifted', () => {
    const html = renderToStaticMarkup(
      <LegacyTabs activeKey="safe" animated={false}>
        <LegacyTabs.TabPane tab="站点" key="site">
          站点内容
        </LegacyTabs.TabPane>
        <LegacyTabs.TabPane tab="安全" key="safe">
          安全内容
        </LegacyTabs.TabPane>
      </LegacyTabs>,
    );

    expect(html).toContain(
      'class="ant-tabs ant-tabs-top ant-tabs-line ant-tabs-no-animation"',
    );
    expect(html).toContain('class="ant-tabs-nav ant-tabs-nav-no-animated"');
    expect(html).toContain('class="ant-tabs-ink-bar ant-tabs-ink-bar-no-animated"');
    expect(html).toContain(
      'class="ant-tabs-content ant-tabs-content-no-animated ant-tabs-top-content"',
    );
    expect(html).not.toContain('margin-left:-100%');
  });

  it('uses old card and editable-card classes with default no-animation content', () => {
    const html = renderToStaticMarkup(
      <LegacyTabs type="editable-card">
        <LegacyTabs.TabPane tab="站点" key="site">
          站点内容
        </LegacyTabs.TabPane>
      </LegacyTabs>,
    );

    expect(html).toContain(
      'class="ant-tabs ant-tabs-top ant-tabs-card ant-tabs-editable-card ant-tabs-no-animation"',
    );
    expect(html).toContain('class="ant-tabs-bar ant-tabs-top-bar ant-tabs-card-bar"');
    expect(html).toContain(
      'class="ant-tabs-content ant-tabs-content-no-animated ant-tabs-top-content ant-tabs-card-content"',
    );
  });

  it('skips old disabled panes for default active key, clicks, and arrow navigation', async () => {
    const onChange = vi.fn();
    await act(async () => {
      root.render(
        <LegacyTabs onChange={onChange}>
          <LegacyTabs.TabPane disabled tab="站点" key="site">
            站点内容
          </LegacyTabs.TabPane>
          <LegacyTabs.TabPane tab="安全" key="safe">
            安全内容
          </LegacyTabs.TabPane>
          <LegacyTabs.TabPane tab="支付" key="payment">
            支付内容
          </LegacyTabs.TabPane>
        </LegacyTabs>,
      );
    });

    expect(container.querySelector('.ant-tabs-tab-disabled')?.textContent).toBe('站点');
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('安全');

    await act(async () => {
      container.querySelectorAll<HTMLElement>('.ant-tabs-tab')[0]!.click();
    });
    expect(onChange).not.toHaveBeenCalled();
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('安全');

    const tabBar = container.querySelector<HTMLElement>('.ant-tabs-bar')!;
    const event = new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowRight' });
    Object.defineProperty(event, 'keyCode', { value: 39 });
    await act(async () => {
      tabBar.dispatchEvent(event);
    });
    expect(onChange).toHaveBeenLastCalledWith('payment');
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('支付');
  });

  it('keeps old forceRender and destroyInactiveTabPane rendering rules', async () => {
    await act(async () => {
      root.render(
        <LegacyTabs defaultActiveKey="site" destroyInactiveTabPane>
          <LegacyTabs.TabPane tab="站点" key="site">
            站点内容
          </LegacyTabs.TabPane>
          <LegacyTabs.TabPane forceRender tab="安全" key="safe">
            安全内容
          </LegacyTabs.TabPane>
          <LegacyTabs.TabPane placeholder="占位" tab="支付" key="payment">
            支付内容
          </LegacyTabs.TabPane>
        </LegacyTabs>,
      );
    });

    expect(container.querySelectorAll('.ant-tabs-tabpane')[0]!.textContent).toContain('站点内容');
    expect(container.querySelectorAll('.ant-tabs-tabpane')[1]!.textContent).toContain('安全内容');
    expect(container.querySelectorAll('.ant-tabs-tabpane')[2]!.textContent).toContain('占位');
    expect(container.querySelectorAll('.ant-tabs-tabpane')[2]!.textContent).not.toContain(
      '支付内容',
    );

    await act(async () => {
      container.querySelectorAll<HTMLElement>('.ant-tabs-tab')[2]!.click();
    });
    expect(container.querySelectorAll('.ant-tabs-tabpane')[0]!.textContent).not.toContain(
      '站点内容',
    );
    expect(container.querySelectorAll('.ant-tabs-tabpane')[2]!.textContent).toContain('支付内容');
  });

  it('cycles tabs from the tab bar with legacy arrow-key navigation', async () => {
    const onChange = vi.fn();
    await act(async () => {
      root.render(
        <LegacyTabs defaultActiveKey="site" onChange={onChange}>
          <LegacyTabs.TabPane tab="站点" key="site">
            站点内容
          </LegacyTabs.TabPane>
          <LegacyTabs.TabPane tab="安全" key="safe">
            安全内容
          </LegacyTabs.TabPane>
          <LegacyTabs.TabPane tab="支付" key="payment">
            支付内容
          </LegacyTabs.TabPane>
        </LegacyTabs>,
      );
    });

    const tabBar = container.querySelector<HTMLElement>('.ant-tabs-bar')!;
    const press = async (key: string, keyCode: number) => {
      const event = new KeyboardEvent('keydown', { bubbles: true, key });
      Object.defineProperty(event, 'keyCode', { value: keyCode });
      await act(async () => {
        tabBar.dispatchEvent(event);
      });
    };

    await press('ArrowRight', 39);
    expect(onChange).toHaveBeenLastCalledWith('safe');
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('安全');

    await press('ArrowDown', 40);
    expect(onChange).toHaveBeenLastCalledWith('payment');
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('支付');

    await press('ArrowLeft', 37);
    expect(onChange).toHaveBeenLastCalledWith('safe');
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('安全');

    await press('ArrowUp', 38);
    expect(onChange).toHaveBeenLastCalledWith('site');
    expect(container.querySelector('.ant-tabs-tab-active')?.textContent).toBe('站点');
  });

  it('fires old onTabClick separately from active-key changes', async () => {
    const onChange = vi.fn();
    const onTabClick = vi.fn();
    await act(async () => {
      root.render(
        <LegacyTabs defaultActiveKey="site" onChange={onChange} onTabClick={onTabClick}>
          <LegacyTabs.TabPane tab="站点" key="site">
            站点内容
          </LegacyTabs.TabPane>
          <LegacyTabs.TabPane tab="安全" key="safe">
            安全内容
          </LegacyTabs.TabPane>
        </LegacyTabs>,
      );
    });

    await act(async () => {
      container.querySelectorAll<HTMLElement>('.ant-tabs-tab')[0]!.click();
    });
    expect(onTabClick).toHaveBeenLastCalledWith('site');
    expect(onChange).not.toHaveBeenCalled();

    await act(async () => {
      container.querySelectorAll<HTMLElement>('.ant-tabs-tab')[1]!.click();
    });
    expect(onTabClick).toHaveBeenLastCalledWith('safe');
    expect(onChange).toHaveBeenLastCalledWith('safe');
  });
});
