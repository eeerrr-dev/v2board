import type { KeyboardEvent, ReactNode, Ref } from 'react';
import {
  LegacyCaretDownIcon,
  LegacyCaretUpIcon,
  LegacyDoubleLeftIcon,
  LegacyDoubleRightIcon,
  LegacyLeftIcon,
  LegacyRightIcon,
} from './legacy-ant-icon';
import { LegacyEmpty } from './legacy-empty';
import { LegacySelect, type LegacySelectOption, type LegacySelectValue } from './legacy-select';
import { useFixedColumnRowHeights } from '@/lib/use-fixed-column-row-heights';

const LEGACY_ROW_KEY_ATTRIBUTE = `data-${'row-key'}`;
const LEGACY_UNSELECTABLE_ATTRIBUTE = { unselectable: 'unselectable' } as Record<string, string>;

export type LegacyStandaloneTableHeader = {
  title: ReactNode;
  alignCenter?: boolean;
  alignLeft?: boolean;
  alignRight?: boolean;
  className?: string;
  fixedRight?: boolean;
  onClick?: () => void;
  sortable?: boolean;
  sortOrder?: 'ASC' | 'DESC';
  suffix?: ReactNode;
};

export function legacyTableRowKey(value: number) {
  return { [LEGACY_ROW_KEY_ATTRIBUTE]: value };
}

function legacyHeaderClassName(
  header: LegacyStandaloneTableHeader,
  index: number,
  count: number,
  fixedRightTable = false,
) {
  const classes = [
    header.className,
    header.fixedRight && !fixedRightTable ? 'ant-table-fixed-columns-in-body' : undefined,
    header.alignCenter ? 'ant-table-align-center' : undefined,
    header.alignLeft ? 'ant-table-align-left' : undefined,
    header.alignRight ? 'ant-table-align-right' : undefined,
    index === count - 1 ? 'ant-table-row-cell-last' : undefined,
  ].filter(Boolean);
  return classes.join(' ');
}

function LegacyStandaloneTableHeaderCell({
  sortOrder,
  sortable,
  title,
}: {
  sortOrder?: 'ASC' | 'DESC';
  sortable?: boolean;
  title: ReactNode;
}) {
  return (
    <span className="ant-table-header-column">
      <div className={sortable ? 'ant-table-column-sorters' : undefined}>
        <span className="ant-table-column-title">{title}</span>
        {sortable ? (
          <span className="ant-table-column-sorter">
            <div
              title="排序"
              className="ant-table-column-sorter-inner ant-table-column-sorter-inner-full"
            >
              <LegacyCaretUpIcon
                className={`ant-table-column-sorter-up ${sortOrder === 'ASC' ? 'on' : 'off'}`}
              />
              <LegacyCaretDownIcon
                className={`ant-table-column-sorter-down ${sortOrder === 'DESC' ? 'on' : 'off'}`}
              />
            </div>
          </span>
        ) : (
          <span className="ant-table-column-sorter" />
        )}
      </div>
    </span>
  );
}

function LegacyStandaloneTableHead({
  headers,
  fixedRightTable,
  rowHeight,
}: {
  headers: LegacyStandaloneTableHeader[];
  fixedRightTable?: boolean;
  rowHeight?: number;
}) {
  return (
    <thead className="ant-table-thead">
      <tr style={rowHeight === undefined ? undefined : { height: rowHeight }}>
        {headers.map((header, index) => (
          <th
            key={index}
            className={legacyHeaderClassName(header, index, headers.length, fixedRightTable)}
            onClick={header.onClick}
            style={
              header.alignRight
                ? { textAlign: 'right' }
                : header.alignLeft
                  ? { textAlign: 'left' }
                  : header.alignCenter
                    ? { textAlign: 'center' }
                    : undefined
            }
          >
            <LegacyStandaloneTableHeaderCell
              sortOrder={header.sortOrder}
              sortable={header.sortable}
              title={header.title}
            />
            {header.suffix}
          </th>
        ))}
      </tr>
    </thead>
  );
}

export interface LegacyTablePaginationChange {
  current: number;
  pageSizeOptions?: number[];
  pageSize: number;
  showSizeChanger?: boolean;
  size?: 'small';
  total?: number;
}

type LegacyScrollPositionRight = boolean | 'desktop';

const LEGACY_PAGER_RANGE = 2;
const LEGACY_PAGE_SIZE_SUFFIX = '条/页';

type LegacyPaginationItem =
  | { page: number; className?: string; type: 'page' }
  | { page: number; title: string; type: 'jump-prev' | 'jump-next' };

function getLegacyPageCount(total: number | undefined, pageSize: number) {
  return Math.max(1, Math.ceil((total ?? 0) / pageSize));
}

function getLegacyPageItems(currentPage: number, pageCount: number): LegacyPaginationItem[] {
  if (pageCount <= 5 + 2 * LEGACY_PAGER_RANGE) {
    return Array.from({ length: pageCount }, (_, index) => ({
      page: index + 1,
      type: 'page' as const,
    }));
  }

  const items: LegacyPaginationItem[] = [];
  let start = Math.max(1, currentPage - LEGACY_PAGER_RANGE);
  let end = Math.min(currentPage + LEGACY_PAGER_RANGE, pageCount);

  if (currentPage - 1 <= LEGACY_PAGER_RANGE) end = 1 + 2 * LEGACY_PAGER_RANGE;
  if (pageCount - currentPage <= LEGACY_PAGER_RANGE) start = pageCount - 2 * LEGACY_PAGER_RANGE;

  for (let page = start; page <= end; page += 1) {
    items.push({ page, type: 'page' });
  }

  if (currentPage - 1 >= 2 * LEGACY_PAGER_RANGE && currentPage !== 3) {
    const first = items[0];
    if (first?.type === 'page') first.className = 'ant-pagination-item-after-jump-prev';
    items.unshift({
      page: Math.max(1, currentPage - 5),
      title: '向前 5 页',
      type: 'jump-prev',
    });
  }

  if (pageCount - currentPage >= 2 * LEGACY_PAGER_RANGE && currentPage !== pageCount - 2) {
    const last = items[items.length - 1];
    if (last?.type === 'page') last.className = 'ant-pagination-item-before-jump-next';
    items.push({
      page: Math.min(pageCount, currentPage + 5),
      title: '向后 5 页',
      type: 'jump-next',
    });
  }

  if (start !== 1) items.unshift({ page: 1, type: 'page' });
  if (end !== pageCount) items.push({ page: pageCount, type: 'page' });

  return items;
}

function getLegacyPageSizeOptions(
  pageSizeOptions: number[] | undefined,
  mini: boolean,
): LegacySelectOption[] {
  return (pageSizeOptions ?? []).map((option) => ({
    value: option,
    label: `${option} ${LEGACY_PAGE_SIZE_SUFFIX}`,
    selectedLabel: mini ? String(option) : undefined,
    selectedTitle: mini ? String(option) : undefined,
  }));
}

function runLegacyPaginationEnter(event: KeyboardEvent<HTMLElement>, action: () => void) {
  if (event.key !== 'Enter' && event.charCode !== 13) return;
  action();
}

function legacyPaginationChange(
  current: number,
  pageSize: number,
  total: number | undefined,
  pageSizeOptions: number[] | undefined,
  mini: boolean,
): LegacyTablePaginationChange {
  return {
    current,
    pageSize,
    ...(mini ? { size: 'small' as const } : {}),
    ...(pageSizeOptions
      ? { pageSizeOptions, showSizeChanger: true }
      : {}),
    total,
  };
}

function LegacyPaginationJumpIcon({ direction }: { direction: 'prev' | 'next' }) {
  const Icon = direction === 'prev' ? LegacyDoubleLeftIcon : LegacyDoubleRightIcon;
  return (
    <a className="ant-pagination-item-link">
      <div className="ant-pagination-item-container">
        <Icon className="ant-pagination-item-link-icon" />
        <span className="ant-pagination-item-ellipsis">•••</span>
      </div>
    </a>
  );
}

export function LegacyTablePagination({
  current,
  mini = true,
  pageSizeOptions,
  pageSize,
  total,
  onChange,
}: {
  current: number;
  mini?: boolean;
  pageSizeOptions?: number[];
  pageSize: number;
  total?: number;
  onChange?: (pagination: LegacyTablePaginationChange) => void;
}) {
  const pageCount = getLegacyPageCount(total, pageSize);
  const currentPage = Math.min(Math.max(current || 1, 1), pageCount);
  const previousDisabled = currentPage <= 1;
  const nextDisabled = currentPage >= pageCount;
  const pageItems = getLegacyPageItems(currentPage, pageCount);
  const sizeOptions = getLegacyPageSizeOptions(pageSizeOptions, mini);
  const changePage = (next: number) => {
    const bounded = Math.min(Math.max(next, 1), pageCount);
    if (bounded === currentPage) return;
    onChange?.(legacyPaginationChange(bounded, pageSize, total, pageSizeOptions, mini));
  };
  const changePageSize = (next: LegacySelectValue) => {
    const nextPageSize = Number(next);
    if (!Number.isFinite(nextPageSize) || nextPageSize <= 0 || nextPageSize === pageSize) return;
    const nextPageCount = getLegacyPageCount(total, nextPageSize);
    onChange?.(
      legacyPaginationChange(
        Math.min(currentPage, nextPageCount),
        nextPageSize,
        total,
        pageSizeOptions,
        mini,
      ),
    );
  };

  return (
    <ul
      className={['ant-pagination', 'ant-table-pagination', mini ? 'mini' : undefined]
        .filter(Boolean)
        .join(' ')}
      {...LEGACY_UNSELECTABLE_ATTRIBUTE}
    >
      <li
        title="上一页"
        className={`${previousDisabled ? 'ant-pagination-disabled ' : ''}ant-pagination-prev`}
        aria-disabled={previousDisabled}
        tabIndex={previousDisabled ? undefined : 0}
        onClick={() => changePage(currentPage - 1)}
        onKeyPress={(event) => runLegacyPaginationEnter(event, () => changePage(currentPage - 1))}
      >
        <a className="ant-pagination-item-link">
          <LegacyLeftIcon />
        </a>
      </li>
      {pageItems.map((item) =>
        item.type === 'page' ? (
          <li
            key={item.page}
            title={String(item.page)}
            className={`ant-pagination-item ant-pagination-item-${item.page}${
              item.page === currentPage ? ' ant-pagination-item-active' : ''
            }${item.className ? ` ${item.className}` : ''}`}
            tabIndex={0}
            onClick={() => changePage(item.page)}
            onKeyPress={(event) => runLegacyPaginationEnter(event, () => changePage(item.page))}
          >
            <a>{item.page}</a>
          </li>
        ) : (
          <li
            key={item.type}
            title={item.title}
            className={`ant-pagination-${item.type}`}
            tabIndex={0}
            onClick={() => changePage(item.page)}
            onKeyPress={(event) => runLegacyPaginationEnter(event, () => changePage(item.page))}
          >
            <LegacyPaginationJumpIcon direction={item.type === 'jump-prev' ? 'prev' : 'next'} />
          </li>
        ),
      )}
      <li
        title="下一页"
        className={`${nextDisabled ? 'ant-pagination-disabled ' : ''}ant-pagination-next`}
        aria-disabled={nextDisabled}
        tabIndex={nextDisabled ? undefined : 0}
        onClick={() => changePage(currentPage + 1)}
        onKeyPress={(event) => runLegacyPaginationEnter(event, () => changePage(currentPage + 1))}
      >
        <a className="ant-pagination-item-link">
          <LegacyRightIcon />
        </a>
      </li>
      {pageSizeOptions ? (
        <li className="ant-pagination-options">
          <LegacySelect
            size={mini ? 'small' : undefined}
            className="ant-pagination-options-size-changer"
            dropdownMatchSelectWidth={false}
            getPopupContainer={(trigger) => trigger.parentElement}
            value={pageSize}
            options={sizeOptions}
            onChange={changePageSize}
          />
        </li>
      ) : null}
    </ul>
  );
}

function LegacyStandaloneTableBody({
  headers,
  children,
  scrollX,
  tableRef,
}: {
  headers: LegacyStandaloneTableHeader[];
  children: ReactNode;
  scrollX?: number;
  tableRef?: Ref<HTMLTableElement>;
}) {
  if (scrollX === undefined) {
    return (
      <div className="ant-table-body">
        <table className="">
          <colgroup>
            {headers.map((_, index) => (
              <col key={index} />
            ))}
          </colgroup>
          <LegacyStandaloneTableHead headers={headers} />
          <tbody className="ant-table-tbody">{children}</tbody>
        </table>
      </div>
    );
  }

  return (
    <div tabIndex={-1} className="ant-table-body" style={{ overflowX: 'scroll' }}>
      <table ref={tableRef} className="ant-table-fixed" style={{ width: scrollX }}>
        <colgroup>
          {headers.map((_, index) => (
            <col key={index} />
          ))}
        </colgroup>
        <LegacyStandaloneTableHead headers={headers} />
        <tbody className="ant-table-tbody">{children}</tbody>
      </table>
    </div>
  );
}

function LegacyStandaloneTableFixedRight({
  headers,
  children,
  rowHeight = 54,
  tableRef,
}: {
  headers: LegacyStandaloneTableHeader[];
  children: ReactNode;
  rowHeight?: number;
  tableRef?: Ref<HTMLTableElement>;
}) {
  if (headers.length === 0) return null;

  return (
    <div className="ant-table-fixed-right">
      <div className="ant-table-body-outer">
        <div className="ant-table-body-inner">
          <table ref={tableRef} className="ant-table-fixed">
            <colgroup>
              {headers.map((_, index) => (
                <col key={index} />
              ))}
            </colgroup>
            <LegacyStandaloneTableHead headers={headers} fixedRightTable rowHeight={rowHeight} />
            <tbody className="ant-table-tbody">{children}</tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

export function LegacyStandaloneTable({
  className,
  headers,
  isEmpty,
  children,
  fixedRightChildren,
  fixedRightRowHeight,
  pagination,
  scrollPositionRight = false,
  scrollX,
}: {
  className?: string;
  headers: LegacyStandaloneTableHeader[];
  isEmpty: boolean;
  children: ReactNode;
  fixedRightChildren?: ReactNode;
  fixedRightRowHeight?: number;
  pagination?: ReactNode;
  scrollPositionRight?: LegacyScrollPositionRight;
  scrollX?: number;
}) {
  const fixedRightHeaders =
    scrollX === undefined ? [] : headers.filter((header) => header.fixedRight);
  const fixedRightRowCount = Array.isArray(fixedRightChildren)
    ? fixedRightChildren.length
    : fixedRightChildren
      ? 1
      : 0;
  const fixedColumnHeights = useFixedColumnRowHeights(fixedRightRowCount);
  const resolvedScrollPositionRight =
    scrollPositionRight === 'desktop'
      ? typeof window === 'undefined' || window.innerWidth >= 768
      : scrollPositionRight;
  const scrollClassName =
    scrollX === undefined
      ? 'ant-table-scroll-position-left'
      : resolvedScrollPositionRight
        ? 'ant-table-scroll-position-left ant-table-scroll-position-right'
        : 'ant-table-scroll-position-left';

  return (
    <div className={['ant-table-wrapper', className].filter(Boolean).join(' ')}>
      <div className="ant-spin-nested-loading">
        <div className="ant-spin-container">
          <div
            className={`ant-table ant-table-default${isEmpty ? ' ant-table-empty' : ''} ${scrollClassName}`}
          >
            <div className="ant-table-content">
              {scrollX === undefined ? (
                <>
                  <LegacyStandaloneTableBody headers={headers} scrollX={scrollX}>
                    {children}
                  </LegacyStandaloneTableBody>
                  {isEmpty ? (
                    <div className="ant-table-placeholder">
                      <LegacyEmpty />
                    </div>
                  ) : null}
                </>
              ) : (
                <>
                  <div className="ant-table-scroll">
                    <LegacyStandaloneTableBody
                      headers={headers}
                      scrollX={scrollX}
                      tableRef={fixedColumnHeights.mainTableRef}
                    >
                      {children}
                    </LegacyStandaloneTableBody>
                    {isEmpty ? (
                      <div className="ant-table-placeholder">
                        <LegacyEmpty />
                      </div>
                    ) : null}
                  </div>
                  <LegacyStandaloneTableFixedRight
                    headers={fixedRightHeaders}
                    rowHeight={fixedRightRowHeight}
                    tableRef={fixedColumnHeights.fixedTableRef}
                  >
                    {fixedRightChildren}
                  </LegacyStandaloneTableFixedRight>
                </>
              )}
            </div>
          </div>
          {isEmpty ? null : pagination}
        </div>
      </div>
    </div>
  );
}
