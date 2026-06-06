import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  LegacyCaretDownIcon,
  LegacyCaretUpIcon,
  LegacyCopyIcon,
  LegacyDatabaseIcon,
  LegacyDeleteIcon,
  LegacyEditIcon,
  LegacyFilterIcon,
  LegacyFormIcon,
  LegacyLeftIcon,
  LegacyPlusIcon,
  LegacyQuestionCircleIcon,
  LegacyRightIcon,
  LegacyUserIcon,
} from './legacy-ant-icon';

describe('LegacyPlusIcon', () => {
  it('renders the old Ant Design plus icon DOM used by the bundled admin theme', () => {
    const html = renderToStaticMarkup(<LegacyPlusIcon />);

    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('class="anticon anticon-plus"');
    expect(html).toContain('class=""');
    expect(html).toContain('data-icon="plus"');
    expect(html).toContain('d="M482 152h60q8 0 8 8v704q0 8-8 8h-60q-8 0-8-8V160q0-8 8-8z"');
    expect(html).toContain('d="M176 474h672q8 0 8 8v60q0 8-8 8H176q-8 0-8-8v-60q0-8 8-8z"');
    expect(html).not.toContain('role="img"');
    expect(html).not.toContain('M192 474');
  });

  it('renders the old Ant Design table helper icons', () => {
    const html = renderToStaticMarkup(
      <>
        <LegacyFilterIcon title="筛选" tabIndex={-1} className="ant-dropdown-trigger" />
        <LegacyQuestionCircleIcon />
        <LegacyLeftIcon />
        <LegacyRightIcon />
        <LegacyCaretUpIcon className="ant-table-column-sorter-up off" />
        <LegacyCaretDownIcon className="ant-table-column-sorter-down off" />
        <LegacyEditIcon />
        <LegacyFormIcon />
        <LegacyCopyIcon />
        <LegacyDeleteIcon />
        <LegacyUserIcon style={{ cursor: 'move' }} />
        <LegacyDatabaseIcon style={{ cursor: 'move' }} />
      </>,
    );

    expect(html).toContain('aria-label="图标: filter"');
    expect(html).toContain('class="anticon anticon-filter ant-dropdown-trigger"');
    expect(html).toContain(
      'd="M880.1 154H143.9c-24.5 0-39.8 26.7-27.5 48L349 597.4V838c0 17.7 14.2 32 31.8 32h262.4c17.6 0 31.8-14.3 31.8-32V597.4L907.7 202c12.2-21.3-3.1-48-27.6-48zM603.4 798H420.6V642h182.9v156zm9.6-236.6l-9.5 16.6h-183l-9.5-16.6L212.7 226h598.6L613 561.4z"',
    );
    expect(html).toContain('data-icon="question-circle"');
    expect(html).toContain('aria-label="图标: left"');
    expect(html).toContain('aria-label="图标: right"');
    expect(html).toContain('class="anticon anticon-caret-up ant-table-column-sorter-up off"');
    expect(html).toContain('class="anticon anticon-caret-down ant-table-column-sorter-down off"');
    expect(html).toContain('aria-label="图标: edit"');
    expect(html).toContain('aria-label="图标: form"');
    expect(html).toContain('aria-label="图标: copy"');
    expect(html).toContain('aria-label="图标: delete"');
    expect(html).toContain('aria-label="图标: user"');
    expect(html).toContain('class="anticon anticon-user" style="cursor:move"');
    expect(html).toContain('aria-label="图标: database"');
    expect(html).toContain('class="anticon anticon-database" style="cursor:move"');
    expect(html).not.toContain('role="img"');
  });
});
