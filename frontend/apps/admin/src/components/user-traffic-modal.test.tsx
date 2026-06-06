import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { UserTrafficModal } from './user-traffic-modal';

const mocks = vi.hoisted(() => ({
  useAdminUserTraffic: vi.fn(),
}));

vi.mock('@/lib/queries', () => ({
  useAdminUserTraffic: mocks.useAdminUserTraffic,
}));

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('UserTrafficModal', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.useAdminUserTraffic.mockReset();
    mocks.useAdminUserTraffic.mockReturnValue({
      data: {
        data: [{ record_at: 1700000000, u: 1024, d: 2048, server_rate: 1 }],
        total: 25,
      },
      isFetching: false,
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
    document.body.className = '';
  });

  it('renders the bundled admin traffic modal table and mini pagination markup', async () => {
    await act(async () => {
      root.render(<UserTrafficModal userId={1} open onClose={() => undefined} />);
    });

    const modal = document.querySelector('.ant-modal')!;
    const body = document.querySelector('.ant-modal-body')!;
    const html = document.body.innerHTML;

    expect(mocks.useAdminUserTraffic).toHaveBeenCalledWith(
      1,
      { page: 1, pageSize: 10, total: 0 },
      true,
    );
    expect(html).toContain('class="ant-modal-root"');
    expect(modal.outerHTML).toContain('<div class="ant-modal" role="document"');
    expect(modal.getAttribute('style')).toBe(
      'width: 100%; max-width: 1000px; padding: 0px 10px; top: 20px;',
    );
    expect(html).toContain('<div class="ant-modal-title">流量记录</div>');
    expect(body.getAttribute('style')).toBe('padding: 0px;');
    expect(document.querySelector('.ant-modal-footer')).toBeNull();
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain('class="ant-table ant-table-default ant-table-scroll-position-left"');
    expect(html).toContain('class="ant-table-header-column"');
    expect(html).toContain('日期');
    expect(html).toContain('上行');
    expect(html).toContain('下行');
    expect(html).toContain('倍率');
    expect(html).toContain('class="ant-table-row ant-table-row-level-0" data-row-key="0"');
    expect(html).toContain('<td>2023-11-14</td>');
    expect(html).toContain('<td style="text-align: right;">1024.00 B</td>');
    expect(html).toContain('<td style="text-align: right;">2.00 KB</td>');
    expect(html).toContain('<td style="text-align: right;">1</td>');
    expect(html).toContain('class="ant-pagination ant-table-pagination mini"');
    expect(html).toContain(
      'class="ant-pagination-item ant-pagination-item-1 ant-pagination-item-active"',
    );
    expect(html).toContain('class="ant-pagination-item ant-pagination-item-3"');
    expect(html).not.toContain('css-dev-only-do-not-override');
  });

  it('keeps the modal closed without rendering stale traffic rows', async () => {
    await act(async () => {
      root.render(<UserTrafficModal userId={1} open={false} onClose={() => undefined} />);
    });

    expect(mocks.useAdminUserTraffic).toHaveBeenCalledWith(
      1,
      { page: 1, pageSize: 10, total: 0 },
      false,
    );
    expect(document.querySelector('.ant-modal-root')).toBeNull();
  });
});
