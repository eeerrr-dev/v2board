import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Button } from './button';

describe('Button', () => {
  it('renders a shadcn-style primary button by default', () => {
    const html = renderToStaticMarkup(<Button>Go</Button>);
    expect(html).toContain('bg-primary');
    expect(html).toContain('text-primary-foreground');
    expect(html).toContain('rounded-md');
    expect(html).toContain('type="button"');
    expect(html).toContain('>Go<');
  });

  it('applies the requested variant and size', () => {
    const html = renderToStaticMarkup(
      <Button variant="outline" size="sm">
        x
      </Button>,
    );
    expect(html).toContain('border');
    expect(html).toContain('border-border');
    expect(html).toContain('h-8');
  });

  it('stretches full width with block', () => {
    expect(renderToStaticMarkup(<Button block>x</Button>)).toContain('w-full');
  });

  it('shows a spinner, disables, and marks aria-busy while loading, keeping the label visible', () => {
    const html = renderToStaticMarkup(<Button loading>Sign in</Button>);
    expect(html).toContain('animate-spin');
    expect(html).toContain('disabled=""');
    expect(html).toContain('aria-busy="true"');
    expect(html).toContain('Sign in');
  });

  it('keeps loading buttons disabled even when a caller passes disabled={false}', () => {
    const html = renderToStaticMarkup(
      <Button loading disabled={false}>
        Saving
      </Button>,
    );
    expect(html).toContain('disabled=""');
    expect(html).toContain('aria-busy="true"');
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
    expect(renderToStaticMarkup(<Button className="mt-2">x</Button>)).toContain('mt-2');
  });
});
