import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LegacySwitch } from './legacy-switch';

describe('LegacySwitch', () => {
  it('renders the old Ant Design switch classes without v5 hashes', () => {
    const checked = renderToStaticMarkup(<LegacySwitch checked={1} size="small" />);

    expect(checked).toContain(
      '<button type="button" role="switch" aria-checked="true" class="ant-switch-small ant-switch ant-switch-checked">',
    );
    expect(checked).toContain('<span class="ant-switch-inner"></span>');
    expect(checked).not.toContain('css-dev-only-do-not-override');

    const unchecked = renderToStaticMarkup(<LegacySwitch checked={0} />);
    expect(unchecked).toContain(
      '<button type="button" role="switch" aria-checked="false" class="ant-switch">',
    );
    expect(unchecked).not.toContain('ant-switch-small');
    expect(unchecked).not.toContain('ant-switch-checked');
  });

  it('renders the original checked and unchecked inner labels', () => {
    const checked = renderToStaticMarkup(
      <LegacySwitch checked={1} checkedChildren="亮" unCheckedChildren="暗" />,
    );
    const unchecked = renderToStaticMarkup(
      <LegacySwitch checked={0} checkedChildren="亮" unCheckedChildren="暗" />,
    );

    expect(checked).toContain('<span class="ant-switch-inner">亮</span>');
    expect(unchecked).toContain('<span class="ant-switch-inner">暗</span>');
  });
});
