import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  LegacyStandaloneTable,
  LegacyTablePagination,
  type LegacyStandaloneTableHeader,
} from './legacy-standalone-table';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyStandaloneTable', () => {
  let container: HTMLDivElement | undefined;
  let root: Root | undefined;

  afterEach(() => {
    if (root) {
      act(() => root?.unmount());
      root = undefined;
    }
    container?.remove();
    container = undefined;
    document.body.innerHTML = '';
  });

  it('renders old sortable headers and page-size selector markup', () => {
    const headers: LegacyStandaloneTableHeader[] = [
      {
        title: 'ID',
        className: 'ant-table-column-has-actions ant-table-column-has-sorters',
        sortable: true,
      },
      { title: '操作', alignRight: true, fixedRight: true },
    ];
    const html = renderToStaticMarkup(
      <LegacyStandaloneTable
        className="v2board-table"
        headers={headers}
        isEmpty={false}
        scrollX={1500}
        scrollPositionRight={false}
        fixedRightRowHeight={54}
        pagination={
          <LegacyTablePagination
            current={1}
            pageSize={10}
            total={1}
            pageSizeOptions={[10, 50, 100, 150]}
          />
        }
        fixedRightChildren={
          <tr className="ant-table-row ant-table-row-level-0" style={{ height: 54 }}>
            <td className="" style={{ textAlign: 'right' }}>
              操作
            </td>
          </tr>
        }
      >
        <tr className="ant-table-row ant-table-row-level-0">
          <td className="ant-table-column-has-actions ant-table-column-has-sorters">1</td>
          <td className="ant-table-fixed-columns-in-body" style={{ textAlign: 'right' }}>
            操作
          </td>
        </tr>
      </LegacyStandaloneTable>,
    );

    expect(html).toContain('class="ant-table-wrapper v2board-table"');
    expect(html).toContain('class="ant-table-column-sorters"');
    expect(html).toContain('title="排序"');
    expect(html).toContain('class="anticon anticon-caret-up ant-table-column-sorter-up off"');
    expect(html).toContain('class="anticon anticon-caret-down ant-table-column-sorter-down off"');
    expect(html).toContain('class="ant-table-fixed-right"');
    expect(html).toContain('style="height:54px"');
    expect(html).toContain('ant-pagination-options-size-changer');
    expect(html).toContain('class="anticon anticon-down ant-select-arrow-icon"');
    expect(html).toContain('title="10"');
    expect(html).toContain('>10</div>');
    expect(html).not.toContain('10 条/页');
  });

  it('keeps the initial scroll class on the left so the fixed-right shadow is visible', () => {
    const headers: LegacyStandaloneTableHeader[] = [
      { title: 'ID' },
      { title: '操作', fixedRight: true },
    ];
    const html = renderToStaticMarkup(
      <LegacyStandaloneTable
        headers={headers}
        isEmpty={false}
        scrollX={1200}
        fixedRightChildren={
          <tr className="ant-table-row ant-table-row-level-0">
            <td>操作</td>
          </tr>
        }
      >
        <tr className="ant-table-row ant-table-row-level-0">
          <td>1</td>
          <td className="ant-table-fixed-columns-in-body">操作</td>
        </tr>
      </LegacyStandaloneTable>,
    );

    expect(html).toContain(
      'class="ant-table ant-table-default ant-table-scroll-position-left"',
    );
    expect(html).not.toContain('ant-table-scroll-position-right');
  });

  it('hides the old table pagination while the table is empty', () => {
    const html = renderToStaticMarkup(
      <LegacyStandaloneTable
        headers={[{ title: 'ID' }]}
        isEmpty
        pagination={<LegacyTablePagination current={1} pageSize={10} total={0} />}
      >
        {null}
      </LegacyStandaloneTable>,
    );

    expect(html).toContain('class="ant-table-placeholder"');
    expect(html).not.toContain('ant-table-pagination');
  });

  it('can mirror old desktop-only right-scroll fixed-column shadow state', () => {
    const originalInnerWidth = window.innerWidth;
    const headers: LegacyStandaloneTableHeader[] = [
      { title: 'ID' },
      { title: '操作', fixedRight: true },
    ];
    const render = () =>
      renderToStaticMarkup(
        <LegacyStandaloneTable
          headers={headers}
          isEmpty={false}
          scrollX={1200}
          scrollPositionRight="desktop"
          fixedRightChildren={
            <tr className="ant-table-row ant-table-row-level-0">
              <td>操作</td>
            </tr>
          }
        >
          <tr className="ant-table-row ant-table-row-level-0">
            <td>1</td>
            <td className="ant-table-fixed-columns-in-body">操作</td>
          </tr>
        </LegacyStandaloneTable>,
      );

    try {
      Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 });
      expect(render()).toContain(
        'class="ant-table ant-table-default ant-table-scroll-position-left ant-table-scroll-position-right"',
      );

      Object.defineProperty(window, 'innerWidth', { configurable: true, value: 390 });
      const mobileHtml = render();
      expect(mobileHtml).toContain(
        'class="ant-table ant-table-default ant-table-scroll-position-left"',
      );
      expect(mobileHtml).not.toContain('ant-table-scroll-position-right');
    } finally {
      Object.defineProperty(window, 'innerWidth', {
        configurable: true,
        value: originalInnerWidth,
      });
    }
  });

  it('folds large page ranges with the old rc-pagination jump nodes', () => {
    const html = renderToStaticMarkup(
      <LegacyTablePagination current={6} pageSize={10} total={200} />,
    );

    expect(html).toContain('class="ant-pagination-jump-prev"');
    expect(html).toContain('title="向前 5 页"');
    expect(html).toContain('class="ant-pagination-item-container"');
    expect(html).toContain('class="anticon anticon-double-left ant-pagination-item-link-icon"');
    expect(html).toContain('class="anticon anticon-double-right ant-pagination-item-link-icon"');
    expect(html).toContain('class="ant-pagination-item-ellipsis">•••</span>');
    expect(html).toContain('class="ant-pagination-item ant-pagination-item-4');
    expect(html).toContain('ant-pagination-item-after-jump-prev');
    expect(html).toContain('class="ant-pagination-item ant-pagination-item-8');
    expect(html).toContain('ant-pagination-item-before-jump-next');
    expect(html).toContain('class="ant-pagination-jump-next"');
    expect(html).toContain('title="向后 5 页"');
    expect(html).toContain('class="ant-pagination-item ant-pagination-item-20"');
    expect(html).not.toContain('ant-pagination-item-2"');
    expect(html).not.toContain('ant-pagination-item-9"');
  });

  it('uses the full legacy page-size label on non-mini pagination', () => {
    const html = renderToStaticMarkup(
      <LegacyTablePagination
        current={1}
        mini={false}
        pageSize={10}
        pageSizeOptions={[10, 50, 100, 500]}
        total={1}
      />,
    );

    expect(html).not.toContain('class="ant-pagination ant-table-pagination mini"');
    expect(html).toContain('title="10 条/页"');
    expect(html).toContain('>10 条/页</div>');
    expect(html).not.toContain('ant-select-sm');
  });

  it('emits old pagination changes for folded jumpers and page-size changes', async () => {
    const onChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    await act(async () => {
      root?.render(
        <LegacyTablePagination
          current={20}
          pageSize={10}
          total={200}
          pageSizeOptions={[10, 50, 100, 150]}
          onChange={onChange}
        />,
      );
      await Promise.resolve();
    });

    await act(async () => {
      container
        ?.querySelector('.ant-pagination-jump-prev')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenLastCalledWith({
      current: 15,
      pageSize: 10,
      pageSizeOptions: [10, 50, 100, 150],
      showSizeChanger: true,
      size: 'small',
      total: 200,
    });

    await act(async () => {
      container
        ?.querySelector('.ant-pagination-options-size-changer .ant-select-selection')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
    });

    expect(
      container?.querySelector('.ant-pagination-options-size-changer .ant-select-selection-selected-value')
        ?.textContent,
    ).toBe('10');

    const pageSizeOption = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown-menu-item'),
    ).find((item) => item.textContent === '50 条/页');
    expect(pageSizeOption).toBeTruthy();

    await act(async () => {
      pageSizeOption?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenLastCalledWith({
      current: 4,
      pageSize: 50,
      pageSizeOptions: [10, 50, 100, 150],
      showSizeChanger: true,
      size: 'small',
      total: 200,
    });
  });

  it('handles old prev and next li click hit areas', async () => {
    const onChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    await act(async () => {
      root?.render(
        <LegacyTablePagination current={2} pageSize={10} total={30} onChange={onChange} />,
      );
      await Promise.resolve();
    });

    const previous = container.querySelector<HTMLElement>('.ant-pagination-prev')!;
    const next = container.querySelector<HTMLElement>('.ant-pagination-next')!;
    expect(previous.tabIndex).toBe(0);
    expect(next.tabIndex).toBe(0);

    await act(async () => {
      previous.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenLastCalledWith({
      current: 1,
      pageSize: 10,
      total: 30,
    });

    await act(async () => {
      next.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenLastCalledWith({
      current: 3,
      pageSize: 10,
      total: 30,
    });
  });
});
