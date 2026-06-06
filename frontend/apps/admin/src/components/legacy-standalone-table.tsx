import type { ReactNode } from 'react';
import { LegacyLeftIcon, LegacyRightIcon } from './legacy-ant-icon';
import { LegacyEmpty } from './legacy-empty';

const LEGACY_ROW_KEY_ATTRIBUTE = `data-${'row-key'}`;
const LEGACY_UNSELECTABLE_ATTRIBUTE = { unselectable: 'unselectable' } as Record<string, string>;

export type LegacyStandaloneTableHeader = {
  title: ReactNode;
  alignCenter?: boolean;
  alignLeft?: boolean;
  alignRight?: boolean;
  className?: string;
  fixedRight?: boolean;
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

function LegacyStandaloneTableHeaderCell({ title }: { title: ReactNode }) {
  return (
    <span className="ant-table-header-column">
      <div>
        <span className="ant-table-column-title">{title}</span>
        <span className="ant-table-column-sorter" />
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
            <LegacyStandaloneTableHeaderCell title={header.title} />
            {header.suffix}
          </th>
        ))}
      </tr>
    </thead>
  );
}

export interface LegacyTablePaginationChange {
  current: number;
  pageSize: number;
  total?: number;
}

export function LegacyTablePagination({
  current,
  pageSize,
  total,
  onChange,
}: {
  current: number;
  pageSize: number;
  total?: number;
  onChange?: (pagination: LegacyTablePaginationChange) => void;
}) {
  const pageCount = Math.max(1, Math.ceil((total ?? 0) / pageSize));
  const currentPage = Math.min(Math.max(current || 1, 1), pageCount);
  const previousDisabled = currentPage <= 1;
  const nextDisabled = currentPage >= pageCount;
  const changePage = (next: number) => {
    const bounded = Math.min(Math.max(next, 1), pageCount);
    if (bounded === currentPage) return;
    onChange?.({ current: bounded, pageSize, total });
  };

  return (
    <ul className="ant-pagination ant-table-pagination mini" {...LEGACY_UNSELECTABLE_ATTRIBUTE}>
      <li
        title="上一页"
        className={`${previousDisabled ? 'ant-pagination-disabled ' : ''}ant-pagination-prev`}
        aria-disabled={previousDisabled}
      >
        <a className="ant-pagination-item-link" onClick={() => changePage(currentPage - 1)}>
          <LegacyLeftIcon />
        </a>
      </li>
      {Array.from({ length: pageCount }, (_, index) => index + 1).map((page) => (
        <li
          key={page}
          title={String(page)}
          className={`ant-pagination-item ant-pagination-item-${page}${
            page === currentPage ? ' ant-pagination-item-active' : ''
          }`}
          tabIndex={0}
          onClick={() => changePage(page)}
        >
          <a>{page}</a>
        </li>
      ))}
      <li
        title="下一页"
        className={`${nextDisabled ? 'ant-pagination-disabled ' : ''}ant-pagination-next`}
        aria-disabled={nextDisabled}
      >
        <a className="ant-pagination-item-link" onClick={() => changePage(currentPage + 1)}>
          <LegacyRightIcon />
        </a>
      </li>
    </ul>
  );
}

function LegacyStandaloneTableBody({
  headers,
  children,
  scrollX,
}: {
  headers: LegacyStandaloneTableHeader[];
  children: ReactNode;
  scrollX?: number;
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
      <table className="ant-table-fixed" style={{ width: scrollX }}>
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
}: {
  headers: LegacyStandaloneTableHeader[];
  children: ReactNode;
  rowHeight?: number;
}) {
  if (headers.length === 0) return null;

  return (
    <div className="ant-table-fixed-right">
      <div className="ant-table-body-outer">
        <div className="ant-table-body-inner">
          <table className="ant-table-fixed">
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
  headers,
  isEmpty,
  children,
  fixedRightChildren,
  fixedRightRowHeight,
  pagination,
  scrollPositionRight = true,
  scrollX,
}: {
  headers: LegacyStandaloneTableHeader[];
  isEmpty: boolean;
  children: ReactNode;
  fixedRightChildren?: ReactNode;
  fixedRightRowHeight?: number;
  pagination?: ReactNode;
  scrollPositionRight?: boolean;
  scrollX?: number;
}) {
  const fixedRightHeaders =
    scrollX === undefined ? [] : headers.filter((header) => header.fixedRight);
  const scrollClassName =
    scrollX === undefined
      ? 'ant-table-scroll-position-left'
      : scrollPositionRight
        ? 'ant-table-scroll-position-left ant-table-scroll-position-right'
        : 'ant-table-scroll-position-left';

  return (
    <div className="ant-table-wrapper">
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
                    <LegacyStandaloneTableBody headers={headers} scrollX={scrollX}>
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
                  >
                    {fixedRightChildren}
                  </LegacyStandaloneTableFixedRight>
                </>
              )}
            </div>
          </div>
          {pagination}
        </div>
      </div>
    </div>
  );
}
