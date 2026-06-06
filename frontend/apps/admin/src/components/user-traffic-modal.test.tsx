import type { CSSProperties, ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { UserTrafficModal } from './user-traffic-modal';

const mocks = vi.hoisted(() => ({
  useAdminUserTraffic: vi.fn(),
}));

vi.mock('@/lib/queries', () => ({
  useAdminUserTraffic: mocks.useAdminUserTraffic,
}));

vi.mock('antd', () => ({
  Modal: ({
    children,
    footer,
    open,
    style,
    styles,
    title,
    width,
  }: {
    children: ReactNode;
    footer?: boolean;
    open?: boolean;
    style?: CSSProperties;
    styles?: { body?: CSSProperties };
    title?: ReactNode;
    width?: string | number;
  }) =>
    open ? (
      <section
        className="ant-modal"
        data-footer={String(footer)}
        data-title={title}
        data-width={width}
        style={style}
      >
        <div className="ant-modal-body" style={styles?.body}>
          {children}
        </div>
      </section>
    ) : null,
}));

describe('UserTrafficModal', () => {
  beforeEach(() => {
    mocks.useAdminUserTraffic.mockReset();
    mocks.useAdminUserTraffic.mockReturnValue({
      data: {
        data: [{ record_at: 1700000000, u: 1024, d: 2048, server_rate: 1 }],
        total: 25,
      },
      isFetching: false,
    });
  });

  it('renders the bundled admin traffic modal table and mini pagination markup', () => {
    const html = renderToStaticMarkup(
      <UserTrafficModal userId={1} open onClose={() => undefined} />,
    );

    expect(mocks.useAdminUserTraffic).toHaveBeenCalledWith(
      1,
      { page: 1, pageSize: 10, total: 0 },
      true,
    );
    expect(html).toContain('class="ant-modal"');
    expect(html).toContain('data-title="流量记录"');
    expect(html).toContain('data-width="100%"');
    expect(html).toContain('data-footer="false"');
    expect(html).toContain('style="max-width:1000px;padding:0 10px;top:20px"');
    expect(html).toContain('class="ant-modal-body" style="padding:0"');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain('class="ant-table ant-table-default ant-table-scroll-position-left"');
    expect(html).toContain('class="ant-table-header-column"');
    expect(html).toContain('日期');
    expect(html).toContain('上行');
    expect(html).toContain('下行');
    expect(html).toContain('倍率');
    expect(html).toContain('class="ant-table-row ant-table-row-level-0" data-row-key="0"');
    expect(html).toContain('<td>2023-11-14</td>');
    expect(html).toContain('<td style="text-align:right">1024.00 B</td>');
    expect(html).toContain('<td style="text-align:right">2.00 KB</td>');
    expect(html).toContain('<td style="text-align:right">1</td>');
    expect(html).toContain('class="ant-pagination ant-table-pagination mini"');
    expect(html).toContain(
      'class="ant-pagination-item ant-pagination-item-1 ant-pagination-item-active"',
    );
    expect(html).toContain('class="ant-pagination-item ant-pagination-item-3"');
    expect(html).not.toContain('css-dev-only-do-not-override');
  });

  it('keeps the modal closed without rendering stale traffic rows', () => {
    const html = renderToStaticMarkup(
      <UserTrafficModal userId={1} open={false} onClose={() => undefined} />,
    );

    expect(mocks.useAdminUserTraffic).toHaveBeenCalledWith(
      1,
      { page: 1, pageSize: 10, total: 0 },
      false,
    );
    expect(html).toBe('');
  });
});
