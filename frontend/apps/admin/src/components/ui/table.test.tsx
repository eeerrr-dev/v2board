import { screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderWithProviders } from '@/test/render';
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
  VIRTUALIZE_MIN_ROWS,
} from './table';

describe('Table', () => {
  it('composes table primitives and passes caller hook classes and data attributes through', () => {
    renderWithProviders(
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

    const table = screen.getByRole('table');
    // Caller-supplied stable-selector hooks must land on the rendered elements.
    expect(table).toHaveClass('v2board-table');
    expect(table.parentElement).toHaveClass('v2board-table-scroll');
    expect(screen.getByRole('columnheader', { name: 'Name' })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: 'Amount' })).toHaveClass('text-right');
    expect(screen.getByRole('cell', { name: '12.00' })).toHaveClass('text-right');
    // Data attributes pass through so row-level hooks stay addressable.
    expect(screen.getByRole('cell', { name: 'Alpha' }).closest('tr')).toHaveAttribute(
      'data-row-key',
      '0',
    );
  });

  it('renders a reusable empty row spanning the table columns', () => {
    renderWithProviders(
      <Table>
        <TableBody>
          <TableEmpty colSpan={3} rowClassName="v2board-empty">
            Empty
          </TableEmpty>
        </TableBody>
      </Table>,
    );

    const cell = screen.getByRole('cell', { name: 'Empty' });
    expect(cell).toHaveAttribute('colspan', '3');
    expect(cell.closest('tr')).toHaveClass('v2board-empty');
  });

  it('renders DataTable rows, cells, and row keys from column definitions', () => {
    renderWithProviders(
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

    expect(screen.getByRole('columnheader', { name: 'Name' })).toBeInTheDocument();
    expect(screen.getByRole('cell', { name: 'Alpha' })).toBeInTheDocument();
    // meta.align drives cell alignment through the column definition API.
    expect(screen.getByRole('cell', { name: '12.00' })).toHaveClass('text-right');
    // getRowKey feeds the row identity exposed as data-row-key.
    expect(screen.getByRole('cell', { name: 'Alpha' }).closest('tr')).toHaveAttribute(
      'data-row-key',
      'Alpha',
    );
  });

  it('virtualizes large datasets behind aria-hidden spacer rows', () => {
    const data = Array.from({ length: 300 }, (_, index) => ({ name: `row-${index}` }));
    const { container } = renderWithProviders(
      <DataTable
        columns={[{ accessorKey: 'name', header: 'Name', cell: ({ row }) => row.original.name }]}
        data={data}
        getRowKey={(row) => row.name}
        virtualizer={{ enabled: data.length >= VIRTUALIZE_MIN_ROWS, estimateSize: 50 }}
      />,
    );

    // The virtualizer must window the DOM instead of materializing every row
    // (in this zero-height test viewport the visible window is empty)...
    const materialized = container.querySelectorAll('tbody tr[data-row-key]');
    expect(materialized.length).toBeLessThan(data.length);
    // ...while aria-hidden spacer rows preserve the total scroll height.
    const spacers = [...container.querySelectorAll('tbody tr[aria-hidden="true"] td')];
    const spacerHeight = spacers.reduce(
      (total, cell) => total + Number.parseFloat((cell as HTMLElement).style.height || '0'),
      0,
    );
    expect(spacerHeight + materialized.length * 50).toBe(data.length * 50);
  });
});

describe('DataTable sorting', () => {
  it('makes accessor columns sortable and leaves display-only columns inert', () => {
    renderWithProviders(
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
    const nameHeader = screen.getByRole('columnheader', { name: 'Name' });
    expect(nameHeader).toHaveAttribute('aria-sort', 'none');
    expect(within(nameHeader).getByRole('button')).toBeInTheDocument();
    // ...while the display-only "Action" header stays a plain, non-interactive cell.
    const actionHeader = screen.getByRole('columnheader', { name: 'Action' });
    expect(actionHeader).not.toHaveAttribute('aria-sort');
    expect(within(actionHeader).queryByRole('button')).toBeNull();
  });

  it('reorders rows and updates aria-sort when a sortable header is toggled', async () => {
    const { container, user } = renderWithProviders(
      <DataTable
        columns={[{ accessorKey: 'name', header: 'Name', cell: ({ row }) => row.original.name }]}
        data={[{ name: 'Beta' }, { name: 'Alpha' }]}
        getRowKey={(row) => row.name}
      />,
    );

    const rowKeys = () =>
      [...container.querySelectorAll('[data-row-key]')].map((row) =>
        row.getAttribute('data-row-key'),
      );
    const headerCell = screen.getByRole('columnheader', { name: 'Name' });
    const sortButton = within(headerCell).getByRole('button');

    expect(rowKeys()).toEqual(['Beta', 'Alpha']);
    expect(headerCell).toHaveAttribute('aria-sort', 'none');

    // Strings sort ascending on the first toggle.
    await user.click(sortButton);
    expect(rowKeys()).toEqual(['Alpha', 'Beta']);
    expect(headerCell).toHaveAttribute('aria-sort', 'ascending');

    // A second toggle flips to descending.
    await user.click(sortButton);
    expect(rowKeys()).toEqual(['Beta', 'Alpha']);
    expect(headerCell).toHaveAttribute('aria-sort', 'descending');
  });
});
