import {
  forwardRef,
  type HTMLAttributes,
  type TableHTMLAttributes,
  type TdHTMLAttributes,
  type ThHTMLAttributes,
} from 'react';
import { cn } from '@/lib/cn';

const TableScroll = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
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

export {
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
  TableScroll,
};
