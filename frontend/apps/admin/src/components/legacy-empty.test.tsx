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

  it('renders the old default Empty illustration when requested', () => {
    const html = renderToStaticMarkup(<LegacyEmpty context="default" />);

    expect(html).toContain('class="ant-empty"');
    expect(html).not.toContain('ant-empty-normal');
    expect(html).toContain('width="184"');
    expect(html).toContain('height="152"');
  });

  it('matches old Select empty class composition', () => {
    const html = renderToStaticMarkup(<LegacyEmpty context="select" className="extra-empty" />);

    expect(html).toContain('class="ant-empty ant-empty-normal ant-empty-small extra-empty"');
  });

  it('passes through old Empty props for image, description, imageStyle, and footer', () => {
    const html = renderToStaticMarkup(
      <LegacyEmpty
        data-kind="empty"
        description={null}
        image="/empty.png"
        imageStyle={{ height: 20 }}
        prefixCls="custom-empty"
      >
        <button type="button">创建</button>
      </LegacyEmpty>,
    );

    expect(html).toContain('class="custom-empty"');
    expect(html).toContain('data-kind="empty"');
    expect(html).toContain('class="custom-empty-image" style="height:20px"');
    expect(html).toContain('<img alt="empty" src="/empty.png"/>');
    expect(html).not.toContain('custom-empty-description');
    expect(html).toContain('class="custom-empty-footer"');
    expect(html).toContain('<button type="button">创建</button>');
  });

  it('does not fall back to a default image when image is explicitly null', () => {
    const html = renderToStaticMarkup(<LegacyEmpty image={null} />);

    expect(html).toContain('class="ant-empty"');
    expect(html).not.toContain('ant-empty-normal');
    expect(html).not.toContain('<svg');
    expect(html).toContain('暂无数据');
  });
});
