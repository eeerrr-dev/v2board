import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

describe('legacy antd table CSS', () => {
  const css = () => readFileSync('src/styles/globals.css', 'utf8');

  it('keeps table and pagination layout rules aligned with the packaged theme', () => {
    const globals = css();

    expect(globals).toContain('.ant-table-wrapper {\n  zoom: 1;');
    expect(globals).toContain(
      ".ant-table-wrapper::before,\n.ant-table-wrapper::after {\n  display: table;\n  content: '';",
    );
    expect(globals).toContain('.ant-table {\n  box-sizing: border-box;\n  position: relative;\n  clear: both;');
    expect(globals).toContain(
      '.ant-table-content {\n  position: relative;\n  border-radius: 4px 4px 0 0;\n}',
    );
    expect(globals).toContain('.ant-table-scroll {\n  overflow: auto;\n  overflow-x: hidden;');
    expect(globals).toContain('.ant-table-body-inner {\n  height: 100%;\n}');
    expect(globals).toContain('.ant-table-thead > tr > th {\n  color: rgba(0, 0, 0, 0.85);');
    expect(globals).toContain(
      '.ant-table-thead > tr > th .ant-table-header-column {\n  display: inline-block;\n  max-width: 100%;\n  vertical-align: top;\n}',
    );
    expect(globals).toContain('background: #fafafa;\n  border-bottom: 1px solid #e8e8e8;');
    expect(globals).toContain('.ant-table-thead > tr > th {\n  background: #fff !important;\n}');
    expect(globals.lastIndexOf('.ant-table-thead > tr > th {\n  background: #fff !important;\n}')).toBeGreaterThan(
      globals.indexOf('background: #fafafa;\n  border-bottom: 1px solid #e8e8e8;'),
    );
    expect(globals).toContain(
      '.ant-table-tbody > tr.ant-table-row-hover:not(.ant-table-expanded-row):not(.ant-table-row-selected) > td,\n.ant-table-tbody > tr:hover:not(.ant-table-expanded-row):not(.ant-table-row-selected) > td,',
    );
    expect(globals).toContain('background: #e6f6ff;');
    expect(globals).toContain('.ant-table-fixed-right {\n  position: absolute;\n  top: 0;\n  right: 0;\n  z-index: auto;');
    expect(globals).toContain('.ant-table-pagination.ant-pagination {\n  float: right;\n  margin: 16px !important;');
  });
});

describe('legacy themed antd CSS overrides', () => {
  const css = () => readFileSync('src/styles/globals.css', 'utf8');

  it('preserves the packaged default theme overrides for dropdowns, pagination, and tags', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-select-dropdown-menu-item:hover:not(.ant-select-dropdown-menu-item-disabled),\n.ant-select-dropdown-menu-item-active:not(.ant-select-dropdown-menu-item-disabled) {\n  background-color: #e6f6ff;',
    );
    expect(globals).toContain(
      '.ant-pagination-jump-prev .ant-pagination-item-container .ant-pagination-item-link-icon,\n.ant-pagination-jump-next .ant-pagination-item-container .ant-pagination-item-link-icon {\n  display: inline-block;\n  color: var(--legacy-ant-primary);',
    );
    expect(globals).toContain('.ant-dropdown-menu-item:hover {\n  background-color: #e6f6ff;');
    expect(globals).toContain('.ant-input-group-addon {\n  position: relative;\n  display: table-cell;\n  width: 1px;\n  padding: 0 11px;');
    expect(globals).toContain('.ant-select-lg {\n  font-size: 16px;\n}');
    expect(globals).toContain('.ant-select-lg .ant-select-selection--single {\n  height: 40px;\n}');
    expect(globals).toContain(
      '.ant-select-dropdown--empty.ant-select-dropdown--multiple .ant-select-dropdown-menu-item {\n  padding-right: 12px;\n}',
    );
    expect(globals).toContain(
      '.ant-select-selection__rendered {\n  position: relative;\n  display: block;\n  margin-right: 11px;\n  margin-left: 11px;\n  line-height: 30px;\n}',
    );
    expect(globals).toContain(
      '.ant-select-selection--single .ant-select-selection__rendered {\n  margin-right: 24px;\n}',
    );
    expect(globals).toContain('.ant-tag {\n  display: inline-block;\n  height: auto;\n  margin-right: 8px;\n  padding: 0 7px;');
    expect(globals).toContain('font-size: 12px;\n  line-height: 1.5;\n  white-space: nowrap;');
    expect(globals).toContain('.ant-tag:last-child {\n  margin: 0;\n}');
    expect(globals).toContain('.ant-tabs-bar {\n  margin-bottom: 0;\n}');
    expect(globals.lastIndexOf('.ant-tag:last-child {\n  margin: 0;\n}')).toBeGreaterThan(
      globals.indexOf('.block-header.plan {\n  background-color: #fff !important;\n}'),
    );
    expect(globals).not.toContain('  text-align: center;\n  background: #fafafa;');
  });
});

describe('legacy antd button and radio CSS', () => {
  const css = () => readFileSync('src/styles/globals.css', 'utf8');

  it('keeps button variant rules that would otherwise be overwritten by the app bundle', () => {
    const globals = css();

    expect(globals).toContain('.ant-btn-link {\n  color: var(--legacy-ant-primary);');
    expect(globals).toContain('font-weight: 400;\n  line-height: 1.5;\n  white-space: nowrap;');
    expect(globals).toContain('background-color: transparent;\n  border-color: transparent;\n  box-shadow: none;');
    expect(globals).toContain(
      '.ant-btn-link:active {\n  color: var(--legacy-ant-active);\n  background-color: transparent;\n  border-color: transparent;',
    );
    expect(globals).toContain('.ant-btn-background-ghost {\n  color: #fff;');
    expect(globals).toContain('background: transparent !important;\n  border-color: #fff;');
    expect(globals).toContain('.ant-btn-icon-only {\n  width: 32px;\n  height: 32px;\n  padding: 0;');
    expect(globals).toContain('.ant-btn-round {\n  height: 32px;\n  padding: 0 16px;');
    expect(globals).toContain('.ant-btn-circle,\n.ant-btn-circle-outline {\n  min-width: 32px;');
    expect(globals).toContain(
      '.ant-btn-group .ant-btn-primary:not(:first-child):not(:last-child) {\n  border-right-color: var(--legacy-ant-hover);',
    );
    expect(globals).toContain(
      '.ant-btn-group .ant-btn-primary + .ant-btn:not(.ant-btn-primary):not([disabled]) {\n  border-left-color: transparent;',
    );
  });

  it('uses the packaged theme focus opacity for radio buttons separately from native radios', () => {
    const globals = css();

    expect(globals).toContain(
      '--legacy-ant-radio-focus-shadow: rgba(6, 101, 208, 0.08);',
    );
    expect(globals).toContain(
      '--legacy-ant-radio-button-focus-shadow: rgba(6, 101, 208, 0.06);',
    );
    expect(globals).toContain(
      '.ant-radio-button-wrapper:focus-within {\n  outline: 3px solid var(--legacy-ant-radio-button-focus-shadow);',
    );
    expect(globals).toContain(
      '.ant-radio {\n  box-sizing: border-box;\n  position: relative;\n  display: inline-block;\n  margin: 0;\n  padding: 0;\n  color: rgba(0, 0, 0, 0.65);\n  font-size: 14px;\n  font-variant: tabular-nums;\n  line-height: 1.5;',
    );
    expect(globals).toContain(
      '.ant-radio-input:focus + .ant-radio-inner {\n  box-shadow: 0 0 0 3px var(--legacy-ant-radio-focus-shadow);',
    );
  });
});

describe('legacy OneUI utility and form CSS', () => {
  const css = () => readFileSync('src/styles/globals.css', 'utf8');

  it('keeps packaged Bootstrap utility and button focus rules intact', () => {
    const globals = css();

    expect(globals).toContain('.text-light { color: #f8f9fa !important; }');
    expect(globals).toContain('animation: spinner-grow 0.75s linear infinite;');
    expect(globals).toContain(
      '.btn:hover {\n  color: #495057;\n  text-decoration: none;\n}',
    );
    expect(globals).toContain(
      '.btn:focus {\n  outline: 0;\n  box-shadow: 0 0 0 0.2rem rgba(6, 101, 208, 0.25);\n}',
    );
    expect(globals).not.toContain('.btn:hover,\n.btn:focus');
    expect(globals).toContain(
      '.block {\n  margin-bottom: 1.75rem;\n  background-color: #fff;\n  box-shadow: var(--shadow-block);\n}',
    );
    expect(globals).toContain('.block.block-rounded {\n  border-radius: 0.25rem;\n}');
    expect(globals).toContain(
      '.block.block-transparent {\n  background-color: transparent;\n  box-shadow: none;\n}',
    );
    expect(globals).toContain(
      '.block.block-fx-pop {\n  box-shadow: 0 0.5rem 2rem var(--legacy-block-pop-shadow);\n  opacity: 1;\n}',
    );
    expect(globals).toContain(
      'a.block {\n  display: block;\n  color: #495057;\n  font-weight: 400;\n  transition:',
    );
    expect(globals).toContain(
      'a.block:hover {\n  color: #495057;\n  opacity: 0.65;\n}',
    );
  });

  it('matches the packaged custom control and badge cascade', () => {
    const globals = css();
    const badge = globals.match(/\.badge \{[\s\S]*?\n\}/)?.[0] ?? '';

    expect(globals).toContain(
      ".custom-control-label::before {\n  position: absolute;\n  top: 0.25rem;\n  left: -1.5rem;\n  display: block;\n  width: 1rem;\n  height: 1rem;\n  pointer-events: none;\n  content: '';\n  background-color: #e2e8f2;\n  border: none;\n}",
    );
    expect(badge).not.toContain('color: #fff;');
    expect(globals).toContain(
      '.badge-danger {\n  color: #fff;\n  background-color: #e04f1a;\n}',
    );
  });

  it('keeps the packaged sidebar transition gate for the OneUI shell', () => {
    const globals = css();

    expect(globals).toContain(
      '#page-header .content-header {\n  padding-right: 0.875rem;\n  padding-left: 0.875rem;\n}',
    );
    expect(globals).toContain(
      '@media (min-width: 768px) {\n  #page-header .content-header {\n    padding-right: 1.75rem;\n    padding-left: 1.75rem;\n  }\n}',
    );
    expect(globals).toContain(
      '@media (max-width: 768px) {\n  #page-header .content-header {\n    padding: 0 !important;\n  }\n}',
    );
    expect(globals).toContain(
      '#page-container.page-header-fixed #main-container {\n  padding-top: 3.25rem;\n}',
    );
    expect(globals).toContain(
      '.content.content-full {\n  padding-bottom: 0.875rem;\n}',
    );
    expect(globals).toContain(
      '.block-content.block-content-full {\n  padding-bottom: 1.25rem;\n}',
    );
    expect(globals).toContain(
      '#page-header {\n  position: relative;\n  width: 100%;\n  margin: 0 auto;\n  background-color: #fff;\n}',
    );
    expect(globals).toContain(
      '#page-container.page-header-fixed #page-header {\n  position: fixed;\n  top: 0;\n  right: 0;\n  left: 0;\n  width: auto;\n  min-width: 320px;\n  max-width: 100%;\n  z-index: 1030;',
    );
    expect(globals).toContain(
      '#page-container.page-header-fixed #page-header,\n#page-container.page-header-glass #page-header {\n  z-index: 998;\n}',
    );
    expect(globals).toContain(
      '.side-trans-enabled #sidebar {\n  transition: transform 0.45s cubic-bezier(0.2, 0.61, 0.42, 0.97);\n}',
    );
    expect(globals).toContain(
      '.side-trans-enabled #side-overlay {\n  transition:\n    transform 0.45s cubic-bezier(0.2, 0.61, 0.42, 0.97),\n    opacity 0.45s cubic-bezier(0.2, 0.61, 0.42, 0.97);\n}',
    );
    expect(globals).toContain('.dropdown-menu {\n  position: absolute;\n  top: 100%;\n  left: 0;\n  z-index: 1000;');
    expect(globals).toContain('min-width: 12rem;\n  padding: 0.5rem 0;\n  margin: 0.125rem 0 0;');
  });

  it('keeps packaged OneUI row, block, and pull spacing helpers', () => {
    const globals = css();

    expect(globals).toContain(
      '.row.gutters-tiny {\n  margin-right: -0.125rem;\n  margin-left: -0.125rem;\n}',
    );
    expect(globals).toContain(
      '.row.gutters-tiny .block,\n.row.gutters-tiny.items-push > div,\n.row.gutters-tiny .push {\n  margin-bottom: 0.25rem;\n}',
    );
    expect(globals).toContain(
      '.row.row-deck > div {\n  display: flex;\n  align-items: stretch;\n}',
    );
    expect(globals).toContain('.row.row-deck > div > .block {\n  min-width: 100%;\n}');
    expect(globals).toContain(
      '.block .block,\n.content-side .block {\n  box-shadow: none;\n}',
    );
    expect(globals).toContain(
      '.block-content > .pull {\n  margin: -1.25rem -1.25rem -1px;\n}',
    );
    expect(globals).toContain(
      '.block-content.block-content-full > .pull,\n.block-content.block-content-full > .pull-b,\n.block-content.block-content-full > .pull-y {\n  margin-bottom: -1.25rem;\n}',
    );
    expect(globals).toContain(
      '.block-content .block,\n.block-content .items-push > div,\n.block-content .push,\n.block-content p {\n  margin-bottom: 1.25rem;\n}',
    );
    expect(globals).toContain(
      '.content > .pull {\n  margin: -0.875rem -0.875rem -1px;\n}',
    );
    expect(globals).toContain(
      '.content.content-full > .pull,\n.content.content-full > .pull-b,\n.content.content-full > .pull-y {\n  margin-bottom: -0.875rem;\n}',
    );
    expect(globals).toContain(
      '.content .block,\n.content .items-push > div,\n.content .push,\n.content p {\n  margin-bottom: 0.875rem;\n}',
    );
    expect(globals).toContain(
      '.content-side > .pull {\n  margin: -1.125rem -1.125rem -1px;\n}',
    );
    expect(globals).toContain(
      '.content-side .block,\n.content-side .items-push > div,\n.content-side .push,\n.content-side p {\n  margin-bottom: 1.125rem;\n}',
    );
    expect(
      globals.indexOf('.block-content .block,\n.block-content .items-push > div'),
    ).toBeGreaterThan(globals.indexOf('.content .block,\n.content .items-push > div'));
  });
});

describe('legacy antd feedback CSS', () => {
  const css = () => readFileSync('src/styles/globals.css', 'utf8');

  it('keeps badge status layout aligned with the packaged cascade', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-badge {\n  box-sizing: border-box;\n  position: relative;\n  display: inline-block;\n  margin: 0;\n  padding: 0;\n  color: rgba(0, 0, 0, 0.65);\n  font-size: 14px;\n  font-variant: tabular-nums;\n  line-height: 1.5;',
    );
    expect(globals).toContain('.ant-badge-status {\n  line-height: inherit;\n  vertical-align: baseline;\n}');
    expect(globals).toContain(
      '.ant-badge-status-processing::after {\n  position: absolute;\n  top: 0;\n  left: 0;\n  width: 100%;\n  height: 100%;',
    );
    expect(globals).toContain(
      '.ant-badge-not-a-wrapper .ant-badge-count {\n  transform: none;\n}',
    );
  });

  it('keeps tooltip bubble, arrow, and zoom-big-fast motion rules from antd v3', () => {
    const globals = css();

    expect(globals).toContain('.ant-tooltip {\n  box-sizing: border-box;\n  position: absolute;\n  z-index: 1060;');
    expect(globals).toContain(
      '.ant-tooltip-inner {\n  min-width: 30px;\n  min-height: 32px;\n  padding: 6px 8px;\n  color: #fff;',
    );
    expect(globals).toContain(
      '.ant-tooltip-placement-topRight .ant-tooltip-arrow {\n  right: 13px;\n}',
    );
    expect(globals).toContain('@keyframes antZoomBigIn {');
    expect(globals).toContain(
      '.zoom-big-fast-enter.zoom-big-fast-enter-active,\n.zoom-big-fast-appear.zoom-big-fast-appear-active {\n  animation-name: antZoomBigIn;\n  animation-play-state: running;\n}',
    );
    expect(globals).toContain(
      '.zoom-big-fast-leave.zoom-big-fast-leave-active {\n  animation-name: antZoomBigOut;\n  animation-play-state: running;\n  pointer-events: none;\n}',
    );
  });
});
