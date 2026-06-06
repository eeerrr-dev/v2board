import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  LegacyCaretDownIcon,
  LegacyCaretUpIcon,
  LegacyCopyIcon,
  LegacyDeleteIcon,
  LegacyFilterIcon,
  LegacyFormIcon,
  LegacyPlusIcon,
  LegacyQuestionCircleIcon,
} from './legacy-ant-icon';

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

  it('renders the old Ant Design table helper icons', () => {
    const html = renderToStaticMarkup(
      <>
        <LegacyFilterIcon title="筛选" tabIndex={-1} className="ant-dropdown-trigger" />
        <LegacyQuestionCircleIcon />
        <LegacyCaretUpIcon className="ant-table-column-sorter-up off" />
        <LegacyCaretDownIcon className="ant-table-column-sorter-down off" />
        <LegacyFormIcon />
        <LegacyCopyIcon />
        <LegacyDeleteIcon />
      </>,
    );

    expect(html).toContain('aria-label="图标: filter"');
    expect(html).toContain('class="anticon anticon-filter ant-dropdown-trigger"');
    expect(html).toContain('data-icon="question-circle"');
    expect(html).toContain('class="anticon anticon-caret-up ant-table-column-sorter-up off"');
    expect(html).toContain('class="anticon anticon-caret-down ant-table-column-sorter-down off"');
    expect(html).toContain('aria-label="图标: form"');
    expect(html).toContain('aria-label="图标: copy"');
    expect(html).toContain('aria-label="图标: delete"');
    expect(html).not.toContain('role="img"');
  });
});
