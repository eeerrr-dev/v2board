import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LegacyBadge } from './legacy-badge';

describe('LegacyBadge', () => {
  it('renders the old status badge shell including the empty status text spacer', () => {
    const html = renderToStaticMarkup(<LegacyBadge status="success" />);

    expect(html).toContain(
      '<span class="ant-badge ant-badge-status ant-badge-not-a-wrapper">',
    );
    expect(html).toContain(
      '<span class="ant-badge-status-dot ant-badge-status-success"></span>',
    );
    expect(html).toContain('<span class="ant-badge-status-text"></span>');
  });

  it('keeps old status text color and custom color behavior', () => {
    const html = renderToStaticMarkup(
      <LegacyBadge color="#123456" style={{ color: '#654321' }} text="自定义" />,
    );

    expect(html).toContain('class="ant-badge ant-badge-status ant-badge-not-a-wrapper"');
    expect(html).toContain('class="ant-badge-status-dot" style="background:#123456"');
    expect(html).toContain(
      '<span style="color:#654321" class="ant-badge-status-text">自定义</span>',
    );
  });

  it('renders the old count badge classes around children', () => {
    const html = renderToStaticMarkup(
      <LegacyBadge count={120} overflowCount={99}>
        <a>订单</a>
      </LegacyBadge>,
    );

    expect(html).toContain('<span class="ant-badge"><a>订单</a>');
    expect(html).toContain('<sup data-show="true" class="ant-badge-count ant-badge-multiple-words">99+</sup>');
  });

  it('hides zero counts unless showZero is set', () => {
    const hiddenHtml = renderToStaticMarkup(
      <LegacyBadge count={0}>
        <a>订单</a>
      </LegacyBadge>,
    );
    const visibleHtml = renderToStaticMarkup(
      <LegacyBadge count={0} showZero>
        <a>订单</a>
      </LegacyBadge>,
    );

    expect(hiddenHtml).not.toContain('ant-badge-count');
    expect(visibleHtml).toContain('<sup data-show="true" class="ant-badge-count">0</sup>');
  });
});
