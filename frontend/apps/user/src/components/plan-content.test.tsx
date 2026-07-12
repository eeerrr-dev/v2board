// @vitest-environment jsdom
import { screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { PlanContent } from './plan-content';

const featureContent = JSON.stringify([
  { feature: 'Supported', support: true },
  { feature: 'Unsupported', support: false },
]);

describe('PlanContent shadcn feature rendering', () => {
  it('renders parsed feature rows with check/x icons split by support', () => {
    const { container } = renderWithProviders(
      <PlanContent content={featureContent} className="mb-3" />,
    );

    const supportedRow = screen.getByText('Supported').closest('div')!;
    const unsupportedRow = screen.getByText('Unsupported').closest('div')!;

    // The lucide icons are aria-hidden, so the class token is the only stable
    // handle distinguishing the supported check from the unsupported x.
    expect(supportedRow.querySelector('svg[class*="lucide-check"]')).not.toBeNull();
    expect(supportedRow.querySelector('svg[class*="lucide-x"]')).toBeNull();
    expect(unsupportedRow.querySelector('svg[class*="lucide-x"]')).not.toBeNull();
    expect(unsupportedRow.querySelector('svg[class*="lucide-check"]')).toBeNull();

    // className is merged onto the feature-list wrapper.
    expect(container.firstElementChild).toHaveClass('mb-3');
  });

  it('reuses feature row DOM nodes across rerenders (stable keys, no random remounts)', () => {
    const { rerender } = renderWithProviders(
      <PlanContent content={featureContent} className="mb-3" />,
    );

    const supportedBefore = screen.getByText('Supported');
    const unsupportedBefore = screen.getByText('Unsupported');

    rerender(<PlanContent content={featureContent} className="mb-3" />);

    // Random keys (key={Math.random()}) would remount every row and hand back
    // fresh DOM nodes; stable keys keep the exact same elements alive.
    expect(screen.getByText('Supported')).toBe(supportedBefore);
    expect(screen.getByText('Unsupported')).toBe(unsupportedBefore);
  });

  it('falls back to raw HTML for non-JSON content', () => {
    const { container } = renderWithProviders(
      <PlanContent content="<p>Raw HTML</p>" className="mb-3" />,
    );

    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper).toHaveClass('mb-3');
    expect(within(wrapper).getByText('Raw HTML').tagName).toBe('P');
  });

  it('falls back to raw HTML for JSON null in plan lists instead of crashing', () => {
    const { container } = renderWithProviders(<PlanContent content="null" className="mb-3" />);

    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper).toHaveClass('mb-3');
    expect(wrapper).toHaveTextContent('null');
  });

  it('keeps the checkout JSON null fallback as raw HTML with htmlClassName precedence', () => {
    const { container } = renderWithProviders(
      <PlanContent
        content="null"
        className="test-plan-content px-3"
        htmlClassName="test-plan-content"
      />,
    );

    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper).toHaveClass('test-plan-content');
    expect(wrapper).not.toHaveClass('px-3');
    expect(wrapper).toHaveTextContent('null');
  });

  it.each([
    ['a null row', [null]],
    ['a string row', ['not-a-feature-object']],
    ['a null feature', [{ feature: null, support: false }]],
    ['an object feature', [{ feature: { label: 'nested' }, support: true }]],
  ])('falls back to sanitized HTML for a feature array containing %s', (_case, value) => {
    const content = JSON.stringify(value);
    const { container } = renderWithProviders(
      <PlanContent content={content} className="feature-list" htmlClassName="html-fallback" />,
    );

    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper).toHaveClass('html-fallback');
    expect(wrapper).not.toHaveClass('feature-list');
    expect(wrapper.querySelector('svg')).toBeNull();
    expect(wrapper.textContent).toBe(content);
  });

  it('preserves scalar support truthiness and renders numeric feature labels', () => {
    renderWithProviders(
      <PlanContent
        content={JSON.stringify([
          { feature: 1, support: 1 },
          { feature: 'String support', support: 'yes' },
          { feature: 'Empty support', support: '' },
          { feature: 'Null support', support: null },
        ])}
      />,
    );

    expect(screen.getByText('1').closest('div')?.querySelector('.lucide-check')).not.toBeNull();
    expect(
      screen.getByText('String support').closest('div')?.querySelector('.lucide-check'),
    ).not.toBeNull();
    expect(
      screen.getByText('Empty support').closest('div')?.querySelector('.lucide-x'),
    ).not.toBeNull();
    expect(
      screen.getByText('Null support').closest('div')?.querySelector('.lucide-x'),
    ).not.toBeNull();
  });

  it('sanitizes the raw HTML handoff, stripping scripts and event handlers', () => {
    const { container } = renderWithProviders(
      <PlanContent
        content={'<p onclick="alert(1)">Raw HTML</p><script>window.pwned = true;</script>'}
        className="mb-3"
      />,
    );

    const paragraph = within(container.firstElementChild as HTMLElement).getByText('Raw HTML');
    expect(paragraph.tagName).toBe('P');
    expect(paragraph).not.toHaveAttribute('onclick');
    expect(container.querySelector('script')).toBeNull();
  });
});
