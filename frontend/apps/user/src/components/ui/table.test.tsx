import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  DataTable,
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
  TableScroll,
} from './table';
import { readFileSync } from 'node:fs';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const source = readFileSync(`${process.cwd()}/src/components/ui/table.tsx`, 'utf8');

describe('Table', () => {
  it('renders shadcn-style table primitives with local hooks preserved', () => {
    const html = renderToStaticMarkup(
      <TableScroll className="v2board-table-scroll">
        <Table className="v2board-table min-w-[640px]">
          <TableHeader className="border-y">
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead className="text-right">Amount</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow data-row-key={0}>
              <TableCell>Alpha</TableCell>
              <TableCell className="text-right">12.00</TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </TableScroll>,
    );

    expect(html).toContain('overflow-x-auto');
    expect(html).toContain('v2board-table-scroll');
    expect(html).toContain('v2board-table');
    expect(html).toContain('bg-muted/50');
    expect(html).toContain('divide-y');
    expect(html).toContain('data-row-key="0"');
    expect(html).toContain('text-right');
  });

  it('renders a reusable empty row without an antd empty shell', () => {
    const html = renderToStaticMarkup(
      <Table>
        <TableBody>
          <TableEmpty colSpan={3} rowClassName="v2board-empty">
            Empty
          </TableEmpty>
        </TableBody>
      </Table>,
    );

    expect(html).toContain('v2board-empty');
    expect(html).toContain('colSpan="3"');
    expect(html).toContain('Empty');
    expect(html).not.toContain('ant-empty');
  });

  it('renders DataTable through TanStack row and column models', () => {
    const html = renderToStaticMarkup(
      <DataTable
        columns={[
          { header: 'Name', cell: ({ row }) => row.original.name },
          {
            header: 'Amount',
            cell: ({ row }) => row.original.amount.toFixed(2),
            meta: { align: 'right' },
          },
        ]}
        data={[{ name: 'Alpha', amount: 12 }]}
        getRowKey={(row) => row.name}
      />,
    );

    expect(html).toContain('Name');
    expect(html).toContain('Alpha');
    expect(html).toContain('12.00');
    expect(html).toContain('data-row-key="Alpha"');
    expect(source).toContain("from '@tanstack/react-table'");
    expect(source).toContain("from '@tanstack/react-virtual'");
    expect(source).toContain('useReactTable');
    expect(source).toContain('getRowId: getRowKey');
    expect(source).toContain('useVirtualizer');
  });
});

describe('DataTable sorting', () => {
  it('makes accessor columns sortable and leaves display-only columns inert', () => {
    const html = renderToStaticMarkup(
      <DataTable
        columns={[
          { accessorKey: 'name', header: 'Name', cell: ({ row }) => row.original.name },
          { header: 'Action', cell: () => 'x' },
        ]}
        data={[{ name: 'Beta' }, { name: 'Alpha' }]}
        getRowKey={(row) => row.name}
      />,
    );

    // The accessor column exposes a sort toggle and an aria-sort affordance...
    expect(html).toContain('data-slot="table-sort"');
    expect(html).toContain('aria-sort="none"');
    // ...while the display-only "Action" header stays a plain, non-interactive cell.
    expect(html.match(/data-slot="table-sort"/g)).toHaveLength(1);
    expect(source).toContain('getSortedRowModel');
  });

  it('reorders rows and updates aria-sort when a sortable header is toggled', async () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const root = createRoot(container);

    await act(async () => {
      root.render(
        <DataTable
          columns={[{ accessorKey: 'name', header: 'Name', cell: ({ row }) => row.original.name }]}
          data={[{ name: 'Beta' }, { name: 'Alpha' }]}
          getRowKey={(row) => row.name}
        />,
      );
      await Promise.resolve();
    });

    const rowKeys = () =>
      [...container.querySelectorAll('[data-row-key]')].map((row) =>
        row.getAttribute('data-row-key'),
      );
    const sortButton = container.querySelector<HTMLButtonElement>('[data-slot="table-sort"]')!;
    const headerCell = sortButton.closest('th')!;

    expect(rowKeys()).toEqual(['Beta', 'Alpha']);
    expect(headerCell.getAttribute('aria-sort')).toBe('none');

    // Strings sort ascending on the first toggle.
    await act(async () => {
      sortButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(rowKeys()).toEqual(['Alpha', 'Beta']);
    expect(headerCell.getAttribute('aria-sort')).toBe('ascending');

    // A second toggle flips to descending.
    await act(async () => {
      sortButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(rowKeys()).toEqual(['Beta', 'Alpha']);
    expect(headerCell.getAttribute('aria-sort')).toBe('descending');

    act(() => root.unmount());
    container.remove();
  });
});
