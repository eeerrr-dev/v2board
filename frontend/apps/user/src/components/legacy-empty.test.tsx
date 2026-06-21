import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { LegacyEmpty } from './legacy-empty';
import { readUserStyles } from '../test/read-user-styles';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ i18n: { language: 'en-US' } }),
}));

describe('LegacyEmpty antd class names', () => {
  it('renders table/list empty with ant-empty-normal', () => {
    const html = renderToStaticMarkup(<LegacyEmpty />);

    expect(html).toContain('class="ant-empty ant-empty-normal"');
  });

  it('renders select empty with both normal and small classes', () => {
    const html = renderToStaticMarkup(<LegacyEmpty size="small" />);

    expect(html).toContain('class="ant-empty ant-empty-normal ant-empty-small"');
  });

  it('keeps the legacy antd Empty spacing and image heights in CSS', () => {
    const css = readUserStyles();

    expect(css).toContain('.ant-empty {\n  margin: 0 8px;');
    expect(css).toContain('.ant-empty-normal .ant-empty-image {\n  height: 40px;');
    expect(css).toContain('.ant-empty-small {\n  margin: 8px 0;');
    expect(css).toContain('.ant-empty-small .ant-empty-image {\n  height: 35px;');
  });
});
