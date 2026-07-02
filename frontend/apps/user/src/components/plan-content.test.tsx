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
        className="v2board-plan-content px-3"
        htmlClassName="v2board-plan-content"
      />,
    );

    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper).toHaveClass('v2board-plan-content');
    expect(wrapper).not.toHaveClass('px-3');
    expect(wrapper).toHaveTextContent('null');
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
