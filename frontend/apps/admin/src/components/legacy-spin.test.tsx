import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LegacySpin } from './legacy-spin';

describe('LegacySpin', () => {
  it('renders the old nested loading structure without Ant Design v5 Spin', () => {
    const idle = renderToStaticMarkup(
      <LegacySpin loading={false}>
        <button type="button">编辑排序</button>
      </LegacySpin>,
    );

    expect(idle).toContain('class="ant-spin-nested-loading"');
    expect(idle).toContain('class="ant-spin-container"');
    expect(idle).not.toContain('class="ant-spin"');
    expect(idle).not.toContain('class="spinner-grow text-primary"');
    expect(idle).toContain('<button type="button">编辑排序</button>');
    expect(idle).not.toContain('css-dev-only-do-not-override');

    const loading = renderToStaticMarkup(
      <LegacySpin loading>
        <span>content</span>
      </LegacySpin>,
    );

    expect(loading).toContain('class="ant-spin ant-spin-spinning"');
    expect(loading).toContain('class="ant-spin-container ant-spin-blur"');
  });
});
