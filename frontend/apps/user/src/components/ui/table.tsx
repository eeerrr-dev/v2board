import {
  forwardRef,
  useRef,
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
  useReactTable,
  type ColumnDef,
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import { cn } from '@/lib/cn';

type DataAttributes = {
  [key: `data-${string}`]: string | number | boolean | undefined;
};

type TableScrollProps = HTMLAttributes<HTMLDivElement> & DataAttributes;

const TableScroll = forwardRef<HTMLDivElement, TableScrollProps>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn('overflow-x-auto', className)} {...props} />
  ),
);
TableScroll.displayName = 'TableScroll';

const Table = forwardRef<HTMLTableElement, TableHTMLAttributes<HTMLTableElement>>(
  ({ className, ...props }, ref) => (
    <table ref={ref} className={cn('w-full text-sm', className)} {...props} />
  ),
);
Table.displayName = 'Table';

const TableHeader = forwardRef<HTMLTableSectionElement, HTMLAttributes<HTMLTableSectionElement>>(
  ({ className, ...props }, ref) => (
    <thead
      ref={ref}
      className={cn('border-b border-border bg-muted/50 text-muted-foreground', className)}
      {...props}
    />
  ),
);
TableHeader.displayName = 'TableHeader';

const TableBody = forwardRef<HTMLTableSectionElement, HTMLAttributes<HTMLTableSectionElement>>(
  ({ className, ...props }, ref) => (
    <tbody ref={ref} className={cn('divide-y divide-border', className)} {...props} />
  ),
);
TableBody.displayName = 'TableBody';

const TableRow = forwardRef<HTMLTableRowElement, HTMLAttributes<HTMLTableRowElement>>(
  ({ className, ...props }, ref) => (
    <tr ref={ref} className={cn('transition-colors hover:bg-muted/50', className)} {...props} />
  ),
);
TableRow.displayName = 'TableRow';

const TableHead = forwardRef<HTMLTableCellElement, ThHTMLAttributes<HTMLTableCellElement>>(
  ({ className, ...props }, ref) => (
    <th ref={ref} className={cn('px-4 py-3 text-left font-medium', className)} {...props} />
  ),
);
TableHead.displayName = 'TableHead';

const TableCell = forwardRef<HTMLTableCellElement, TdHTMLAttributes<HTMLTableCellElement>>(
  ({ className, ...props }, ref) => (
    <td ref={ref} className={cn('px-4 py-4', className)} {...props} />
  ),
);
TableCell.displayName = 'TableCell';

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
TableEmpty.displayName = 'TableEmpty';

interface DataTableColumnMeta {
  align?: 'left' | 'center' | 'right';
  className?: string;
  headerClassName?: string;
}

type DataTableColumn<TData> = ColumnDef<TData> & DataTableColumnMeta;

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
  const tableColumns = columns.map((column, index) => ({
    ...column,
    id: column.id ?? `column-${index}`,
  }));
  const table = useReactTable({
    data,
    columns: tableColumns,
    getCoreRowModel: getCoreRowModel(),
  });
  const rows = table.getRowModel().rows;
  const columnCount = table.getAllLeafColumns().length;
  const shouldVirtualize = Boolean(virtualizer?.enabled && rows.length > 0 && !empty);
  const rowVirtualizer = useVirtualizer({
    count: shouldVirtualize ? rows.length : 0,
    getScrollElement: () => scrollElementRef.current,
    estimateSize: () => virtualizer?.estimateSize ?? 56,
    overscan: virtualizer?.overscan ?? 8,
  });
  const virtualItems = shouldVirtualize ? rowVirtualizer.getVirtualItems() : [];
  const topPadding = virtualItems[0]?.start ?? 0;
  const bottomPadding = shouldVirtualize
    ? rowVirtualizer.getTotalSize() -
      (virtualItems[virtualItems.length - 1]?.end ?? 0)
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
    else if (scrollRef) {
      (scrollRef as { current: HTMLDivElement | null }).current = node;
    }
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
                const column = header.column.columnDef as DataTableColumn<TData>;
                return (
                  <TableHead
                    className={cn(
                      column.align === 'center' && 'text-center',
                      column.align === 'right' && 'text-right',
                      column.headerClassName,
                    )}
                    colSpan={header.colSpan}
                    key={header.id}
                  >
                    {header.isPlaceholder
                      ? null
                      : flexRender(header.column.columnDef.header, header.getContext())}
                  </TableHead>
                );
              })}
            </tr>
          ))}
        </TableHeader>
        <TableBody className={bodyClassName}>
          {empty ? (
            <TableEmpty
              className={emptyClassName}
              colSpan={columnCount}
              data-testid={emptyTestId}
            >
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
                const rowKey = getRowKey?.(row.original, index) ?? index;
                return (
                  <TableRow data-row-key={rowKey} key={row.id}>
                    {row.getVisibleCells().map((cell) => {
                      const column = cell.column.columnDef as DataTableColumn<TData>;
                      return (
                        <TableCell
                          className={cn(
                            column.align === 'center' && 'text-center',
                            column.align === 'right' && 'text-right',
                            column.className,
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
DataTable.displayName = 'DataTable';

export {
  DataTable,
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
