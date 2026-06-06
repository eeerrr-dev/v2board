import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacyTabs } from './legacy-tabs';

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
});
