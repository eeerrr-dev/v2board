import {
  useCallback,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type HTMLAttributes,
  type Ref,
  type TableHTMLAttributes,
  type TdHTMLAttributes,
  type ThHTMLAttributes,
} from 'react';
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type RowData,
  type SortDirection,
  type SortingState,
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ArrowDown, ArrowUp, ChevronsUpDown } from 'lucide-react';
import { cn } from '@/lib/cn';

declare module '@tanstack/react-table' {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars -- declaration merging must repeat TanStack's exact generic signature
  interface ColumnMeta<TData extends RowData, TValue> {
    align?: 'left' | 'right' | 'center';
    className?: string;
    headerClassName?: string;
  }
}

type DataAttributes = {
  [key: `data-${string}`]: string | number | boolean | undefined;
};

type TableScrollProps = HTMLAttributes<HTMLDivElement> & DataAttributes;

function TableScroll({
  className,
  ref,
  ...props
}: TableScrollProps & { ref?: Ref<HTMLDivElement> }) {
  return (
    <div
      ref={ref}
      data-slot="table-scroll"
      className={cn('overflow-x-auto', className)}
      {...props}
    />
  );
}

function Table({
  className,
  ref,
  ...props
}: TableHTMLAttributes<HTMLTableElement> & { ref?: Ref<HTMLTableElement> }) {
  return (
    <table ref={ref} data-slot="table" className={cn('w-full text-sm', className)} {...props} />
  );
}

function TableHeader({
  className,
  ref,
  ...props
}: HTMLAttributes<HTMLTableSectionElement> & { ref?: Ref<HTMLTableSectionElement> }) {
  return (
    <thead
      ref={ref}
      data-slot="table-header"
      className={cn('border-b border-border bg-muted/50 text-muted-foreground', className)}
      {...props}
    />
  );
}

function TableBody({
  className,
  ref,
  ...props
}: HTMLAttributes<HTMLTableSectionElement> & { ref?: Ref<HTMLTableSectionElement> }) {
  return (
    <tbody
      ref={ref}
      data-slot="table-body"
      className={cn('divide-y divide-border', className)}
      {...props}
    />
  );
}

function TableRow({
  className,
  ref,
  ...props
}: HTMLAttributes<HTMLTableRowElement> & { ref?: Ref<HTMLTableRowElement> }) {
  return (
    <tr
      ref={ref}
      data-slot="table-row"
      className={cn('transition-colors hover:bg-muted/50', className)}
      {...props}
    />
  );
}

function TableHead({
  className,
  ref,
  ...props
}: ThHTMLAttributes<HTMLTableCellElement> & { ref?: Ref<HTMLTableCellElement> }) {
  return (
    <th
      ref={ref}
      data-slot="table-head"
      className={cn('px-4 py-3 text-left font-medium', className)}
      {...props}
    />
  );
}

function TableCell({
  className,
  ref,
  ...props
}: TdHTMLAttributes<HTMLTableCellElement> & { ref?: Ref<HTMLTableCellElement> }) {
  return <td ref={ref} data-slot="table-cell" className={cn('px-4 py-4', className)} {...props} />;
}

interface TableEmptyProps extends TdHTMLAttributes<HTMLTableCellElement> {
  rowClassName?: string;
}

function TableEmpty({ children, className, colSpan, rowClassName, ...props }: TableEmptyProps) {
  return (
    <TableRow className={rowClassName}>
      <TableCell
        className={cn('py-14 text-center text-sm text-muted-foreground', className)}
        colSpan={colSpan}
        {...props}
      >
        {children}
      </TableCell>
    </TableRow>
  );
}

function ariaSort(sorted: false | SortDirection) {
  return sorted === 'asc' ? 'ascending' : sorted === 'desc' ? 'descending' : 'none';
}

function SortIndicator({ sorted }: { sorted: false | SortDirection }) {
  const Icon = sorted === 'asc' ? ArrowUp : sorted === 'desc' ? ArrowDown : ChevronsUpDown;
  return <Icon className={cn('size-3.5', !sorted && 'opacity-50')} aria-hidden="true" />;
}

type DataTableColumn<TData> = ColumnDef<TData>;

// Row virtualization only pays for itself once the DOM row count is large enough
// that the spacer math + measureElement reflows cost less than rendering every
// row. Below a few hundred rows a plain scroll container is faster and simpler,
// so callers gate `virtualizer.enabled` on this shared threshold instead of
// virtualizing small tables.
const VIRTUALIZE_MIN_ROWS = 150;

interface DataTableVirtualizerOptions {
  enabled?: boolean;
  estimateSize?: number;
  overscan?: number;
}

interface DataTableProps<TData> extends Omit<TableHTMLAttributes<HTMLTableElement>, 'children'> {
  bodyClassName?: string;
  columns: DataTableColumn<TData>[];
  data: TData[];
  empty?: ReactNode;
  emptyClassName?: string;
  emptyTestId?: string;
  getRowKey?: (row: TData, index: number) => string | number;
  headerClassName?: string;
  scrollClassName?: string;
  scrollProps?: TableScrollProps;
  scrollRef?: Ref<HTMLDivElement>;
  virtualizer?: DataTableVirtualizerOptions;
}

function DataTable<TData>({
  bodyClassName,
  className,
  columns,
  data,
  empty,
  emptyClassName,
  emptyTestId,
  getRowKey,
  headerClassName,
  scrollClassName,
  scrollProps,
  scrollRef,
  virtualizer,
  ...props
}: DataTableProps<TData>) {
  const scrollElementRef = useRef<HTMLDivElement | null>(null);
  // Client-side sort. Sorting is opt-in per column: only columns that declare an
  // accessor (accessorKey/accessorFn) report getCanSort(), so display-only columns
  // stay inert and the default ([]) preserves the server's row order.
  const [sorting, setSorting] = useState<SortingState>([]);
  const tableColumns = useMemo(
    () =>
      columns.map((column, index) => ({
        ...column,
        id: column.id ?? `column-${index}`,
      })),
    [columns],
  );
  // TanStack Table returns non-memoizable functions, so the React Compiler skips
  // memoizing this component by design — an accepted tradeoff for a sanctioned dep.
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Table API
  const table = useReactTable({
    data,
    columns: tableColumns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getRowId: getRowKey ? (row, index) => String(getRowKey(row, index)) : undefined,
  });
  const rows = table.getRowModel().rows;
  const columnCount = table.getAllLeafColumns().length;
  const shouldVirtualize = Boolean(virtualizer?.enabled && rows.length > 0 && !empty);
  const getVirtualRowKey = useCallback((index: number) => rows[index]?.id ?? index, [rows]);
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    enabled: shouldVirtualize,
    getItemKey: getVirtualRowKey,
    getScrollElement: () => scrollElementRef.current,
    estimateSize: () => virtualizer?.estimateSize ?? 56,
    overscan: virtualizer?.overscan ?? 8,
  });
  const virtualItems = shouldVirtualize ? rowVirtualizer.getVirtualItems() : [];
  const topPadding = virtualItems[0]?.start ?? 0;
  const bottomPadding = shouldVirtualize
    ? rowVirtualizer.getTotalSize() - (virtualItems[virtualItems.length - 1]?.end ?? 0)
    : 0;
  const visibleRows = shouldVirtualize
    ? virtualItems.flatMap((item) => {
        const row = rows[item.index];
        return row ? [{ item, row }] : [];
      })
    : rows.map((row, index) => ({ item: { index }, row }));
  const setScrollRef = (node: HTMLDivElement | null) => {
    scrollElementRef.current = node;
    if (typeof scrollRef === 'function') scrollRef(node);
    else if (scrollRef) scrollRef.current = node;
  };

  return (
    <TableScroll
      ref={setScrollRef}
      {...scrollProps}
      className={cn(
        shouldVirtualize && 'max-h-[min(42rem,calc(100vh-12rem))] overflow-auto',
        scrollClassName,
        scrollProps?.className,
      )}
    >
      <Table className={className} {...props}>
        <TableHeader className={headerClassName}>
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id}>
              {headerGroup.headers.map((header) => {
                const meta = header.column.columnDef.meta;
                const canSort = header.column.getCanSort();
                const sorted = header.column.getIsSorted();
                const label = header.isPlaceholder
                  ? null
                  : flexRender(header.column.columnDef.header, header.getContext());
                return (
                  <TableHead
                    aria-sort={canSort ? ariaSort(sorted) : undefined}
                    className={cn(
                      meta?.align === 'center' && 'text-center',
                      meta?.align === 'right' && 'text-right',
                      meta?.headerClassName,
                    )}
                    colSpan={header.colSpan}
                    key={header.id}
                  >
                    {canSort ? (
                      <button
                        type="button"
                        data-slot="table-sort"
                        className="inline-flex items-center gap-1.5 rounded-sm transition-colors outline-none select-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
                        onClick={header.column.getToggleSortingHandler()}
                      >
                        {label}
                        <SortIndicator sorted={sorted} />
                      </button>
                    ) : (
                      label
                    )}
                  </TableHead>
                );
              })}
            </tr>
          ))}
        </TableHeader>
        <TableBody className={bodyClassName}>
          {empty ? (
            <TableEmpty className={emptyClassName} colSpan={columnCount} data-testid={emptyTestId}>
              {empty}
            </TableEmpty>
          ) : (
            <>
              {topPadding > 0 ? (
                <TableRow aria-hidden="true" className="hover:bg-transparent">
                  <TableCell colSpan={columnCount} style={{ height: topPadding, padding: 0 }} />
                </TableRow>
              ) : null}
              {visibleRows.map(({ item, row }) => {
                const index = item.index;
                const rowKey = getRowKey ? row.id : index;
                return (
                  <TableRow
                    data-row-key={rowKey}
                    // Let react-virtual measure real row heights (e.g. tag-heavy
                    // node rows that wrap past estimateSize) so the spacer math
                    // and visible window stay aligned instead of drifting.
                    data-index={shouldVirtualize ? index : undefined}
                    key={row.id}
                    ref={shouldVirtualize ? rowVirtualizer.measureElement : undefined}
                  >
                    {row.getVisibleCells().map((cell) => {
                      const meta = cell.column.columnDef.meta;
                      return (
                        <TableCell
                          className={cn(
                            meta?.align === 'center' && 'text-center',
                            meta?.align === 'right' && 'text-right',
                            meta?.className,
                          )}
                          key={cell.id}
                        >
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </TableCell>
                      );
                    })}
                  </TableRow>
                );
              })}
              {bottomPadding > 0 ? (
                <TableRow aria-hidden="true" className="hover:bg-transparent">
                  <TableCell colSpan={columnCount} style={{ height: bottomPadding, padding: 0 }} />
                </TableRow>
              ) : null}
            </>
          )}
        </TableBody>
      </Table>
    </TableScroll>
  );
}

export {
  DataTable,
  VIRTUALIZE_MIN_ROWS,
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
  TableScroll,
};
export type { DataTableColumn, DataTableProps, TableScrollProps };
