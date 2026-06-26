import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  Card,
  CardBody,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from './card';

describe('Card', () => {
  it('renders a shadcn-style surface', () => {
    const html = renderToStaticMarkup(<Card>x</Card>);
    expect(html).toContain('rounded-xl');
    expect(html).toContain('bg-card');
    expect(html).toContain('border-border');
    expect(html).toContain('text-card-foreground');
    expect(html).toContain('shadow-sm');
  });

  it('composes header, content, body alias, and footer primitives', () => {
    const html = renderToStaticMarkup(
      <Card>
        <CardHeader>
          <CardTitle>title</CardTitle>
          <CardDescription>description</CardDescription>
        </CardHeader>
        <CardContent>content</CardContent>
        <CardBody>body</CardBody>
        <CardFooter>footer</CardFooter>
      </Card>,
    );
    expect(html).toContain('title');
    expect(html).toContain('description');
    expect(html).toContain('content');
    expect(html).toContain('body');
    expect(html).toContain('footer');
    expect(html).toContain('px-6');
    expect(html).toContain('text-muted-foreground');
  });
});
