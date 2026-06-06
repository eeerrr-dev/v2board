import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LegacyEmpty } from './legacy-empty';

describe('LegacyEmpty', () => {
  it('renders the old Ant Design zh-CN empty table placeholder', () => {
    const html = renderToStaticMarkup(<LegacyEmpty />);

    expect(html).toContain('class="ant-empty ant-empty-normal"');
    expect(html).toContain('class="ant-empty-image"');
    expect(html).toContain('class="ant-empty-description"');
    expect(html).toContain('暂无数据');
    expect(html).not.toContain('css-dev-only-do-not-override');
  });
});
