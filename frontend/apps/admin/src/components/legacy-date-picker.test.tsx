import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LegacyDatePicker } from './legacy-date-picker';

describe('LegacyDatePicker', () => {
  it('renders the old Ant Design date picker input shell', () => {
    const html = renderToStaticMarkup(
      <LegacyDatePicker style={{ width: '100%' }} onChange={() => undefined} />,
    );

    expect(html).toContain(
      '<span class="ant-calendar-picker" style="min-width:195px;width:100%"><div><input readOnly="" placeholder="请选择日期" class="ant-calendar-picker-input ant-input" value=""/><i aria-label="图标: calendar" class="anticon anticon-calendar ant-calendar-picker-icon">',
    );
    expect(html).not.toContain('ant-picker');
    expect(html).not.toContain('css-dev-only');
  });
});
