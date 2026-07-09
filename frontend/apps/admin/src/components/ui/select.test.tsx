import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './select';

describe('Select', () => {
  it('composes Radix select primitives with shadcn classes', () => {
    const html = renderToStaticMarkup(
      <Select value="example.com">
        <SelectTrigger aria-label="Email domain">
          <SelectValue>@example.com</SelectValue>
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="example.com">@example.com</SelectItem>
        </SelectContent>
      </Select>,
    );

    expect(html).toContain('role="combobox"');
    expect(html).toContain('aria-label="Email domain"');
    expect(html).toContain('border-input');
    expect(html).toContain('@example.com');
    expect(html).not.toContain('<option');
  });

  it('renders invalid trigger state with destructive treatment', () => {
    const html = renderToStaticMarkup(
      <Select>
        <SelectTrigger invalid>
          <SelectValue placeholder="Choose" />
        </SelectTrigger>
      </Select>,
    );

    expect(html).toContain('aria-invalid="true"');
    expect(html).toContain('border-destructive');
  });
});
