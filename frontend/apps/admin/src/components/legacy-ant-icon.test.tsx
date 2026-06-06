import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  LegacyAccountBookIcon,
  LegacyCaretDownIcon,
  LegacyCaretUpIcon,
  LegacyCopyIcon,
  LegacyDatabaseIcon,
  LegacyDeleteIcon,
  LegacyDownIcon,
  LegacyEditIcon,
  LegacyFileExcelIcon,
  LegacyFilterIcon,
  LegacyFormIcon,
  LegacyInfoCircleIcon,
  LegacyLeftIcon,
  LegacyLoadingIcon,
  LegacyMailIcon,
  LegacyPlusIcon,
  LegacyQuestionCircleIcon,
  LegacyReloadIcon,
  LegacyRightIcon,
  LegacySelectIcon,
  LegacySolutionIcon,
  LegacyStopIcon,
  LegacyUserIcon,
  LegacyUserAddIcon,
  LegacyUsergroupAddIcon,
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
        <LegacySelectIcon />
        <LegacyUserAddIcon />
        <LegacyQuestionCircleIcon />
        <LegacyInfoCircleIcon />
        <LegacyLoadingIcon />
        <LegacyLeftIcon />
        <LegacyRightIcon />
        <LegacyDownIcon className="ant-select-arrow-icon" />
        <LegacyCaretUpIcon className="ant-table-column-sorter-up off" />
        <LegacyCaretDownIcon className="ant-table-column-sorter-down off" />
        <LegacyEditIcon />
        <LegacyFormIcon />
        <LegacyCopyIcon />
        <LegacyReloadIcon />
        <LegacyAccountBookIcon />
        <LegacyUsergroupAddIcon />
        <LegacySolutionIcon />
        <LegacyFileExcelIcon />
        <LegacyMailIcon />
        <LegacyStopIcon />
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
    expect(html).toContain('aria-label="图标: select"');
    expect(html).toContain('aria-label="图标: user-add"');
    expect(html).toContain('data-icon="question-circle"');
    expect(html).toContain('aria-label="图标: info-circle"');
    expect(html).toContain('class="anticon anticon-info-circle"');
    expect(html).toContain('data-icon="info-circle"');
    expect(html).toContain(
      'd="M464 336a48 48 0 1096 0 48 48 0 10-96 0zm72 112h-48c-4.4 0-8 3.6-8 8v272c0 4.4 3.6 8 8 8h48c4.4 0 8-3.6 8-8V456c0-4.4-3.6-8-8-8z"',
    );
    expect(html).toContain('aria-label="图标: loading"');
    expect(html).toContain('class="anticon anticon-loading"');
    expect(html).toContain('class="anticon-spin"');
    expect(html).toContain('data-icon="loading"');
    expect(html).toContain(
      'd="M988 548c-19.9 0-36-16.1-36-36 0-59.4-11.6-117-34.6-171.3a440.45 440.45 0 0 0-94.3-139.9 437.71 437.71 0 0 0-139.9-94.3C629 83.6 571.4 72 512 72c-19.9 0-36-16.1-36-36s16.1-36 36-36c69.1 0 136.2 13.5 199.3 40.3C772.3 66 827 103 874 150c47 47 83.9 101.8 109.7 162.7 26.7 63.1 40.2 130.2 40.2 199.3.1 19.9-16 36-35.9 36z"',
    );
    expect(html).toContain('aria-label="图标: left"');
    expect(html).toContain('aria-label="图标: right"');
    expect(html).toContain('class="anticon anticon-down ant-select-arrow-icon"');
    expect(html).toContain('class="anticon anticon-caret-up ant-table-column-sorter-up off"');
    expect(html).toContain('class="anticon anticon-caret-down ant-table-column-sorter-down off"');
    expect(html).toContain('aria-label="图标: edit"');
    expect(html).toContain('aria-label="图标: form"');
    expect(html).toContain('aria-label="图标: copy"');
    expect(html).toContain('aria-label="图标: reload"');
    expect(html).toContain('aria-label="图标: account-book"');
    expect(html).toContain('aria-label="图标: usergroup-add"');
    expect(html).toContain('aria-label="图标: solution"');
    expect(html).toContain('aria-label="图标: file-excel"');
    expect(html).toContain('aria-label="图标: mail"');
    expect(html).toContain('aria-label="图标: stop"');
    expect(html).toContain('aria-label="图标: delete"');
    expect(html).toContain('aria-label="图标: user"');
    expect(html).toContain('class="anticon anticon-user" style="cursor:move"');
    expect(html).toContain('aria-label="图标: database"');
    expect(html).toContain('class="anticon anticon-database" style="cursor:move"');
    expect(html).not.toContain('role="img"');
  });
});
