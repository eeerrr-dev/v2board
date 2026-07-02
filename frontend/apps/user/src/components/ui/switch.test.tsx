import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Switch } from './switch';

describe('Switch', () => {
  it('renders the thumb without the legacy OneUI block class token', () => {
    const html = renderToStaticMarkup(<Switch />);

    expect(html).toContain('inline-block');
    expect(html).not.toMatch(/class="[^"]*(^|\s)block(\s|$)[^"]*"/);
  });

  it('carries the registry dark-mode recipe on the track and thumb', () => {
    const html = renderToStaticMarkup(<Switch />);

    expect(html).toContain('dark:data-[state=unchecked]:bg-input/80');
    expect(html).toContain('dark:data-[state=checked]:bg-primary-foreground');
    expect(html).toContain('dark:data-[state=unchecked]:bg-foreground');
  });
});
