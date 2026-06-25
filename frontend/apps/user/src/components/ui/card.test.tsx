import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Card, CardBody, CardFooter } from './card';

describe('Card', () => {
  it('renders a token-driven surface', () => {
    const html = renderToStaticMarkup(<Card>x</Card>);
    expect(html).toContain('tw:rounded-card');
    expect(html).toContain('tw:bg-surface');
    expect(html).toContain('tw:shadow-card');
  });

  it('composes a body and a bordered footer', () => {
    const html = renderToStaticMarkup(
      <Card>
        <CardBody>body</CardBody>
        <CardFooter>footer</CardFooter>
      </Card>,
    );
    expect(html).toContain('body');
    expect(html).toContain('footer');
    expect(html).toContain('tw:border-t');
    expect(html).toContain('tw:border-border');
  });
});
