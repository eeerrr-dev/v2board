import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  LegacyStandaloneTable,
  LegacyTablePagination,
  type LegacyStandaloneTableHeader,
} from './legacy-standalone-table';

describe('LegacyStandaloneTable', () => {
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
  });
});
