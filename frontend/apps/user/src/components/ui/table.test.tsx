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
