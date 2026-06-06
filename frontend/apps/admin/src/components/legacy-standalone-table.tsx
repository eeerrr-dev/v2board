import type { ReactNode } from 'react';
import { LegacyEmpty } from './legacy-empty';

const LEGACY_ROW_KEY_ATTRIBUTE = `data-${'row-key'}`;

export type LegacyStandaloneTableHeader = {
  title: ReactNode;
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
}: {
  headers: LegacyStandaloneTableHeader[];
  children: ReactNode;
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
            <LegacyStandaloneTableHead headers={headers} fixedRightTable rowHeight={54} />
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
  scrollPositionRight = true,
  scrollX,
}: {
  headers: LegacyStandaloneTableHeader[];
  isEmpty: boolean;
  children: ReactNode;
  fixedRightChildren?: ReactNode;
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
                  <LegacyStandaloneTableFixedRight headers={fixedRightHeaders}>
                    {fixedRightChildren}
                  </LegacyStandaloneTableFixedRight>
                </>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
