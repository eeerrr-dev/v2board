import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Input } from './input';

describe('Input', () => {
  it('renders a token-driven text input by default', () => {
    const html = renderToStaticMarkup(<Input />);
    expect(html).toContain('type="text"');
    expect(html).toContain('tw:border-input');
    expect(html).toContain('tw:rounded-field');
    expect(html).toContain('tw:bg-surface');
  });

  it('respects an explicit type and forwards attributes', () => {
    const html = renderToStaticMarkup(<Input type="password" name="pw" />);
    expect(html).toContain('type="password"');
    expect(html).toContain('name="pw"');
  });

  it('merges a caller className', () => {
    expect(renderToStaticMarkup(<Input className="tw:mt-1" />)).toContain('tw:mt-1');
  });
});
