import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { PlanContent } from './plan-content';

const planContentSource = readFileSync(`${process.cwd()}/src/components/plan-content.tsx`, 'utf8');

describe('PlanContent shadcn feature rendering', () => {
  it('renders parsed feature rows with lucide icons and shadcn text treatment', () => {
    const html = renderToStaticMarkup(
      <PlanContent
        content={JSON.stringify([
          { feature: 'Supported', support: true },
          { feature: 'Unsupported', support: false },
        ])}
        className="mb-3"
      />,
    );

    expect(html).toContain('grid gap-2.5 text-sm mb-3');
    expect(html).toContain('lucide-check');
    expect(html).toContain('lucide-x');
    expect(html).toContain('text-primary');
    expect(html).toContain('Supported');
    expect(html).toContain('Unsupported');
    expect(html).toContain('opacity-70');
    expect(html).not.toContain('si si-check');
  });

  it('uses stable feature row keys without random remounts', () => {
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

  it('falls back to raw HTML for JSON null in plan lists instead of crashing', () => {
    const html = renderToStaticMarkup(<PlanContent content="null" className="mb-3" />);

    expect(html).toContain('class="mb-3"');
    expect(html).toContain('>null</div>');
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

  it('keeps the direct raw HTML handoff without an empty-string fallback', () => {
    expect(planContentSource).toContain('dangerouslySetInnerHTML={{ __html: content as string }}');
    expect(planContentSource).not.toContain("dangerouslySetInnerHTML={{ __html: content ?? '' }}");
  });
});
