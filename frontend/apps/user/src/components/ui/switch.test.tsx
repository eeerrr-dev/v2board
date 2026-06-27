import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Switch } from './switch';

describe('Switch', () => {
  it('renders the thumb without the legacy OneUI block class token', () => {
    const html = renderToStaticMarkup(<Switch />);

    expect(html).toContain('inline-block');
    expect(html).not.toMatch(/class="[^"]*(^|\s)block(\s|$)[^"]*"/);
  });
});
