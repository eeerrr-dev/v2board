import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Textarea } from './textarea';

describe('Textarea', () => {
  it('renders a shadcn-style textarea by default', () => {
    const html = renderToStaticMarkup(<Textarea />);
    expect(html).toContain('<textarea');
    expect(html).toContain('border-input');
    expect(html).toContain('rounded-md');
    expect(html).toContain('bg-transparent');
  });

  it('omits aria-invalid in the resting state', () => {
    expect(renderToStaticMarkup(<Textarea />)).not.toContain('aria-invalid="');
  });

  it('sets aria-invalid so the destructive variants engage when invalid', () => {
    const html = renderToStaticMarkup(<Textarea aria-invalid />);
    expect(html).toContain('aria-invalid="true"');
    expect(html).toContain('aria-invalid:border-destructive');
    expect(html).toContain('dark:aria-invalid:ring-destructive/40');
  });

  it('merges a caller className and forwards attributes', () => {
    const html = renderToStaticMarkup(<Textarea className="mt-1" name="message" />);
    expect(html).toContain('mt-1');
    expect(html).toContain('name="message"');
  });
});
