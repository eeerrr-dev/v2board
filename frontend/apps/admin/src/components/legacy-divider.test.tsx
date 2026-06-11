import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LegacyDivider } from './legacy-divider';

describe('LegacyDivider', () => {
  it('renders the old horizontal divider shell by default', () => {
    const html = renderToStaticMarkup(<LegacyDivider />);

    expect(html).toBe(
      '<div class="ant-divider ant-divider-horizontal" role="separator"></div>',
    );
    expect(html).not.toContain('css-dev-only-do-not-override');
  });

  it('renders the old vertical divider shell', () => {
    const html = renderToStaticMarkup(<LegacyDivider type="vertical" />);

    expect(html).toBe(
      '<div class="ant-divider ant-divider-vertical" role="separator"></div>',
    );
  });

  it('matches the old text, orientation, dashed, and custom prefix classes', () => {
    const html = renderToStaticMarkup(
      <LegacyDivider className="extra" dashed orientation="left" prefixCls="legacy-divider">
        条件
      </LegacyDivider>,
    );

    expect(html).toBe(
      '<div class="extra legacy-divider legacy-divider-horizontal legacy-divider-with-text-left legacy-divider-dashed" role="separator"><span class="legacy-divider-inner-text">条件</span></div>',
    );
  });

  it('forces the old separator role over a passed role prop', () => {
    const html = renderToStaticMarkup(<LegacyDivider role="presentation" data-kind="split" />);

    expect(html).toContain('data-kind="split"');
    expect(html).toContain('role="separator"');
    expect(html).not.toContain('role="presentation"');
  });
});
