import {
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
} from 'lucide-react';
import {
  forwardRef,
  type HTMLAttributes,
  type LiHTMLAttributes,
} from 'react';
import { Button, type ButtonProps } from './button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './select';
import { cn } from '@/lib/cn';

type PaginationItemValue = number | 'jump-prev' | 'jump-next';

interface PaginationLabels {
  itemsPerPage: string;
  nextPage: string;
  nextWindow: string;
  pagination?: string;
  previousPage: string;
  previousWindow: string;
}

interface PaginationControlProps extends Omit<HTMLAttributes<HTMLElement>, 'onChange'> {
  current: number;
  labels: PaginationLabels;
  onChange: (page: number, pageSize: number) => void;
  pageSize: number;
  pageSizeOptions?: number[];
  testIds?: {
    page?: string;
    pageSize?: string;
  };
  total: number;
}

interface PaginationButtonProps extends ButtonProps {
  isActive?: boolean;
}

const Pagination = forwardRef<HTMLElement, HTMLAttributes<HTMLElement>>(
  ({ className, ...props }, ref) => (
    <nav
      ref={ref}
      aria-label="pagination"
      className={cn('mx-auto flex w-full justify-center', className)}
      {...props}
    />
  ),
);
Pagination.displayName = 'Pagination';

const PaginationContent = forwardRef<HTMLUListElement, HTMLAttributes<HTMLUListElement>>(
  ({ className, ...props }, ref) => (
    <ul ref={ref} className={cn('flex flex-row items-center gap-1', className)} {...props} />
  ),
);
PaginationContent.displayName = 'PaginationContent';

const PaginationItem = forwardRef<HTMLLIElement, LiHTMLAttributes<HTMLLIElement>>(
  ({ className, ...props }, ref) => (
    <li ref={ref} className={cn('inline-flex', className)} {...props} />
  ),
);
PaginationItem.displayName = 'PaginationItem';

const PaginationButton = forwardRef<HTMLButtonElement, PaginationButtonProps>(
  ({ className, isActive, variant, ...props }, ref) => {
    const ariaCurrent = isActive ? 'page' : props['aria-current'];
    return (
      <Button
        ref={ref}
        {...props}
        aria-current={ariaCurrent}
        className={className}
        variant={variant ?? (isActive ? 'default' : 'ghost')}
      />
    );
  },
);
PaginationButton.displayName = 'PaginationButton';

function getPaginationPageCount(total: number, pageSize: number) {
  if (pageSize <= 0) return 0;
  return Math.floor((total - 1) / pageSize) + 1;
}

function getPaginationMaxCurrent(total: number, current: number, pageSize: number) {
  const pageCount = getPaginationPageCount(total, pageSize);
  return (current - 1) * pageSize >= total ? pageCount : current;
}

function getPaginationItems(current: number, totalPages: number): PaginationItemValue[] {
  if (totalPages <= 0) return [];
  if (totalPages <= 9) return Array.from({ length: totalPages }, (_, index) => index + 1);

  let left = Math.max(2, current - 2);
  let right = Math.min(totalPages - 1, current + 2);
  if (current - 1 <= 2) right = 5;
  if (totalPages - current <= 2) left = totalPages - 4;

  const items: PaginationItemValue[] = [1];
  if (left > 2) items.push('jump-prev');
  for (let page = left; page <= right; page += 1) items.push(page);
  if (right < totalPages - 1) items.push('jump-next');
  items.push(totalPages);
  return items;
}

function PaginationControl({
  className,
  current,
  labels,
  onChange,
  pageSize,
  pageSizeOptions = [10, 50, 100, 150],
  testIds,
  total,
  ...props
}: PaginationControlProps) {
  const totalPages = getPaginationPageCount(total, pageSize);
  const safeCurrent = totalPages > 0 ? Math.min(Math.max(current, 1), totalPages) : 0;
  const items = getPaginationItems(safeCurrent, totalPages);
  const jumpPage = (item: 'jump-prev' | 'jump-next') =>
    item === 'jump-prev'
      ? Math.max(1, safeCurrent - 5)
      : Math.min(totalPages, safeCurrent + 5);
  const changePage = (targetPage: number) => {
    if (totalPages <= 0) return;
    onChange(Math.min(Math.max(targetPage, 1), totalPages), pageSize);
  };

  return (
    <Pagination
      aria-label={labels.pagination ?? 'pagination'}
      className={cn(
        'flex flex-col gap-3 border-t border-border p-4 sm:flex-row sm:items-center sm:justify-end',
        className,
      )}
      {...props}
    >
      <PaginationContent className="flex-wrap">
        <PaginationItem>
          <PaginationButton
            type="button"
            variant="ghost"
            size="icon"
            aria-label={labels.previousPage}
            disabled={safeCurrent <= 1}
            onClick={() => changePage(safeCurrent - 1)}
          >
            <ChevronLeft className="size-4" />
          </PaginationButton>
        </PaginationItem>
        {items.map((item) => (
          <PaginationItem key={item}>
            {typeof item === 'number' ? (
              <PaginationButton
                type="button"
                size="sm"
                isActive={item === safeCurrent}
                data-page={item}
                data-testid={testIds?.page}
                onClick={() => changePage(item)}
              >
                {item}
              </PaginationButton>
            ) : (
              <PaginationButton
                type="button"
                variant="ghost"
                size="icon"
                aria-label={item === 'jump-prev' ? labels.previousWindow : labels.nextWindow}
                onClick={() => changePage(jumpPage(item))}
              >
                {item === 'jump-prev' ? (
                  <ChevronsLeft className="size-4" />
                ) : (
                  <ChevronsRight className="size-4" />
                )}
              </PaginationButton>
            )}
          </PaginationItem>
        ))}
        <PaginationItem>
          <PaginationButton
            type="button"
            variant="ghost"
            size="icon"
            aria-label={labels.nextPage}
            disabled={safeCurrent >= totalPages}
            onClick={() => changePage(safeCurrent + 1)}
          >
            <ChevronRight className="size-4" />
          </PaginationButton>
        </PaginationItem>
      </PaginationContent>

      <Select
        value={String(pageSize)}
        onValueChange={(value) => {
          const nextPageSize = Number.parseInt(value, 10);
          const nextTotalPages = getPaginationPageCount(total, nextPageSize);
          onChange(nextTotalPages === 0 ? safeCurrent : Math.min(safeCurrent, nextTotalPages), nextPageSize);
        }}
      >
        <SelectTrigger className="h-9 w-full sm:w-36" data-testid={testIds?.pageSize}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent align="end">
          {pageSizeOptions.map((size) => (
            <SelectItem key={size} value={String(size)}>
              {size} {labels.itemsPerPage}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </Pagination>
  );
}

export {
  Pagination,
  PaginationButton,
  PaginationContent,
  PaginationControl,
  PaginationItem,
  getPaginationItems,
  getPaginationMaxCurrent,
  getPaginationPageCount,
};
