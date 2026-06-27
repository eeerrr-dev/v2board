import {
  forwardRef,
  type ReactNode,
  type HTMLAttributes,
  type Ref,
  type TableHTMLAttributes,
  type TdHTMLAttributes,
  type ThHTMLAttributes,
} from 'react';
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

interface DataTableHeader {
  align?: 'left' | 'center' | 'right';
  className?: string;
  content: ReactNode;
}

interface DataTableProps extends Omit<TableHTMLAttributes<HTMLTableElement>, 'children'> {
  bodyClassName?: string;
  children: ReactNode;
  empty?: ReactNode;
  emptyClassName?: string;
  emptyTestId?: string;
  headerClassName?: string;
  headers: DataTableHeader[];
  scrollClassName?: string;
  scrollProps?: TableScrollProps;
  scrollRef?: Ref<HTMLDivElement>;
}

function DataTable({
  bodyClassName,
  children,
  className,
  empty,
  emptyClassName,
  emptyTestId,
  headerClassName,
  headers,
  scrollClassName,
  scrollProps,
  scrollRef,
  ...props
}: DataTableProps) {
  return (
    <TableScroll
      ref={scrollRef}
      {...scrollProps}
      className={cn(scrollClassName, scrollProps?.className)}
    >
      <Table className={className} {...props}>
        <TableHeader className={headerClassName}>
          <tr>
            {headers.map((header, index) => (
              <TableHead
                className={cn(
                  header.align === 'center' && 'text-center',
                  header.align === 'right' && 'text-right',
                  header.className,
                )}
                key={index}
              >
                {header.content}
              </TableHead>
            ))}
          </tr>
        </TableHeader>
        <TableBody className={bodyClassName}>
          {empty ? (
            <TableEmpty
              className={emptyClassName}
              colSpan={headers.length}
              data-testid={emptyTestId}
            >
              {empty}
            </TableEmpty>
          ) : (
            children
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
export type { DataTableHeader, DataTableProps, TableScrollProps };
