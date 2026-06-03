import { useCallback, useLayoutEffect, useRef } from 'react';

// Mirrors antd v3 Table.syncFixedTableRowHeight: a table with a fixed column renders the
// fixed column in its own table, so antd measures every row of the main (scrolling) table
// and stamps that height onto the matching fixed-table row to keep the two aligned. The
// main table keeps its natural height; only the fixed table's <tr>s get an inline height.
// Rows correspond one-to-one by document order (thead row first, then each body row).
// antd v3 calls this after every update for fixed-column tables, not just when the
// data length changes, because content can wrap differently while the row count stays the same.
export function useFixedColumnRowHeights(_rowCount: number) {
  const mainTableRef = useRef<HTMLTableElement | null>(null);
  const fixedTableRef = useRef<HTMLTableElement | null>(null);

  const sync = useCallback(() => {
    const main = mainTableRef.current;
    const fixed = fixedTableRef.current;
    if (!main || !fixed) return;
    const mainRows = main.querySelectorAll('tr');
    const fixedRows = fixed.querySelectorAll('tr');
    fixedRows.forEach((row, index) => {
      const height = mainRows[index]?.getBoundingClientRect().height;
      row.style.height = height ? `${height}px` : 'auto';
    });
  }, []);

  useLayoutEffect(() => {
    sync();
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
