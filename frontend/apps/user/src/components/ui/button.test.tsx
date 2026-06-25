import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Button } from './button';

describe('Button', () => {
  it('renders a token-driven primary button by default', () => {
    const html = renderToStaticMarkup(<Button>Go</Button>);
    expect(html).toContain('tw:bg-primary');
    expect(html).toContain('tw:text-primary-foreground');
    expect(html).toContain('tw:rounded-field');
    expect(html).toContain('type="button"');
    expect(html).toContain('>Go<');
  });

  it('applies the requested variant and size', () => {
    const html = renderToStaticMarkup(
      <Button variant="outline" size="sm">
        x
      </Button>,
    );
    expect(html).toContain('tw:border-input');
    expect(html).toContain('tw:h-9');
  });

  it('stretches full width with block', () => {
    expect(renderToStaticMarkup(<Button block>x</Button>)).toContain('tw:w-full');
  });

  it('shows a spinner, disables, and marks aria-busy while loading, keeping the label visible', () => {
    const html = renderToStaticMarkup(<Button loading>Sign in</Button>);
    expect(html).toContain('tw:animate-spin');
    expect(html).toContain('disabled=""');
    expect(html).toContain('aria-busy="true"');
    expect(html).toContain('Sign in');
  });

  it('honors an explicit disabled state and type', () => {
    const html = renderToStaticMarkup(
      <Button disabled type="submit">
        x
      </Button>,
    );
    expect(html).toContain('disabled=""');
    expect(html).toContain('type="submit"');
  });

  it('merges a caller className', () => {
    expect(renderToStaticMarkup(<Button className="tw:mt-2">x</Button>)).toContain('tw:mt-2');
  });
});
