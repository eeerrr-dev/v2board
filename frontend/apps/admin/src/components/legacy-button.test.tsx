import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LegacyButton } from './legacy-button';
import { LegacyPlusIcon } from './legacy-ant-icon';

describe('LegacyButton', () => {
  it('renders the old Ant Design button shell without Ant Design 5 runtime classes', () => {
    const html = renderToStaticMarkup(
      <LegacyButton className="ant-btn">
        <LegacyPlusIcon />
      </LegacyButton>,
    );

    expect(html).toContain('<button type="button" class="ant-btn">');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).not.toContain('css-dev-only-do-not-override');
    expect(html).not.toContain('ant-btn-default');
    expect(html).not.toContain('ant-btn-color-default');
  });

  it('keeps ant-btn before dropdown trigger when rc-dropdown clones the child', () => {
    const html = renderToStaticMarkup(
      <LegacyButton className="ant-dropdown-trigger ant-btn">
        <LegacyPlusIcon />
      </LegacyButton>,
    );

    expect(html).toContain('<button type="button" class="ant-btn ant-dropdown-trigger">');
  });

  it('keeps the bundled auto space behavior for a single two-character Chinese label', () => {
    const html = renderToStaticMarkup(<LegacyButton className="ant-btn">提交</LegacyButton>);

    expect(html).toContain('<span>提 交</span>');
  });

  it('keeps the old primary button attribute order with inline style', () => {
    const html = renderToStaticMarkup(
      <LegacyButton className="ant-btn ant-btn-primary" style={{ float: 'right' }}>
        编辑排序
      </LegacyButton>,
    );

    expect(html).toContain(
      '<button type="button" class="ant-btn ant-btn-primary" style="float:right"><span>编辑排序</span></button>',
    );
  });
});
