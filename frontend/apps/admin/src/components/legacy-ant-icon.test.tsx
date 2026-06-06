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
  LegacyLinkIcon,
  LegacyLoadingIcon,
  LegacyMailIcon,
  LegacyPlusIcon,
  LegacyQuestionCircleIcon,
  LegacyReadIcon,
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
        <LegacyLinkIcon />
        <LegacyReadIcon />
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
    expect(html).toContain('aria-label="图标: link"');
    expect(html).toContain('class="anticon anticon-link"');
    expect(html).toContain('data-icon="link"');
    expect(html).toContain(
      'd="M574 665.4a8.03 8.03 0 00-11.3 0L446.5 781.6c-53.8 53.8-144.6 59.5-204 0-59.5-59.5-53.8-150.2 0-204l116.2-116.2c3.1-3.1 3.1-8.2 0-11.3l-39.8-39.8a8.03 8.03 0 00-11.3 0L191.4 526.5c-84.6 84.6-84.6 221.5 0 306s221.5 84.6 306 0l116.2-116.2c3.1-3.1 3.1-8.2 0-11.3L574 665.4zm258.6-474c-84.6-84.6-221.5-84.6-306 0L410.3 307.6a8.03 8.03 0 000 11.3l39.7 39.7c3.1 3.1 8.2 3.1 11.3 0l116.2-116.2c53.8-53.8 144.6-59.5 204 0 59.5 59.5 53.8 150.2 0 204L665.3 562.6a8.03 8.03 0 000 11.3l39.8 39.8c3.1 3.1 8.2 3.1 11.3 0l116.2-116.2c84.5-84.6 84.5-221.5 0-306.1zM610.1 372.3a8.03 8.03 0 00-11.3 0L372.3 598.7a8.03 8.03 0 000 11.3l39.6 39.6c3.1 3.1 8.2 3.1 11.3 0l226.4-226.4c3.1-3.1 3.1-8.2 0-11.3l-39.5-39.6z"',
    );
    expect(html).toContain('aria-label="图标: read"');
    expect(html).toContain('class="anticon anticon-read"');
    expect(html).toContain('data-icon="read"');
    expect(html).toContain(
      'd="M928 161H699.2c-49.1 0-97.1 14.1-138.4 40.7L512 233l-48.8-31.3A255.2 255.2 0 00324.8 161H96c-17.7 0-32 14.3-32 32v568c0 17.7 14.3 32 32 32h228.8c49.1 0 97.1 14.1 138.4 40.7l44.4 28.6c1.3.8 2.8 1.3 4.3 1.3s3-.4 4.3-1.3l44.4-28.6C602 807.1 650.1 793 699.2 793H928c17.7 0 32-14.3 32-32V193c0-17.7-14.3-32-32-32zM324.8 721H136V233h188.8c35.4 0 69.8 10.1 99.5 29.2l48.8 31.3 6.9 4.5v462c-47.6-25.6-100.8-39-155.2-39zm563.2 0H699.2c-54.4 0-107.6 13.4-155.2 39V298l6.9-4.5 48.8-31.3c29.7-19.1 64.1-29.2 99.5-29.2H888v488zM396.9 361H211.1c-3.9 0-7.1 3.4-7.1 7.5v45c0 4.1 3.2 7.5 7.1 7.5h185.7c3.9 0 7.1-3.4 7.1-7.5v-45c.1-4.1-3.1-7.5-7-7.5zm223.1 7.5v45c0 4.1 3.2 7.5 7.1 7.5h185.7c3.9 0 7.1-3.4 7.1-7.5v-45c0-4.1-3.2-7.5-7.1-7.5H627.1c-3.9 0-7.1 3.4-7.1 7.5zM396.9 501H211.1c-3.9 0-7.1 3.4-7.1 7.5v45c0 4.1 3.2 7.5 7.1 7.5h185.7c3.9 0 7.1-3.4 7.1-7.5v-45c.1-4.1-3.1-7.5-7-7.5zm416 0H627.1c-3.9 0-7.1 3.4-7.1 7.5v45c0 4.1 3.2 7.5 7.1 7.5h185.7c3.9 0 7.1-3.4 7.1-7.5v-45c.1-4.1-3.1-7.5-7-7.5z"',
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
