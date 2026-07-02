import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { HeaderTooltip } from './header-tooltip';
import { TooltipProvider } from './tooltip';

describe('HeaderTooltip', () => {
  it('renders the parity trigger span with the help icon and Radix tooltip wiring', () => {
    const html = renderToStaticMarkup(
      <TooltipProvider>
        <HeaderTooltip title="What this column means">Status</HeaderTooltip>
      </TooltipProvider>,
    );

    // Default alignment matches invite's local implementation byte-for-byte.
    expect(html).toContain(
      'class="v2board-service-tooltip-trigger inline-flex cursor-help items-center gap-1"',
    );
    expect(html).toContain('Status');
    expect(html).toContain('<svg');
    expect(html).toContain('size-3.5');
    // asChild hands the Radix trigger state to the span itself.
    expect(html).toContain('data-state="closed"');
  });

  it('cn-merges caller className so each table keeps its alignment variant', () => {
    const centered = renderToStaticMarkup(
      <TooltipProvider>
        <HeaderTooltip className="justify-center" title="tip">
          Rate
        </HeaderTooltip>
      </TooltipProvider>,
    );
    expect(centered).toContain(
      'class="v2board-service-tooltip-trigger inline-flex cursor-help items-center gap-1 justify-center"',
    );

    const endAligned = renderToStaticMarkup(
      <TooltipProvider>
        <HeaderTooltip className="justify-end" placement="topRight" title="tip">
          Total
        </HeaderTooltip>
      </TooltipProvider>,
    );
    expect(endAligned).toContain('justify-end');
  });

  it('accepts the placement knob exposed by TooltipContent', () => {
    // The closed tooltip portals no content, so this pins the prop contract:
    // both TooltipContent placements type-check and render without throwing.
    for (const placement of ['top', 'topRight'] as const) {
      expect(() =>
        renderToStaticMarkup(
          <TooltipProvider>
            <HeaderTooltip placement={placement} title="tip">
              Header
            </HeaderTooltip>
          </TooltipProvider>,
        ),
      ).not.toThrow();
    }
  });
});
