import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { PlanContent } from './plan-content';

const planContentSource = readFileSync(`${process.cwd()}/src/components/plan-content.tsx`, 'utf8');

describe('PlanContent bundled-theme quirks', () => {
  it('renders parsed feature rows with the bundled-theme icon markup', () => {
    const html = renderToStaticMarkup(
      <PlanContent
        content={JSON.stringify([
          { feature: 'Supported', support: true },
          { feature: 'Unsupported', support: false },
        ])}
        className="mb-3"
      />,
    );

    expect(html).toContain('class="mb-3"');
    expect(html).toContain('si si-check text-primary');
    expect(html).toContain('si si-close text-primary');
    expect(html).toContain('Supported');
    expect(html).toContain('Unsupported');
    expect(html).toContain('opacity:0.3');
  });

  it('uses stable feature row keys without changing the bundled-theme markup', () => {
    const featureSource = planContentSource.slice(
      planContentSource.indexOf('features.map((item, index) => {'),
      planContentSource.indexOf('</div>', planContentSource.indexOf('features.map((item, index) => {')),
    );

    expect(featureSource).toContain('features.map((item, index) => {');
    expect(featureSource).toContain('key={index}');
    expect(featureSource).not.toContain('key={Math.random()}');
  });

  it('falls back to raw HTML for non-JSON content', () => {
    const html = renderToStaticMarkup(
      <PlanContent content="<p>Raw HTML</p>" className="mb-3" />,
    );

    expect(html).toContain('class="mb-3"');
    expect(html).toContain('<p>Raw HTML</p>');
  });

  it('keeps the plan-list JSON null crash from the original typeof-object guard', () => {
    expect(() =>
      renderToStaticMarkup(<PlanContent content="null" className="mb-3" />),
    ).toThrow();
  });

  it('keeps the checkout JSON null fallback as raw HTML', () => {
    const html = renderToStaticMarkup(
      <PlanContent
        content="null"
        className="v2board-plan-content px-3"
        htmlClassName="v2board-plan-content"
        guardNull
      />,
    );

    expect(html).toContain('class="v2board-plan-content"');
    expect(html).not.toContain('class="v2board-plan-content px-3"');
    expect(html).toContain('>null</div>');
  });

  it('keeps the bundled-theme raw HTML handoff without an empty-string fallback', () => {
    expect(planContentSource).toContain('dangerouslySetInnerHTML={{ __html: content as string }}');
    expect(planContentSource).not.toContain("dangerouslySetInnerHTML={{ __html: content ?? '' }}");
  });
});
