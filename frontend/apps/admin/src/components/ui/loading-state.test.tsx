import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LoadingState, SkeletonFields, SkeletonLines, SkeletonRows } from './loading-state';

describe('LoadingState', () => {
  it('renders an accessible status region with the default loading label', () => {
    const html = renderToStaticMarkup(
      <LoadingState data-testid="example-loading">
        <SkeletonRows rows={2} />
      </LoadingState>,
    );
    expect(html).toContain('role="status"');
    expect(html).toContain('data-testid="example-loading"');
    expect(html).toContain('sr-only');
    expect(html).toContain('加载中');
  });

  it('supports a custom label override', () => {
    const html = renderToStaticMarkup(<LoadingState label="正在加载支付接口" />);
    expect(html).toContain('正在加载支付接口');
    expect(html).not.toContain('>加载中<');
  });

  it('renders aria-hidden skeleton placeholders shaped by the composition helpers', () => {
    for (const children of [
      <SkeletonRows key="rows" rows={3} />,
      <SkeletonFields key="fields" fields={2} />,
      <SkeletonLines key="lines" lines={4} />,
    ]) {
      const html = renderToStaticMarkup(<LoadingState>{children}</LoadingState>);
      expect(html).toContain('aria-hidden');
      expect(html).toContain('data-slot="skeleton"');
      expect(html).toContain('animate-pulse');
    }
  });
});
