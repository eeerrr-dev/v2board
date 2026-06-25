import { useCallback, useLayoutEffect, useRef } from 'react';

function measuredHeight(element: Element | null): number | null {
  if (!element) return null;
  const height = element.getBoundingClientRect().height;
  return height > 0 ? height : null;
}

function writeHeight(row: HTMLElement, height: number | null): void {
  row.style.height = height ? `${height}px` : 'auto';
}

function collectBodyRowsByKey(rows: Element[]): Map<string, number> {
  const heights = new Map<string, number>();
  for (const row of rows) {
    const key = row.getAttribute('data-row-key');
    const height = measuredHeight(row);
    if (key !== null && height !== null) {
      heights.set(key, height);
    }
  }
  return heights;
}

export function syncFixedColumnRowHeights(
  mainTable: HTMLTableElement,
  fixedTable: HTMLTableElement,
): void {
  const tableNode = mainTable.closest('.ant-table') ?? mainTable;
  const tableHeight = tableNode.getBoundingClientRect().height;
  if (tableHeight <= 0) return;

  const fixedHeaderRows = Array.from(
    fixedTable.querySelectorAll<HTMLElement>('thead > tr'),
  );
  const mainHeadHeight = measuredHeight(mainTable.querySelector('thead'));
  const headerHeight =
    mainHeadHeight && fixedHeaderRows.length > 1
      ? mainHeadHeight / fixedHeaderRows.length
      : mainHeadHeight;
  for (const row of fixedHeaderRows) {
    writeHeight(row, headerHeight);
  }

  const mainBodyRows = Array.from(mainTable.querySelectorAll('tbody .ant-table-row'));
  const mainRowsByKey = collectBodyRowsByKey(mainBodyRows);
  const fixedBodyRows = Array.from(
    fixedTable.querySelectorAll<HTMLElement>('tbody .ant-table-row'),
  );

  fixedBodyRows.forEach((row, index) => {
    const key = row.getAttribute('data-row-key');
    const mainHeight =
      key !== null ? mainRowsByKey.get(key) ?? null : measuredHeight(mainBodyRows[index] ?? null);
    writeHeight(row, mainHeight);
  });
}

// Mirrors antd v3 Table.syncFixedTableRowHeight for restored legacy tables.
export function useFixedColumnRowHeights(_rowCount: number) {
  const mainTableRef = useRef<HTMLTableElement | null>(null);
  const fixedTableRef = useRef<HTMLTableElement | null>(null);

  const sync = useCallback(() => {
    const main = mainTableRef.current;
    const fixed = fixedTableRef.current;
    if (!main || !fixed) return;
    syncFixedColumnRowHeights(main, fixed);
  }, []);

  useLayoutEffect(() => {
    sync();
    const frame = window.requestAnimationFrame(sync);
    let cancelled = false;
    const fontsReady = document.fonts?.ready;
    void fontsReady?.then(() => {
      if (!cancelled) sync();
    });
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(frame);
    };
  });

  useLayoutEffect(() => {
    let timer: number | undefined;
    const onResize = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(sync, 150);
    };
    window.addEventListener('resize', onResize);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener('resize', onResize);
    };
  }, [sync]);

  return { mainTableRef, fixedTableRef };
}
