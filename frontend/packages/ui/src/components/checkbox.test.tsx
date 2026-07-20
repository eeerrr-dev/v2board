import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Checkbox } from './checkbox';

describe('Checkbox', () => {
  it('renders a Radix shadcn-style checkbox control', () => {
    const html = renderToStaticMarkup(<Checkbox id="tos" checked />);

    expect(html).toContain('role="checkbox"');
    expect(html).toContain('aria-checked="true"');
    expect(html).toContain('data-state="checked"');
    expect(html).toContain('data-[state=checked]:bg-primary');
    expect(html).toContain('<button type="button" role="checkbox"');
    expect(html).toContain('<input type="checkbox" aria-hidden="true"');
  });

  it('merges caller class names', () => {
    expect(renderToStaticMarkup(<Checkbox className="mt-0.5" />)).toContain('mt-0.5');
  });
});
