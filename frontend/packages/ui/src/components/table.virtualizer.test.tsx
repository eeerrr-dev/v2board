import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { DataTable, VIRTUALIZE_MIN_ROWS } from './table';

interface CapturedVirtualizerOptions {
  count: number;
  enabled: boolean;
  getItemKey: (index: number) => string | number;
}

const captured = vi.hoisted(() => ({
  options: undefined as CapturedVirtualizerOptions | undefined,
}));

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: (options: CapturedVirtualizerOptions) => {
    captured.options = options;
    return {
      getTotalSize: () => 0,
      getVirtualItems: () => [],
      measureElement: vi.fn(),
    };
  },
}));

describe('DataTable virtualizer options', () => {
  it('keeps the full row count and keys measurements by the sorted row id', async () => {
    const data = Array.from({ length: VIRTUALIZE_MIN_ROWS }, (_, index) => ({
      name: `row-${String(VIRTUALIZE_MIN_ROWS - index - 1).padStart(3, '0')}`,
    }));
    const user = userEvent.setup();
    render(
      <DataTable
        columns={[{ accessorKey: 'name', header: 'Name', cell: ({ row }) => row.original.name }]}
        data={data}
        getRowKey={(row) => row.name}
        virtualizer={{ enabled: true }}
      />,
    );

    expect(captured.options).toMatchObject({
      count: VIRTUALIZE_MIN_ROWS,
      enabled: true,
    });
    expect(captured.options?.getItemKey(0)).toBe('row-149');

    await user.click(
      within(screen.getByRole('columnheader', { name: 'Name' })).getByRole('button'),
    );

    expect(captured.options?.getItemKey(0)).toBe('row-000');
  });
});
