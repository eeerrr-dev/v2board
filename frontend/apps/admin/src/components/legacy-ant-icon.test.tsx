import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LegacyPlusIcon } from './legacy-ant-icon';

describe('LegacyPlusIcon', () => {
  it('renders the old Ant Design plus icon DOM used by the bundled admin theme', () => {
    const html = renderToStaticMarkup(<LegacyPlusIcon />);

    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('class="anticon anticon-plus"');
    expect(html).toContain('class=""');
    expect(html).toContain('data-icon="plus"');
    expect(html).toContain(
      'd="M482 152h60q8 0 8 8v704q0 8-8 8h-60q-8 0-8-8V160q0-8 8-8z"',
    );
    expect(html).toContain(
      'd="M176 474h672q8 0 8 8v60q0 8-8 8H176q-8 0-8-8v-60q0-8 8-8z"',
    );
    expect(html).not.toContain('role="img"');
    expect(html).not.toContain('M192 474');
  });
});
