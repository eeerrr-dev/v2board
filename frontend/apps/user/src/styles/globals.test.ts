import { describe, expect, it } from 'vitest';
import { readUserStyles } from '../test/read-user-styles';

const css = readUserStyles;

describe('legacy custom HTML content CSS', () => {
  it('keeps rich announcement and knowledge article content aligned with the packaged theme', () => {
    const globals = css();

    expect(globals).toContain('.custom-html-style {\n  color: #333;\n}');
    expect(globals).toContain(
      '.custom-html-style h1 {\n  font-size: 32px;\n  padding: 0;\n  border: none;',
    );
    expect(globals).toContain(
      '.custom-html-style h2 {\n  font-size: 24px;\n  padding: 0;\n  border: none;',
    );
    expect(globals).toContain(
      '.custom-html-style h3 {\n  font-size: 18px;\n  margin: 18px 0;\n  padding: 0;',
    );
    expect(globals).toContain('.custom-html-style p {\n  font-size: 14px;\n  line-height: 1.7;');
    expect(globals).toContain('.custom-html-style a {\n  color: #0052d9;\n}');
    expect(globals).toContain('.custom-html-style a:hover {\n  text-decoration: none;\n}');
    expect(globals).toContain('.custom-html-style strong {\n  font-weight: 700;\n}');
    expect(globals).toContain(
      '.custom-html-style ol,\n.custom-html-style ul {\n  font-size: 14px;\n  line-height: 28px;',
    );
    expect(globals).toContain('.custom-html-style li {\n  margin-bottom: 8px;\n  line-height: 1.7;');
    expect(globals).toContain(
      '.custom-html-style hr {\n  margin-top: 20px;\n  margin-bottom: 20px;\n  border: 0;',
    );
    expect(globals).toContain(
      '.custom-html-style pre {\n  display: block;\n  background-color: #f5f5f5;\n  padding: 20px;',
    );
    expect(globals).toContain(
      '.custom-html-style code {\n  background-color: #f5f5f5;\n  border-radius: 0;',
    );
    expect(globals).toContain(
      '.custom-html-style code::after,\n.custom-html-style code::before {\n  letter-spacing: 0;\n}',
    );
    expect(globals).toContain(
      '.custom-html-style blockquote {\n  position: relative;\n  margin: 16px 0;\n  padding: 5px 8px 5px 30px;',
    );
    expect(globals).toContain('.custom-html-style img,\n.custom-html-style video {\n  max-width: 100%;\n}');
    expect(globals).toContain(
      '.custom-html-style table {\n  font-size: 14px;\n  line-height: 1.7;\n  max-width: 100%;',
    );
    expect(globals).toContain(
      '.custom-html-style table td,\n.custom-html-style table th {\n  word-break: break-all;',
    );
    expect(globals).toContain('.custom-html-style table tr {\n  border: 1px solid #efefef;\n}');
    expect(globals).toContain(
      '.custom-html-style table th {\n  text-align: center;\n  font-weight: 700;',
    );
    expect(globals).toContain(
      '.custom-html-style table td {\n  border: 1px solid #efefef;\n  text-align: left;\n  padding: 10px 15px;',
    );
    expect(globals.indexOf('.custom-html-style {\n  color: #333;')).toBeLessThan(
      globals.indexOf('.custom-html-style h1 {'),
    );
    expect(globals.indexOf('.custom-html-style h1 {')).toBeLessThan(
      globals.indexOf('.custom-html-style p {'),
    );
    expect(globals.indexOf('.custom-html-style p {')).toBeLessThan(
      globals.indexOf('.custom-html-style ol,\n.custom-html-style ul {'),
    );
    expect(globals.indexOf('.custom-html-style hr {')).toBeLessThan(
      globals.indexOf('.custom-html-style pre {'),
    );
    expect(globals.indexOf('.custom-html-style pre {')).toBeLessThan(
      globals.indexOf('.custom-html-style code {'),
    );
    expect(globals.indexOf('.custom-html-style code::after,')).toBeLessThan(
      globals.indexOf('.custom-html-style blockquote {'),
    );
    expect(globals.indexOf('.custom-html-style blockquote {')).toBeLessThan(
      globals.indexOf('.custom-html-style img,\n.custom-html-style video {'),
    );
    expect(globals.indexOf('.custom-html-style img,\n.custom-html-style video {')).toBeLessThan(
      globals.indexOf('.custom-html-style table {'),
    );
    expect(globals.indexOf('.custom-html-style table {')).toBeLessThan(
      globals.indexOf('.custom-html-style table td,\n.custom-html-style table th {'),
    );
    expect(globals.indexOf('.custom-html-style table tr:nth-child(2n)')).toBeLessThan(
      globals.indexOf('.custom-html-style table th {\n  text-align: center;'),
    );
    expect(globals.indexOf('.custom-html-style table th {\n  text-align: center;')).toBeLessThan(
      globals.indexOf('.custom-html-style table td {'),
    );
  });
});

describe('legacy antd table CSS', () => {
  it('keeps table and pagination layout rules aligned with the packaged theme', () => {
    const globals = css();

    expect(globals).toContain('.ant-table-wrapper {\n  zoom: 1;');
    expect(globals).toContain(
      ".ant-table-wrapper::before,\n.ant-table-wrapper::after {\n  display: table;\n  content: '';",
    );
    expect(globals).toContain(
      '.ant-table {\n  box-sizing: border-box;\n  position: relative;\n  clear: both;',
    );
    expect(globals).toContain('  font-family: menlo !important;\n  font-variant: tabular-nums;');
    expect(globals).toContain('.ant-table-body {\n  transition: opacity 0.3s;\n}');
    expect(globals).toContain(
      '.ant-table-empty .ant-table-body {\n  overflow-x: auto !important;\n  overflow-y: hidden !important;\n}',
    );
    expect(globals).toContain(
      '.ant-table-content {\n  position: relative;\n  border-radius: 4px 4px 0 0;\n}',
    );
    expect(globals).toContain('.ant-table-scroll {\n  overflow: auto;\n  overflow-x: hidden;');
    expect(globals).toContain('.ant-table-scroll table {\n  min-width: 100%;\n}');
    expect(globals).not.toContain('.ant-table-fixed {\n  table-layout: fixed;\n}');
    expect(globals.indexOf('.ant-table-wrapper {\n  zoom: 1;')).toBeLessThan(
      globals.indexOf('.ant-table {\n  box-sizing: border-box;'),
    );
    expect(globals.indexOf('.ant-table-empty .ant-table-body {')).toBeLessThan(
      globals.indexOf('.ant-table-content {\n  position: relative;'),
    );
    expect(globals).toContain('.ant-table-body-inner {\n  height: 100%;\n}');
    expect(globals).toContain('.ant-table-body-outer {\n  position: relative;\n}');
    expect(globals).toContain(
      '.ant-table .ant-table-fixed-left table,\n.ant-table .ant-table-fixed-right table {\n  width: auto;\n}',
    );
    expect(globals).toContain(
      '.ant-table-scroll table .ant-table-fixed-columns-in-body:not([colspan]) {\n  color: transparent;\n}',
    );
    expect(globals).toContain(
      '.ant-table-scroll table .ant-table-fixed-columns-in-body:not([colspan]) > * {\n  visibility: hidden;\n}',
    );
    expect(globals).toContain('.ant-table-thead > tr > th {\n  color: rgba(0, 0, 0, 0.85);');
    expect(globals).toContain(
      '.ant-table-thead > tr > th .ant-table-header-column {\n  display: inline-block;\n  max-width: 100%;\n  vertical-align: top;\n}',
    );
    expect(globals).toContain(
      '.ant-table-row-cell-ellipsis,\n.ant-table-row-cell-ellipsis .ant-table-column-title {\n  overflow: hidden;\n  white-space: nowrap;',
    );
    expect(globals).toContain('background: #fafafa;\n  border-bottom: 1px solid #e8e8e8;');
    expect(globals).toContain('.ant-table-thead > tr > th {\n  background: #fff !important;\n}');
    expect(
      globals.lastIndexOf('.ant-table-thead > tr > th {\n  background: #fff !important;\n}'),
    ).toBeGreaterThan(globals.indexOf('background: #fafafa;\n  border-bottom: 1px solid #e8e8e8;'));
    expect(globals).toMatch(
      /\.ant-table-tbody\s+>\s+tr\.ant-table-row-hover:not\(\.ant-table-expanded-row\):not\(\.ant-table-row-selected\)\s+>\s+td,\s*\.ant-table-tbody > tr:hover:not\(\.ant-table-expanded-row\):not\(\.ant-table-row-selected\) > td,/,
    );
    expect(globals).toContain('background: #e6f6ff;');
    expect(globals).toContain('.ant-table-row {\n  transition: background 0.3s;\n}');
    expect(globals).toContain('.ant-table-row-cell-last {\n  border-right: 0;\n}');
    expect(globals).toContain(
      'a[disabled] {\n  color: rgba(0, 0, 0, 0.25);\n  cursor: not-allowed;\n  pointer-events: none;\n}',
    );
    expect(globals).toContain(
      '.ant-table-fixed-right {\n  position: absolute;\n  top: 0;\n  right: 0;\n  z-index: 1;',
    );
    expect(globals).toContain(
      '.ant-table-fixed-left {\n  position: absolute;\n  top: 0;\n  left: 0;\n  z-index: auto;',
    );
    expect(globals).toContain(
      '.ant-table-fixed-left table,\n.ant-table-fixed-right table {\n  width: auto;\n  background: #fff;\n}',
    );
    expect(globals).toContain('.ant-table-fixed-right a[disabled] {\n  pointer-events: auto;\n}');
    expect(globals).toContain(
      '.ant-table.ant-table-scroll-position-left .ant-table-fixed-left,\n.ant-table.ant-table-scroll-position-right .ant-table-fixed-right {\n  box-shadow: none;\n}',
    );
    expect(globals.indexOf('a:not([class]) {')).toBeLessThan(globals.indexOf('a[disabled] {'));
    expect(globals.indexOf('a[disabled] {')).toBeLessThan(
      globals.indexOf('.ant-table-fixed-right a[disabled] {'),
    );
    expect(globals).toContain(
      '.ant-table-pagination.ant-pagination {\n  float: right;\n  margin: 16px !important;',
    );
    expect(globals).toContain(
      '.ant-table-placeholder {\n  position: relative;\n  z-index: 1;\n  margin-top: -1px;',
    );
    expect(globals).toContain(
      '.ant-pagination-item {\n  display: inline-block;\n  min-width: 32px;\n  height: 32px;',
    );
    expect(globals).toContain(
      '.ant-pagination-item a {\n  display: block;\n  padding: 0 6px;\n  color: rgba(0, 0, 0, 0.65);',
    );
    expect(globals).toContain('.ant-pagination-item a:hover {\n  text-decoration: none;\n}');
    expect(globals).toContain(
      '.ant-pagination-item:focus,\n.ant-pagination-item:hover {\n  border-color: var(--legacy-ant-primary);',
    );
    expect(globals).toContain(
      '.ant-pagination-item-active {\n  font-weight: 500;\n  background: #fff;\n  border-color: var(--legacy-ant-primary);',
    );
    expect(globals).toContain(
      '.ant-pagination-item-active:focus,\n.ant-pagination-item-active:hover {\n  border-color: var(--legacy-ant-hover);',
    );
    expect(globals).toContain(
      '.ant-pagination-jump-prev,\n.ant-pagination-jump-next {\n  outline: 0;\n}',
    );
    expect(globals).toContain(
      '.ant-pagination-jump-prev .ant-pagination-item-container,\n.ant-pagination-jump-next .ant-pagination-item-container {\n  position: relative;\n}',
    );
    expect(globals).toContain(
      '.ant-pagination-jump-prev .ant-pagination-item-ellipsis,\n.ant-pagination-jump-next .ant-pagination-item-ellipsis {\n  position: absolute;\n  inset: 0;',
    );
    expect(globals).toContain(
      '.ant-pagination-jump-prev .ant-pagination-item-container .ant-pagination-item-link-icon,\n.ant-pagination-jump-next .ant-pagination-item-container .ant-pagination-item-link-icon {\n  display: inline-block;',
    );
    expect(globals).toContain(
      ':root .ant-pagination-jump-prev .ant-pagination-item-container .ant-pagination-item-link-icon,\n:root .ant-pagination-jump-next .ant-pagination-item-container .ant-pagination-item-link-icon {\n  font-size: 12px;\n}',
    );
    expect(globals).toContain(
      '.ant-pagination-jump-prev:hover .ant-pagination-item-link-icon,\n.ant-pagination-jump-prev:focus .ant-pagination-item-link-icon,',
    );
    expect(globals).toContain(
      '.ant-pagination-jump-prev:hover .ant-pagination-item-ellipsis,\n.ant-pagination-jump-prev:focus .ant-pagination-item-ellipsis,',
    );
    expect(globals.indexOf('.ant-pagination {')).toBeLessThan(
      globals.indexOf('.ant-pagination-item {'),
    );
    expect(globals.indexOf('.ant-pagination-item {')).toBeLessThan(
      globals.indexOf('.ant-pagination-item a {'),
    );
    expect(globals.indexOf('.ant-pagination-item a {')).toBeLessThan(
      globals.indexOf('.ant-pagination-item:focus,\n.ant-pagination-item:hover {'),
    );
    expect(globals.indexOf('.ant-pagination-item-active {')).toBeLessThan(
      globals.indexOf('.ant-pagination-jump-prev,\n.ant-pagination-jump-next {'),
    );
    expect(globals.indexOf('.ant-pagination-jump-prev .ant-pagination-item-ellipsis')).toBeLessThan(
      globals.indexOf(
        '.ant-pagination-jump-prev .ant-pagination-item-container .ant-pagination-item-link-icon',
      ),
    );
    expect(globals).toContain(
      '.ant-pagination-prev .ant-pagination-item-link,\n.ant-pagination-next .ant-pagination-item-link {\n  display: block;\n  height: 100%;\n  font-size: 12px;',
    );
    expect(globals).toContain(
      '.ant-pagination-prev:focus .ant-pagination-item-link,\n.ant-pagination-prev:hover .ant-pagination-item-link,',
    );
    expect(globals).toContain(
      '.ant-pagination-disabled .ant-pagination-item-link,\n.ant-pagination-disabled:focus .ant-pagination-item-link,',
    );
    expect(globals).toContain(
      '.ant-pagination.ant-pagination-disabled .ant-pagination-item {\n  background: #f5f5f5;\n  border-color: #d9d9d9;',
    );
    expect(globals).toContain(
      '.ant-pagination.ant-pagination-disabled\n  .ant-pagination-jump-prev:focus\n  .ant-pagination-item-link-icon,',
    );
    expect(globals).toContain(
      '@media only screen and (max-width: 992px) {\n  .ant-pagination-item-after-jump-prev,\n  .ant-pagination-item-before-jump-next {\n    display: none;',
    );
    expect(globals).toContain(
      '@media only screen and (max-width: 576px) {\n  .ant-pagination-options {\n    display: none;',
    );
    expect(globals).toContain(
      '.ant-pagination-options {\n  display: inline-block;\n  margin-left: 16px;\n  vertical-align: middle;\n}',
    );
    expect(globals).toContain(
      '.ant-pagination-options-quick-jumper input {\n  position: relative;\n  display: inline-block;\n  width: 50px;',
    );
    expect(globals).toContain(
      '.ant-pagination-options-quick-jumper input:focus {\n  border-color: var(--legacy-ant-hover);\n  border-right-width: 1px !important;',
    );
    expect(globals).toContain(
      '.ant-pagination-options-quick-jumper input-sm {\n  height: 24px;\n  padding: 1px 7px;\n}',
    );
    expect(globals).toContain(
      '.ant-pagination-simple .ant-pagination-simple-pager input {\n  box-sizing: border-box;\n  height: 100%;',
    );
    expect(globals).toContain(
      '.ant-pagination.mini .ant-pagination-item:not(.ant-pagination-item-active),\n.ant-pagination.mini .ant-pagination-prev .ant-pagination-item-link,',
    );
    expect(globals).toContain(
      '.ant-pagination.mini .ant-pagination-options-quick-jumper input {\n  width: 44px;\n  height: 24px;',
    );
  });
});

describe('source-owned icon fonts', () => {
  it('loads Font Awesome and Simple Line Icons from frontend source assets', () => {
    const globals = css();

    expect(globals).toContain("url('../assets/fonts/fa-regular-400.ac21cac3.woff2')");
    expect(globals).toContain("url('../assets/fonts/fa-solid-900.d6d8d5da.woff2')");
    expect(globals).toContain("url('../assets/fonts/Simple-Line-Icons.0cb0b9c5.woff2')");
    expect(globals).toContain(
      '.fa,\n.far,\n.fas,\n.si {\n  display: inline-block;\n  font-style: normal;\n  font-variant: normal;\n  line-height: 1;\n  text-rendering: auto;\n  -webkit-font-smoothing: antialiased;',
    );
    expect(globals.indexOf(".far {\n  font-family: 'Font Awesome 5 Free';")).toBeLessThan(
      globals.indexOf(".fa,\n.fas {\n  font-family: 'Font Awesome 5 Free';"),
    );
    expect(globals).toContain(".fa-copy::before { content: '\\f0c5'; }");
    expect(globals).toContain(".fa-wallet::before { content: '\\f555'; }");
    expect(globals).toContain(".si-login::before { content: '\\e066'; }");
    expect(globals).toContain('.btn-sm {\n  padding: 0.25rem 0.5rem;\n  font-size: 0.875rem;');
    expect(globals).toContain('.btn .fa,\n.btn .si {\n  position: relative;\n  top: 1px;\n}');
    expect(globals).toContain('.btn-group-sm > .btn .fa,\n.btn.btn-sm .fa {\n  top: 0;\n}');
    expect(globals).toContain('.btn-lg {\n  padding: 0.5rem 1rem;\n  font-size: 1.25rem;');
    expect(globals).toContain('.btn-rounded {\n  border-radius: 2rem !important;\n}');
    expect(globals.indexOf('.btn-sm {\n  padding: 0.25rem 0.5rem;')).toBeLessThan(
      globals.indexOf('.btn .fa,\n.btn .si {'),
    );
    expect(globals.indexOf('.btn-group-sm > .btn .fa')).toBeLessThan(
      globals.indexOf('.btn-lg {\n  padding: 0.5rem 1rem;'),
    );
    expect(globals.indexOf('.btn-rounded {')).toBeLessThan(
      globals.indexOf('.row {\n  display: flex;'),
    );
    expect(globals).not.toContain('/theme/default/assets/static/');
  });
});

describe('legacy themed antd CSS overrides', () => {
  it('preserves the packaged default theme overrides for dropdowns, pagination, and tags', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-select-dropdown {\n  position: absolute;\n  top: -9999px;\n  left: -9999px;',
    );
    expect(globals).toContain(
      '.ant-select-dropdown-menu {\n  max-height: 250px;\n  margin: 0;\n  padding: 4px 0;',
    );
    expect(globals).toContain(
      '.ant-select-dropdown-menu-item {\n  position: relative;\n  display: block;\n  padding: 5px 12px;',
    );
    expect(globals).toContain(
      '.ant-select-dropdown-menu-item:hover:not(.ant-select-dropdown-menu-item-disabled),\n.ant-select-dropdown-menu-item-active:not(.ant-select-dropdown-menu-item-disabled) {\n  background-color: #e6f6ff;',
    );
    expect(globals).toContain(
      '.ant-select-dropdown-menu-item-selected {\n  color: rgba(0, 0, 0, 0.65);\n  font-weight: 600;',
    );
    expect(globals).toContain(
      '.ant-select-dropdown-menu-item-disabled {\n  color: rgba(0, 0, 0, 0.25);\n  cursor: not-allowed;',
    );
    expect(globals).toContain(
      '.ant-pagination-jump-prev .ant-pagination-item-container .ant-pagination-item-link-icon,\n.ant-pagination-jump-next .ant-pagination-item-container .ant-pagination-item-link-icon {\n  display: inline-block;\n  color: var(--legacy-ant-primary);',
    );
    expect(globals).toContain('.ant-dropdown-menu-item:hover {\n  background-color: #e6f6ff;');
    expect(globals).toContain('.ant-dropdown {\n  box-sizing: border-box;\n  position: absolute;\n  top: -9999px;');
    expect(globals).toContain('@keyframes antSlideDownOut {');
    expect(globals).toContain(
      '.ant-dropdown-placement-topCenter .ant-dropdown-menu.slide-down-leave.slide-down-leave-active {\n  animation-name: antSlideDownOut;\n  animation-play-state: running;',
    );
    expect(globals).toContain(
      '.ant-input {\n  box-sizing: border-box;\n  position: relative;\n  display: inline-block;',
    );
    expect(globals).toContain(
      '.ant-input:focus {\n  border-color: var(--legacy-ant-hover);\n  border-right-width: 1px !important;',
    );
    expect(globals).toContain(
      '.ant-input-disabled,\n.ant-input[disabled] {\n  color: rgba(0, 0, 0, 0.25);',
    );
    expect(globals).toContain(
      'textarea.ant-input {\n  max-width: 100%;\n  height: auto;\n  min-height: 32px;',
    );
    expect(globals).toContain(
      '.ant-input-group > .ant-input:first-child,\n.ant-input-group-addon:first-child {\n  border-top-right-radius: 0;\n  border-bottom-right-radius: 0;\n}',
    );
    expect(globals).toContain('.ant-input-group > .ant-input {\n  display: table-cell;\n}');
    expect(globals).toContain(
      '.ant-input-group-addon {\n  position: relative;\n  display: table-cell;\n  width: 1px;\n  padding: 0 11px;',
    );
    expect(globals).toContain(
      '.ant-input-group > .ant-input:last-child,\n.ant-input-group-addon:last-child {\n  border-top-left-radius: 0;\n  border-bottom-left-radius: 0;\n}',
    );
    expect(globals.indexOf('.ant-input-group > .ant-input:first-child')).toBeLessThan(
      globals.indexOf('.ant-input-group > .ant-input {\n  display: table-cell;'),
    );
    expect(globals.indexOf('.ant-input-group > .ant-input {\n  display: table-cell;')).toBeLessThan(
      globals.indexOf('.ant-input-group-addon {\n  position: relative;'),
    );
    expect(globals.indexOf('.ant-input-group-addon {\n  position: relative;')).toBeLessThan(
      globals.indexOf('.ant-input-group-addon:first-child {\n  border-right: 0;'),
    );
    expect(globals.indexOf('.ant-input-group-addon:last-child {\n  border-left: 0;')).toBeLessThan(
      globals.indexOf('.ant-input-search-enter-button .ant-input {'),
    );
    expect(globals).toContain('.ant-input-search-enter-button .ant-input {\n  border-right: 0;\n}');
    expect(globals).toContain(
      '.ant-input-search-enter-button input + .ant-input-group-addon {\n  padding: 0;\n  border: 0;\n}',
    );
    expect(globals).toContain('.ant-empty {\n  margin: 0 8px;\n  font-size: 14px;');
    expect(globals).toContain('.ant-empty-image {\n  height: 100px;\n  margin-bottom: 8px;\n}');
    expect(globals).toContain('.ant-empty-description {\n  margin: 0;\n}');
    expect(globals).toContain('.ant-empty-normal {\n  margin: 32px 0;\n  color: rgba(0, 0, 0, 0.25);\n}');
    expect(globals).toContain('.ant-empty-small .ant-empty-image {\n  height: 35px;\n}');
    expect(globals.indexOf('.ant-empty {\n  margin: 0 8px;')).toBeLessThan(
      globals.indexOf('.ant-empty-image {\n  height: 100px;'),
    );
    expect(globals.indexOf('.ant-empty-image svg {')).toBeLessThan(
      globals.indexOf('.ant-empty-description {'),
    );
    expect(globals.indexOf('.ant-empty-footer {')).toBeLessThan(
      globals.indexOf('.ant-empty-normal {'),
    );
    expect(globals.indexOf('.ant-empty-small .ant-empty-image {')).toBeLessThan(
      globals.indexOf('.ant-select {'),
    );
    expect(globals).toContain('.input-group {\n  position: relative;\n  display: flex;');
    expect(globals).toContain(
      '.input-group > .form-control {\n  position: relative;\n  flex: 1 1 auto;',
    );
    expect(globals).toContain('.input-group > .form-control:focus {\n  z-index: 3;\n}');
    expect(globals).toContain(
      '.input-group > .form-control:not(:first-child) {\n  border-top-left-radius: 0;',
    );
    expect(globals).toContain('.input-group-prepend {\n  display: flex;\n  margin-right: -1px;\n}');
    expect(globals).toContain('.input-group-prepend .btn {\n  position: relative;\n  z-index: 2;\n}');
    expect(globals).toContain(
      '.input-group > .input-group-prepend > .btn {\n  border-top-right-radius: 0;',
    );
    expect(globals.indexOf('.input-group {\n  position: relative;')).toBeLessThan(
      globals.indexOf('.input-group > .form-control {'),
    );
    expect(globals.indexOf('.input-group > .form-control:not(:first-child)')).toBeLessThan(
      globals.indexOf('.input-group-prepend {'),
    );
    expect(globals.indexOf('.input-group > .input-group-prepend > .btn')).toBeLessThan(
      globals.indexOf('.form-group label {'),
    );
    expect(globals).toContain(
      '.form-group label {\n  display: inline-block;\n  font-weight: 600;\n  margin-bottom: 0.375rem;',
    );
    expect(globals).toContain('.ant-select-lg {\n  font-size: 16px;\n}');
    expect(globals).toContain('.ant-select-lg .ant-select-selection--single {\n  height: 40px;\n}');
    expect(globals).toContain(
      '.ant-select-dropdown--empty.ant-select-dropdown--multiple .ant-select-dropdown-menu-item {\n  padding-right: 12px;\n}',
    );
    expect(globals.indexOf('.ant-select-dropdown {')).toBeLessThan(
      globals.indexOf('.ant-select-dropdown-menu {'),
    );
    expect(globals.indexOf('.ant-select-dropdown-menu {')).toBeLessThan(
      globals.indexOf('.ant-select-dropdown-menu-item {'),
    );
    expect(globals.indexOf('.ant-select-dropdown-menu-item {')).toBeLessThan(
      globals.indexOf('.ant-select-dropdown-menu-item-selected {'),
    );
    expect(globals.indexOf('.ant-select-dropdown-menu-item-selected {')).toBeLessThan(
      globals.indexOf('.ant-select-dropdown--empty.ant-select-dropdown--multiple'),
    );
    expect(globals).toContain(
      '.ant-select-selection__rendered {\n  position: relative;\n  display: block;\n  margin-right: 11px;\n  margin-left: 11px;\n  line-height: 30px;\n}',
    );
    expect(globals).toContain(
      '.ant-select-selection--single .ant-select-selection__rendered {\n  margin-right: 24px;\n}',
    );
    expect(globals).toContain(
      '.ant-select-selection__placeholder {\n  position: absolute;\n  top: 50%;\n  right: 9px;',
    );
    expect(globals).toContain(
      '.ant-select-arrow {\n  display: inline-block;\n  position: absolute;\n  top: 50%;',
    );
    expect(globals).toContain('.ant-select-arrow > * {\n  line-height: 1;\n}');
    expect(globals).toContain('.ant-select-arrow::before {\n  display: none;\n  content: \'\';\n}');
    expect(globals).toContain('.ant-select-arrow-icon {\n  display: block;\n}');
    expect(globals).toContain(
      '.ant-select-arrow .ant-select-arrow-icon svg {\n  transition: transform 0.3s;\n}',
    );
    expect(globals).toContain(
      '.ant-select-open .ant-select-arrow-icon svg {\n  transform: rotate(180deg);\n}',
    );
    expect(globals.indexOf('.ant-select-arrow {\n  display: inline-block;')).toBeLessThan(
      globals.indexOf('.ant-select-arrow > * {'),
    );
    expect(globals.indexOf('.ant-select-arrow-icon {\n  display: block;')).toBeLessThan(
      globals.indexOf('.ant-select-arrow .ant-select-arrow-icon svg {'),
    );
    expect(globals.indexOf('.ant-select-open .ant-select-arrow-icon svg')).toBeLessThan(
      globals.indexOf('.ant-select-focused .ant-select-selection,'),
    );
    expect(globals).toContain(
      '.ant-select-focused .ant-select-selection,\n.ant-select-selection:active,\n.ant-select-selection:focus,\n.ant-select-open .ant-select-selection {',
    );
    expect(globals).toContain('@keyframes antSlideUpIn {');
    expect(globals).toContain(
      '.slide-up-enter,\n.slide-up-appear {\n  opacity: 0;\n  animation-duration: 0.2s;',
    );
    expect(globals).toContain(
      '.ant-select-dropdown.slide-up-enter.slide-up-enter-active.ant-select-dropdown-placement-bottomLeft,\n.ant-select-dropdown.slide-up-appear.slide-up-appear-active.ant-select-dropdown-placement-bottomLeft {\n  animation-name: antSlideUpIn;',
    );
    expect(globals).toContain('.ant-select-dropdown-hidden {\n  display: none;\n}');
    expect(globals).toContain('.block-header.plan {\n  background-color: #fff !important;\n}');
    expect(globals).toContain('.v2board-plan-tabs {\n  padding: 8px 4px;');
    expect(globals).toContain(".v2board-plan-features > li::before {\n  padding-right: 10px;");
    expect(globals).toContain('.v2board-input-coupon:focus {\n  color: #fff;');
    expect(globals).toContain(
      '.ant-tag {\n  display: inline-block;\n  height: auto;\n  margin-right: 8px;\n  padding: 0 7px;',
    );
    expect(globals).toContain('font-size: 12px;\n  line-height: 1.5;\n  white-space: nowrap;');
    expect(globals).toContain('.ant-tag:last-child {\n  margin: 0;\n}');
    expect(globals).toContain('.ant-tabs-bar {\n  margin-bottom: 0;\n}');
    expect(globals.lastIndexOf('.ant-tag:last-child {\n  margin: 0;\n}')).toBeGreaterThan(
      globals.indexOf('.block-header.plan {\n  background-color: #fff !important;\n}'),
    );
    expect(globals).not.toContain('  text-align: center;\n  background: #fafafa;');
  });
});

describe('legacy account and dashboard utility CSS', () => {
  it('keeps packaged email whitelist, dashboard shortcut, and trade-number utilities', () => {
    const globals = css();

    expect(globals).toContain('.v2board-email-whitelist-enable {\n  display: flex;\n}');
    expect(globals).toContain(
      '.v2board-email-whitelist-enable input {\n  flex: 2 1;\n  border-top-right-radius: 0;',
    );
    expect(globals).toContain(
      '.v2board-email-whitelist-enable select {\n  flex: 1 1;\n  padding-right: 1.5em;',
    );
    expect(globals).toContain(
      'background-image: url("data:image/svg+xml;charset=utf-8,%3Csvg xmlns',
    );
    expect(globals).toContain(
      ".v2board-bg-pixels {\n  background-image: url('data:image/svg+xml;base64,PHN2ZyBoZWlnaHQ9IjIwMCI",
    );
    expect(globals).toContain('.v2board-bg-pixels {\n  background-image:');
    expect(globals).toContain('  background-size: auto;\n}');
    expect(globals).toContain(
      '.v2board-shortcuts-button {\n  display: block;\n  width: 100%;\n  padding: 0;',
    );
    expect(globals).toContain(
      '.v2board-shortcuts-item {\n  position: relative;\n  padding: 20px;\n  cursor: pointer;',
    );
    expect(globals).toContain(
      '.v2board-shortcuts-item > .description {\n  font-size: 12px;\n  opacity: 0.5;\n}',
    );
    expect(globals).toContain(
      '.v2board-shortcuts-item i {\n  position: absolute;\n  top: 25px;\n  right: 20px;',
    );
    expect(globals).toContain('.v2board-shortcuts-item:hover {\n  background: #f6f6f6;\n}');
    expect(globals).toContain(
      '.v2board-trade-no {\n  overflow: hidden;\n  text-overflow: ellipsis;\n  white-space: nowrap;',
    );
    expect(globals.indexOf('.v2board-email-whitelist-enable {')).toBeLessThan(
      globals.indexOf('.v2board-bg-pixels {'),
    );
    expect(globals.indexOf('.v2board-bg-pixels {')).toBeLessThan(
      globals.indexOf('.v2board-shortcuts-button {'),
    );
    expect(globals.indexOf('.v2board-shortcuts-button {')).toBeLessThan(
      globals.indexOf('.v2board-trade-no {'),
    );
  });
});

describe('legacy guest auth shell CSS', () => {
  it('keeps auth backgrounds, language trigger, and notice accents aligned', () => {
    const globals = css();

    expect(globals).toContain('.bg-image {\n  background-position: 0 50%;\n  background-size: cover;\n}');
    expect(globals).toContain(
      '.v2board-background {\n  position: fixed;\n  inset: 0;\n  background-color: #e8eaf2;',
    );
    expect(globals).toContain(
      '.v2board-auth-box {\n  position: fixed;\n  inset: 0;\n  display: flex;',
    );
    expect(globals).toContain(
      '.v2board-auth-box .block-content > .mb-3 > a.font-size-h1 {\n  color: var(--legacy-link);\n}',
    );
    expect(globals).toContain(
      '.v2board-auth-lang-btn {\n  position: absolute;\n  top: 0;\n  right: 0;\n}',
    );
    expect(globals).toContain('.v2board-lang-item {\n  padding: 10px 20px;\n}');
    expect(globals).toContain('.v2board-lang-item:hover {\n  background: #eee;\n}');
    expect(globals).toContain(
      '.v2board-no-access {\n  position: relative;\n  padding: 0.75rem 1.25rem;\n  margin-bottom: 1rem;',
    );
    expect(globals).toContain(
      '.v2board-notice-background {\n  position: absolute;\n  inset: 0;\n  z-index: 80;',
    );
    expect(globals.indexOf('.bg-image {')).toBeLessThan(
      globals.indexOf('.v2board-background {'),
    );
    expect(globals.indexOf('.v2board-auth-box {')).toBeLessThan(
      globals.indexOf('.v2board-auth-lang-btn {'),
    );
    expect(globals).not.toContain('.v2board-login-i18n-btn {');
    expect(globals.indexOf('.v2board-no-access {')).toBeLessThan(
      globals.indexOf('.v2board-notice-background {'),
    );
  });
});

describe('2026 shadcn island presentation CSS', () => {
  it('declares the pure shadcn island theme variables', () => {
    const globals = css();

    expect(globals).toContain("@import 'tailwindcss' prefix(tw);");
    expect(globals).toContain("@import 'tailwindcss/theme.css';");
    expect(globals).toContain('@media important {\n  @tailwind utilities source(none);');
    expect(globals).not.toContain("@import 'tailwindcss';");
    expect(globals).toContain('@theme inline {');
    expect(globals).toContain('--color-card: var(--card);');
    expect(globals).toContain('--color-muted-foreground: var(--muted-foreground);');
    expect(globals).toContain("@source '../pages/auth/**/*.tsx';");
    expect(globals).toContain("@source '../pages/dashboard.tsx';");
    expect(globals).toContain("@source '../components/layout/app-layout.tsx';");
    expect(globals).toContain('.v2board-auth-surface,');
    expect(globals).toContain('.v2board-app-shell,');
    expect(globals).toContain('.v2board-auth-toast-root,');
    expect(globals).toContain('.v2board-auth-language-menu-content,');
    expect(globals).toContain('.v2board-app-shell-menu-content,');
    expect(globals).toContain('.v2board-dashboard-dialog {\n  --radius: 0.625rem;');
    expect(globals).toContain('--card: oklch(1 0 0);');
    expect(globals).toContain('--background: oklch(1 0 0);');
    expect(globals).toContain('--primary: oklch(0.205 0 0);');
    expect(globals).toContain(
      '.v2board-auth-surface::selection,\n.v2board-auth-surface ::selection,',
    );
    expect(globals).toContain(
      '.v2board-app-shell::selection,\n.v2board-app-shell ::selection,',
    );
    expect(globals).toContain(
      '.v2board-auth-toast-root::selection,\n.v2board-auth-toast-root ::selection,',
    );
    expect(globals).toContain(
      '.v2board-app-shell-menu-content::selection,\n.v2board-app-shell-menu-content ::selection,',
    );
    expect(globals).toContain('.v2board-dashboard-dialog::selection,');
    expect(globals).toContain('color: var(--primary-foreground);\n  background: var(--primary);');
  });

  it('keeps only route-level auth CSS outside the shadcn JSX composition', () => {
    const globals = css();

    expect(globals).toContain('#main-container.v2board-auth-surface {');
    expect(globals).not.toContain('.v2board-auth-backdrop {');
    expect(globals).not.toContain('radial-gradient(');
    expect(globals).not.toContain('.v2board-auth-surface .v2board-auth-box {');
    expect(globals).not.toContain('.v2board-auth-surface .v2board-auth-card {');
    expect(globals).not.toContain('.v2board-auth-title--wordmark');
  });

  it('keeps the auth island static and avoids decorative motion or blobs', () => {
    const globals = css();

    expect(globals).not.toContain('v2board-auth-rise');
    expect(globals).not.toContain('@keyframes v2board-auth-rise');
    expect(globals).not.toContain('.v2board-auth-frame {\n    animation:');
    expect(globals).not.toContain('aurora');
    expect(globals).not.toContain('blur-3xl');
  });

  it('scopes the native dark theme to the auth surface and yields to DarkReader', () => {
    const globals = css();

    expect(globals).toContain('@media (prefers-color-scheme: dark) {');
    expect(globals).toContain('html:not([data-darkreader-scheme]) .v2board-auth-surface,');
    expect(globals).toContain('html:not([data-darkreader-scheme]) .v2board-app-shell,');
    expect(globals).toContain('--background: oklch(0.145 0 0);');
    expect(globals).toContain('--card: oklch(0.205 0 0);');
    expect(globals).toContain('--muted-foreground: oklch(0.708 0 0);');
  });

  it('themes shadcn portaled feedback without using legacy login classes', () => {
    const globals = css();

    expect(globals).toContain('.v2board-auth-toast-icon-success {');
    expect(globals).toContain('html:not([data-darkreader-scheme]) .v2board-auth-toast-root,');
    expect(globals).toContain('html:not([data-darkreader-scheme]) .v2board-auth-language-menu-content,');
    expect(globals).toContain('html:not([data-darkreader-scheme]) .v2board-app-shell-menu-content,');
    expect(globals).toContain('html:not([data-darkreader-scheme]) .v2board-dashboard-dialog {');
    expect(globals).not.toContain('.v2board-login-i18n-btn {');
  });
});

describe('legacy antd button and radio CSS', () => {
  it('keeps button variant rules that would otherwise be overwritten by the app bundle', () => {
    const globals = css();

    expect(globals).toContain('html {\n  --antd-wave-shadow-color: var(--legacy-ant-primary);\n}');
    expect(globals).toContain(
      "[ant-click-animating-without-extra-node='true'],\n[ant-click-animating='true'] {\n  position: relative;\n}",
    );
    expect(globals).toContain(
      ".ant-click-animating-node,\n[ant-click-animating-without-extra-node='true']::after {\n  position: absolute;\n  top: 0;",
    );
    expect(globals).toContain('animation:\n    fadeEffect 2s cubic-bezier(0.08, 0.82, 0.17, 1),');
    expect(globals).toContain('@keyframes waveEffect {\n  to {\n    box-shadow: 0 0 0 #0665d0;');
    expect(globals).toContain('@keyframes fadeEffect {\n  to {\n    opacity: 0;');
    expect(globals.indexOf('html {\n  --antd-wave-shadow-color')).toBeLessThan(
      globals.indexOf("[ant-click-animating-without-extra-node='true'],"),
    );
    expect(globals.indexOf("[ant-click-animating='true'] {\n  position: relative;")).toBeLessThan(
      globals.indexOf('.ant-click-animating-node,\n[ant-click-animating-without-extra-node'),
    );
    expect(globals.indexOf('@keyframes fadeEffect')).toBeLessThan(
      globals.indexOf('.ant-btn-two-chinese-chars:first-letter'),
    );
    expect(globals).toContain(
      '.ant-btn-two-chinese-chars:first-letter {\n  letter-spacing: 0.34em;\n}',
    );
    expect(globals).toContain(
      '.ant-btn {\n  position: relative;\n  display: inline-block;\n  height: 32px;',
    );
    expect(globals).toContain(
      '.ant-btn:focus,\n.ant-btn:hover {\n  color: var(--legacy-ant-hover);\n  text-decoration: none;',
    );
    expect(globals).toContain(
      '.ant-btn > .anticon + span,\n.ant-btn > span + .anticon {\n  margin-left: 8px;\n}',
    );
    expect(globals).toContain(
      ".ant-btn:before {\n  position: absolute;\n  top: -1px;\n  right: -1px;\n  bottom: -1px;\n  left: -1px;",
    );
    expect(globals).toContain(
      '.ant-btn.ant-btn-loading:not([disabled]) {\n  pointer-events: none;\n}',
    );
    expect(globals).toContain('.ant-btn.ant-btn-loading:before {\n  display: block;\n}');
    expect(globals).toContain(
      '.ant-btn.ant-btn-loading:not(.ant-btn-circle):not(.ant-btn-circle-outline):not(.ant-btn-icon-only) {\n  padding-left: 29px;',
    );
    expect(globals).toContain('  .anticon:not(:last-child) {\n  margin-left: -14px;\n}');
    expect(globals).toContain(
      '.ant-btn-sm.ant-btn-loading:not(.ant-btn-circle):not(.ant-btn-circle-outline):not(\n    .ant-btn-icon-only\n  ) {\n  padding-left: 24px;\n}',
    );
    expect(globals).toContain('  .anticon {\n  margin-left: -17px;\n}');
    expect(globals.indexOf('.ant-btn:before {\n  position: absolute;')).toBeLessThan(
      globals.indexOf('.ant-btn.ant-btn-loading:not(.ant-btn-circle):not(.ant-btn-circle-outline):not(.ant-btn-icon-only) {'),
    );
    expect(
      globals.indexOf(
        '.ant-btn.ant-btn-loading:not(.ant-btn-circle):not(.ant-btn-circle-outline):not(.ant-btn-icon-only) {',
      ),
    ).toBeLessThan(globals.indexOf('.ant-btn-sm.ant-btn-loading:not(.ant-btn-circle)'));
    expect(globals.indexOf('.ant-btn-sm.ant-btn-loading:not(.ant-btn-circle)')).toBeLessThan(
      globals.indexOf('.ant-btn-primary {\n  color: #fff;'),
    );
    expect(globals).toContain(
      '.ant-btn-primary {\n  color: #fff;\n  text-shadow: 0 -1px 0 rgba(0, 0, 0, 0.12);',
    );
    expect(globals).toContain(
      '.ant-btn-primary:focus,\n.ant-btn-primary:hover {\n  color: #fff;\n  background-color: var(--legacy-ant-hover);',
    );
    expect(globals).toContain(
      '.ant-btn-primary.active,\n.ant-btn-primary:active {\n  color: #fff;\n  background-color: var(--legacy-ant-active);',
    );
    expect(globals).toContain(
      '.ant-btn-primary-disabled,\n.ant-btn-primary.disabled,\n.ant-btn-primary[disabled],',
    );
    expect(globals.indexOf('.ant-btn-primary {\n  color: #fff;')).toBeLessThan(
      globals.indexOf('.ant-btn-primary:focus,\n.ant-btn-primary:hover {'),
    );
    expect(globals.indexOf('.ant-btn-primary.active,\n.ant-btn-primary:active {')).toBeLessThan(
      globals.indexOf('.ant-btn-primary-disabled,\n.ant-btn-primary.disabled,\n.ant-btn-primary[disabled],'),
    );
    expect(globals).toContain('.ant-btn-ghost {\n  color: rgba(0, 0, 0, 0.65);');
    expect(globals).toContain('.ant-btn-dashed {\n  color: rgba(0, 0, 0, 0.65);');
    expect(globals).toContain(
      '.ant-btn-danger {\n  color: #fff;\n  text-shadow: 0 -1px 0 rgba(0, 0, 0, 0.12);',
    );
    expect(globals).toContain(
      '.ant-btn-danger:focus,\n.ant-btn-danger:hover {\n  color: #fff;\n  background-color: #ff7875;',
    );
    expect(globals).toContain(
      '.ant-btn-danger.active,\n.ant-btn-danger:active {\n  color: #fff;\n  background-color: #d9363e;',
    );
    expect(globals).toContain(
      '.ant-btn-danger-disabled,\n.ant-btn-danger.disabled,\n.ant-btn-danger[disabled],',
    );
    expect(globals.indexOf('.ant-btn-danger {\n  color: #fff;')).toBeGreaterThan(
      globals.indexOf('.ant-btn-ghost {'),
    );
    expect(globals.indexOf('.ant-btn-danger {\n  color: #fff;')).toBeLessThan(
      globals.indexOf('.ant-btn-danger:focus,\n.ant-btn-danger:hover {'),
    );
    expect(globals.indexOf('.ant-btn-danger.active,\n.ant-btn-danger:active {')).toBeLessThan(
      globals.indexOf('.ant-btn-danger-disabled,\n.ant-btn-danger.disabled,\n.ant-btn-danger[disabled],'),
    );
    expect(globals.indexOf('.ant-btn-danger-disabled,\n.ant-btn-danger.disabled')).toBeLessThan(
      globals.indexOf('.ant-btn-link {\n  color: var(--legacy-ant-primary);'),
    );
    expect(globals).toContain('.ant-btn-link {\n  color: var(--legacy-ant-primary);');
    expect(globals).toContain('font-weight: 400;\n  line-height: 1.5;\n  white-space: nowrap;');
    expect(globals).toContain(
      'background-color: transparent;\n  border-color: transparent;\n  box-shadow: none;',
    );
    expect(globals).toContain(
      '.ant-btn-link:focus,\n.ant-btn-link:hover {\n  color: var(--legacy-ant-hover);',
    );
    expect(globals).toContain(
      '.ant-btn-link:active {\n  color: var(--legacy-ant-active);\n  background-color: transparent;\n  border-color: transparent;',
    );
    expect(globals).toContain(
      '.ant-btn-link-disabled,\n.ant-btn-link.disabled,\n.ant-btn-link[disabled],',
    );
    expect(globals.indexOf('.ant-btn-link {\n  color: var(--legacy-ant-primary);')).toBeLessThan(
      globals.indexOf('.ant-btn-link:focus,\n.ant-btn-link:hover {'),
    );
    expect(globals.indexOf('.ant-btn-link:focus,\n.ant-btn-link:hover {')).toBeLessThan(
      globals.indexOf('.ant-btn-link-disabled,\n.ant-btn-link.disabled,\n.ant-btn-link[disabled],'),
    );
    expect(globals).toContain('.ant-btn-background-ghost {\n  color: #fff;');
    expect(globals).toContain('background: transparent !important;\n  border-color: #fff;');
    expect(globals).toContain(
      '.ant-btn-background-ghost.ant-btn-primary {\n  color: var(--legacy-ant-primary);\n  text-shadow: none;',
    );
    expect(globals).toContain(
      '.ant-btn-background-ghost.ant-btn-danger:focus,\n.ant-btn-background-ghost.ant-btn-danger:hover {\n  color: #ff7875;',
    );
    expect(globals).toContain(
      '.ant-btn-background-ghost.ant-btn-link.active,\n.ant-btn-background-ghost.ant-btn-link:active {\n  color: var(--legacy-ant-active);',
    );
    expect(globals).toContain(
      '.ant-btn-background-ghost.ant-btn-primary-disabled,\n.ant-btn-background-ghost.ant-btn-primary.disabled,\n.ant-btn-background-ghost.ant-btn-primary[disabled],',
    );
    expect(globals).toContain(
      '.ant-btn-background-ghost.ant-btn-danger-disabled,\n.ant-btn-background-ghost.ant-btn-danger.disabled,\n.ant-btn-background-ghost.ant-btn-danger[disabled],',
    );
    expect(globals).toContain(
      '.ant-btn-background-ghost.ant-btn-link[disabled].active {\n  color: rgba(0, 0, 0, 0.25);',
    );
    expect(globals.indexOf('.ant-btn-background-ghost.ant-btn-primary-disabled')).toBeLessThan(
      globals.indexOf('.ant-btn-background-ghost.ant-btn-danger-disabled'),
    );
    expect(globals.indexOf('.ant-btn-background-ghost.ant-btn-danger-disabled')).toBeLessThan(
      globals.indexOf('.ant-btn-background-ghost.ant-btn-link-disabled'),
    );
    expect(globals).toContain(
      '.ant-btn-icon-only {\n  width: 32px;\n  height: 32px;\n  padding: 0;',
    );
    expect(globals).toContain('.ant-btn-round {\n  height: 32px;\n  padding: 0 16px;');
    expect(globals).toContain('.ant-btn-round.ant-btn-lg {\n  height: 40px;\n  padding: 0 20px;');
    expect(globals).toContain('.ant-btn-round.ant-btn-sm {\n  height: 24px;\n  padding: 0 12px;');
    expect(globals).toContain('.ant-btn-circle,\n.ant-btn-circle-outline {\n  min-width: 32px;');
    expect(globals).toContain('.ant-btn-circle.ant-btn-sm,\n.ant-btn-circle-outline.ant-btn-sm {\n  min-width: 24px;');
    expect(globals.indexOf('.ant-btn-round {\n  height: 32px;')).toBeLessThan(
      globals.indexOf('.ant-btn-circle,\n.ant-btn-circle-outline {'),
    );
    expect(globals.indexOf('.ant-btn-circle.ant-btn-sm')).toBeLessThan(
      globals.indexOf('.ant-btn-group .ant-btn {\n  border-radius: 0;'),
    );
    expect(globals).toContain('.ant-btn-group .ant-btn {\n  border-radius: 0;\n}');
    expect(globals).toContain('.ant-btn-block {\n  display: block;\n  width: 100%;\n}');
    expect(globals).toContain('a.ant-btn {\n  padding-top: 0.1px;\n  line-height: 30px;\n}');
    expect(globals).toContain(
      '.ant-input-search-button {\n  border-top-left-radius: 0;\n  border-bottom-left-radius: 0;\n}',
    );
    expect(globals).toContain(
      '.ant-btn-group .ant-btn-primary:not(:first-child):not(:last-child) {\n  border-right-color: var(--legacy-ant-hover);',
    );
    expect(globals).toContain(
      '.ant-btn-group .ant-btn-primary + .ant-btn:not(.ant-btn-primary):not([disabled]) {\n  border-left-color: transparent;',
    );
  });

  it('uses the packaged theme focus opacity for radio buttons separately from native radios', () => {
    const globals = css();

    expect(globals).toContain('--legacy-ant-radio-focus-shadow: rgba(6, 101, 208, 0.08);');
    expect(globals).toContain('--legacy-ant-radio-button-focus-shadow: rgba(6, 101, 208, 0.06);');
    expect(globals).toContain(
      '.ant-radio-button-wrapper:focus-within {\n  outline: 3px solid var(--legacy-ant-radio-button-focus-shadow);',
    );
    expect(globals).toContain(
      '.ant-radio-group-large .ant-radio-button-wrapper {\n  height: 40px;\n  font-size: 16px;\n  line-height: 38px;',
    );
    expect(globals).toContain(
      '.ant-radio-button-wrapper {\n  position: relative;\n  display: inline-block;\n  height: 32px;',
    );
    expect(globals).toContain(
      ".ant-radio-button-wrapper:not(:first-child)::before {\n  position: absolute;\n  top: 0;\n  left: -1px;\n  display: block;\n  width: 1px;\n  height: 100%;\n  background-color: #d9d9d9;\n  content: '';",
    );
    expect(globals).toContain(
      ".ant-radio-button-wrapper input[type='radio'] {\n  width: 0;\n  height: 0;\n  opacity: 0;",
    );
    expect(globals).toContain(
      '.ant-radio-button-wrapper-checked:not(.ant-radio-button-wrapper-disabled) {\n  z-index: 1;\n  color: var(--legacy-ant-primary);',
    );
    expect(globals).toContain(
      '.ant-radio-button-wrapper-checked:not(.ant-radio-button-wrapper-disabled):hover {\n  color: var(--legacy-ant-hover);',
    );
    expect(globals).toContain(
      '.ant-radio-group-solid .ant-radio-button-wrapper-checked:not(.ant-radio-button-wrapper-disabled) {\n  color: #fff;\n  background: var(--legacy-ant-primary);',
    );
    expect(globals).toContain(
      '.ant-radio-button-wrapper-disabled.ant-radio-button-wrapper-checked {\n  color: #fff;\n  background-color: #e6e6e6;',
    );
    expect(globals).toContain(
      '.ant-radio-wrapper {\n  box-sizing: border-box;\n  position: relative;\n  display: inline-block;',
    );
    expect(globals).toContain(
      '.ant-radio {\n  box-sizing: border-box;\n  position: relative;\n  display: inline-block;\n  margin: 0;\n  padding: 0;\n  color: rgba(0, 0, 0, 0.65);\n  font-size: 14px;\n  font-variant: tabular-nums;\n  line-height: 1.5;',
    );
    expect(globals).toContain(
      '.ant-radio-input {\n  position: absolute;\n  top: 0;\n  right: 0;\n  bottom: 0;',
    );
    expect(globals).toContain(
      '.ant-radio-inner {\n  position: relative;\n  top: 0;\n  left: 0;\n  display: block;\n  width: 16px;\n  height: 16px;',
    );
    expect(globals).toContain(
      '.ant-radio-input:focus + .ant-radio-inner {\n  box-shadow: 0 0 0 3px var(--legacy-ant-radio-focus-shadow);',
    );
    expect(globals).toContain(
      '.ant-radio-checked .ant-radio-inner::after {\n  transform: scale(1);\n  opacity: 1;',
    );
    expect(globals).toContain(
      '.ant-radio-disabled .ant-radio-inner {\n  background-color: #f5f5f5;\n  border-color: #d9d9d9 !important;',
    );
    expect(globals).toContain('span.ant-radio + * {\n  padding-right: 8px;\n  padding-left: 8px;\n}');
    expect(globals).toContain('@keyframes antRadioEffect {');
    expect(globals).toContain('.v2board-select-radio {\n  display: none;\n}');
    expect(globals.indexOf('.v2board-select-radio {\n  display: none;\n}')).toBeGreaterThan(
      globals.indexOf('.ant-radio-wrapper {\n  box-sizing: border-box;'),
    );
    expect(globals.indexOf('.ant-radio {\n  box-sizing: border-box;')).toBeGreaterThan(
      globals.indexOf('.v2board-select-radio {\n  display: none;\n}'),
    );
    expect(globals.indexOf('.ant-radio-input {\n  position: absolute;')).toBeGreaterThan(
      globals.indexOf('.ant-radio {\n  box-sizing: border-box;'),
    );
    expect(globals.indexOf('.ant-radio-inner {\n  position: relative;')).toBeGreaterThan(
      globals.indexOf('.ant-radio-input {\n  position: absolute;'),
    );
    expect(globals.indexOf('  .v2board-select-radio {\n    display: unset;\n  }')).toBeGreaterThan(
      globals.indexOf('.v2board-select-radio {\n  display: none;\n}'),
    );
  });
});

describe('legacy ticket, order, and payment utility CSS', () => {
  it('keeps ticket chat layout, order rows, and checkout selection cards aligned', () => {
    const globals = css();

    expect(globals).toContain(
      '.content___DW5w1 {\n  position: absolute;\n  top: 0;\n  bottom: 0;',
    );
    expect(globals).toContain('.input___1j_ND {\n  position: fixed;\n  bottom: 0;\n}');
    expect(globals).toContain('.tag___12_9H {\n  color: #000;\n  padding: 5px 10px;');
    expect(globals).toContain(
      '.bubble___3NP2- {\n  padding: 10px 10px 30px;\n  font-size: 14px;',
    );
    expect(globals).toContain('.time___1yWOE {\n  position: absolute;\n}');
    expect(globals).toContain(
      '.v2board-order-info > div {\n  display: flex;\n  margin-bottom: 5px;\n  font-size: 14px;',
    );
    expect(globals).toContain(
      '.v2board-order-info > div > span:first-child {\n  flex: 1 1;\n  opacity: 0.5;\n}',
    );
    expect(globals).toContain(
      '.v2board-order-info > div > span:last-child {\n  flex: 2 1;\n  font-family: menlo;\n}',
    );
    expect(globals).toContain(
      '.v2board-select {\n  display: flex;\n  padding: 20px;\n  font-size: 16px;',
    );
    expect(globals).toContain(
      '.v2board-select.active {\n  margin: -2px -2px -1px;\n  border-bottom: unset;',
    );
    expect(globals).toContain(
      '.border-primary {\n  border-color: var(--color-brand-500) !important;\n}',
    );
    expect(globals.indexOf('.content___DW5w1 {')).toBeLessThan(
      globals.indexOf('.v2board-order-info > div {'),
    );
    expect(globals.indexOf('.v2board-order-info > div {')).toBeLessThan(
      globals.indexOf('.v2board-select {'),
    );
    expect(globals.indexOf('.v2board-select.active {')).toBeLessThan(
      globals.indexOf('.border-primary {'),
    );
    expect(globals.indexOf('.border-primary {')).toBeLessThan(
      globals.indexOf('.v2board-select-radio {'),
    );
  });
});

describe('legacy antd switch CSS', () => {
  it('keeps switch shell, handle, checked, loading, and focus states aligned with antd v3', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-switch {\n  margin: 0;\n  position: relative;\n  display: inline-block;\n  box-sizing: border-box;\n  min-width: 44px;',
    );
    expect(globals).toContain(
      '.ant-switch-loading-icon,\n.ant-switch::after {\n  position: absolute;\n  top: 1px;\n  left: 1px;',
    );
    expect(globals).toContain(
      '.ant-switch::after {\n  box-shadow: 0 2px 4px 0 rgba(0, 35, 11, 0.2);\n}',
    );
    expect(globals).toContain(
      '.ant-switch:not(.ant-switch-disabled):active::after,\n.ant-switch:not(.ant-switch-disabled):active::before {\n  width: 24px;\n}',
    );
    expect(globals).toContain(
      '.ant-switch-loading-icon {\n  z-index: 1;\n  display: none;\n  font-size: 12px;',
    );
    expect(globals).toContain(
      '.ant-switch-loading-icon svg {\n  position: absolute;\n  top: 0;\n  right: 0;',
    );
    expect(globals).toContain(
      '.ant-switch-loading .ant-switch-loading-icon {\n  display: inline-block;\n  color: rgba(0, 0, 0, 0.65);\n}',
    );
    expect(globals).toContain(
      '.ant-switch-checked .ant-switch-inner {\n  margin-right: 24px;\n  margin-left: 6px;\n}',
    );
    expect(globals).toContain(
      '.ant-switch-checked::after {\n  left: 100%;\n  margin-left: -1px;\n  transform: translateX(-100%);\n}',
    );
    expect(globals).toContain(
      '.ant-switch-disabled,\n.ant-switch-loading {\n  cursor: not-allowed;\n  opacity: 0.4;\n}',
    );
    expect(globals).toContain(
      '.ant-switch:focus {\n  outline: 0;\n  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);\n}',
    );
    expect(globals.indexOf('.ant-switch-loading-icon,\n.ant-switch::after')).toBeLessThan(
      globals.indexOf('.ant-switch:not(.ant-switch-disabled):active::after'),
    );
    expect(globals.indexOf('.ant-switch-loading .ant-switch-loading-icon')).toBeLessThan(
      globals.indexOf('.ant-switch-checked .ant-switch-inner'),
    );
    expect(globals.indexOf('.ant-switch-checked::after')).toBeLessThan(
      globals.indexOf('.ant-switch-disabled,\n.ant-switch-loading'),
    );
  });
});

describe('legacy OneUI utility and form CSS', () => {
  it('keeps source-owned theme tokens available to Tailwind and legacy CSS', () => {
    const globals = css();

    expect(globals).toContain('--color-brand-500: #0665d0;');
    expect(globals).toContain('--color-page: #f0f3f8;');
    expect(globals).toContain('--legacy-ant-primary: #0665d0;');
    expect(globals).toContain('--legacy-nav-dark-horizontal-bg: #0559b7;');
    expect(globals).toContain('--shadow-block: 0 1px 3px rgba(219, 226, 239, 0.5)');
    expect(globals).toContain('--font-sans:');
    expect(globals).toContain(':root,\n:host {\n  --color-brand-50: #e6f0fb;');
    expect(globals).toContain(':root,\n:host {\n  --legacy-link: #0665d0;');
    expect(globals).toContain(':root,\n:host {\n  --radius-card: 2px;');
  });

  it('keeps body text rendering aligned with the packaged theme', () => {
    const globals = css();
    const bodyBlock = globals.match(/body \{[\s\S]*?\n\}/)?.[0] ?? '';

    expect(globals).toContain('html,\nbody,\n#root {\n  height: 100%;\n  min-height: 100%;\n}');
    expect(bodyBlock).toContain('font-family: var(--font-sans);');
    expect(bodyBlock).toContain('background-color: var(--color-page);');
    expect(bodyBlock).toContain("font-feature-settings: 'tnum';");
    expect(bodyBlock).toContain('-webkit-font-smoothing: antialiased;');
    expect(bodyBlock).toContain('text-rendering: optimizeLegibility;');
  });

  it('keeps packaged heading, prose, and link defaults intact', () => {
    const globals = css();

    expect(globals).toContain(
      '.h1,\n.h2,\n.h3,\n.h4,\n.h5,\n.h6,\nh1,\nh2,\nh3,\nh4,\nh5,\nh6 {\n  color: var(--color-heading);',
    );
    expect(globals).toContain('.h1,\nh1 {\n  font-size: 2.25rem;\n}');
    expect(globals).toContain('.h6,\nh6 {\n  font-size: 1rem;\n}');
    expect(globals).toContain(
      'h1,\nh2,\nh3,\nh4,\nh5,\nh6 {\n  color: rgba(0, 0, 0, 0.85);\n}',
    );
    expect(globals).toContain(
      'p {\n  margin-top: 0;\n  margin-bottom: 1rem;\n  line-height: 1.6;\n}',
    );
    expect(globals).toContain('b,\nstrong {\n  font-weight: 600;\n}');
    expect(globals).toContain(
      'a:not([class]) {\n  color: var(--legacy-link);\n  text-decoration: none;\n  transition: color 0.12s ease-out;',
    );
    expect(globals).toContain('a:not([href]):hover {\n  color: unset;\n}');
  });

  it('keeps packaged Bootstrap utility and button focus rules intact', () => {
    const globals = css();

    expect(globals).toContain('.text-light { color: #f8f9fa !important; }');
    expect(globals).toContain('.d-flex { display: flex !important; }');
    expect(globals).toContain('.px-4 { padding-right: 1.5rem !important; padding-left: 1.5rem !important; }');
    expect(globals).toContain('.overflow-y-auto {\n  overflow-y: auto;\n  -webkit-overflow-scrolling: touch;\n}');
    expect(globals).toContain('.display-4 {\n  font-size: 3.5rem;\n  font-weight: 300;\n  line-height: 1.25;\n}');
    expect(globals).toContain('.bg-white {\n  background-color: #fff !important;\n}');
    expect(globals).toContain('.text-dark {\n  color: #343a40 !important;\n}');
    expect(globals).toContain(
      '.spinner-grow {\n  display: inline-block;\n  width: 2rem;\n  height: 2rem;',
    );
    expect(globals).toContain('animation: spinner-grow 0.75s linear infinite;');
    expect(globals).toContain(
      '.sr-only {\n  position: absolute;\n  width: 1px;\n  height: 1px;',
    );
    expect(globals).toContain(
      '@keyframes spinner-grow {\n  0% {\n    transform: scale(0);\n  }',
    );
    expect(globals.indexOf('.bg-white {')).toBeLessThan(globals.indexOf('.text-dark {'));
    expect(globals.indexOf('.text-dark {')).toBeLessThan(globals.indexOf('.spinner-grow {'));
    expect(globals.indexOf('.spinner-grow {')).toBeLessThan(globals.indexOf('.sr-only {'));
    expect(globals.indexOf('.sr-only {')).toBeLessThan(globals.indexOf('@keyframes spinner-grow'));
    expect(globals).toContain('.hero {\n  position: relative;\n  display: flex;');
    expect(globals).toContain('.hero-static {\n  min-height: 100vh;\n}');
    expect(globals).toContain('  .d-lg-none { display: none !important; }');
    expect(globals).toContain('  .order-md-1 { order: 1; }');
    expect(globals).toContain('  .d-sm-flex { display: flex !important; }');
    expect(globals).toContain('  .col-xl-5 {\n    flex: 0 0 41.666667%;');
    expect(globals.indexOf('@media (min-width: 992px) {\n  .d-lg-none')).toBeLessThan(
      globals.indexOf('@media (min-width: 768px) {\n  .order-md-1'),
    );
    expect(globals.indexOf('@media (min-width: 768px) {\n  .order-md-1')).toBeLessThan(
      globals.indexOf('@media (min-width: 576px) {\n  .d-sm-inline-block'),
    );
    expect(globals.indexOf('@media (min-width: 576px) {\n  .d-sm-inline-block')).toBeLessThan(
      globals.indexOf('@media (min-width: 1200px) {\n  .mx-xl-0'),
    );
    expect(globals).toContain('.btn:hover {\n  color: #495057;\n  text-decoration: none;\n}');
    expect(globals).toContain(
      '.btn:focus {\n  outline: 0;\n  box-shadow: 0 0 0 0.2rem rgba(6, 101, 208, 0.25);\n}',
    );
    expect(globals).not.toContain('.btn:hover,\n.btn:focus');
    expect(globals).toContain(
      '.btn-primary:not(:disabled):not(.disabled).active,\n.btn-primary:not(:disabled):not(.disabled):active,\n.show > .btn-primary.dropdown-toggle {\n  color: #fff;',
    );
    expect(globals).toContain(
      '.btn-dark.disabled,\n.btn-dark:disabled {\n  color: #fff;\n  background-color: #343a40;',
    );
    expect(globals).toContain(
      '.btn-danger:focus {\n  box-shadow: 0 0 0 0.2rem rgba(229, 105, 60, 0.5);\n}',
    );
    expect(globals).toContain(
      '.btn-alt-primary:disabled {\n  color: var(--legacy-alt-primary-disabled-text);\n  background-color: var(--legacy-alt-primary-bg);',
    );
    expect(globals).toContain('.btn-block {\n  display: block;\n  width: 100%;\n}');
    expect(globals).toContain('.btn.disabled,\n.btn:disabled {\n  opacity: 0.65;\n}');
    expect(globals).toContain('.btn-block + .btn-block {\n  margin-top: 0.5rem;\n}');
    expect(globals.indexOf('.btn-block {\n  display: block;')).toBeLessThan(
      globals.indexOf('.btn.disabled,\n.btn:disabled {'),
    );
    expect(globals.indexOf('.btn-block + .btn-block {')).toBeLessThan(
      globals.indexOf('.block {\n  margin-bottom: 1.75rem;'),
    );
    expect(globals).toContain(
      '.block {\n  margin-bottom: 1.75rem;\n  background-color: #fff;\n  box-shadow: 0 1px 3px rgba(219, 226, 239, 0.5), 0 1px 2px rgba(219, 226, 239, 0.5);\n}',
    );
    expect(globals).toContain(
      'p {\n  margin-top: 0;\n  margin-bottom: 1rem;\n  line-height: 1.6;\n}',
    );
    expect(globals).toContain('b,\nstrong {\n  font-weight: 600;\n}');
    expect(globals).toMatch(
      /h1,\s*h2,\s*h3,\s*h4,\s*h5,\s*h6\s*\{\s*color: rgba\(0, 0, 0, 0\.85\);/,
    );
    expect(globals).toContain(
      '.form-control {\n  display: block;\n  width: 100%;\n  height: calc(1.5em + 0.75rem + 2px);',
    );
    expect(globals).toContain(
      '.form-control:focus {\n  color: #495057;\n  background-color: #fff;\n  border-color: var(--legacy-form-focus-border);',
    );
    expect(globals).toContain(
      '.form-control.form-control-alt {\n  background-color: var(--color-page);\n  border-color: var(--color-page);\n  transition: none;\n}',
    );
    expect(globals).toContain(
      '.form-control.form-control-alt:focus {\n  background-color: var(--legacy-sidebar-dark-color);\n  border-color: var(--legacy-sidebar-dark-color);\n  box-shadow: none;\n}',
    );
    expect(globals).toContain('.form-control::placeholder {\n  color: #6c757d;\n  opacity: 1;\n}');
    expect(globals).toContain(
      '.form-control:disabled,\n.form-control[readonly] {\n  background-color: #e9ecef;\n  opacity: 1;\n}',
    );
    expect(globals.indexOf('.form-control {\n  display: block;')).toBeLessThan(
      globals.indexOf('.form-control:focus {'),
    );
    expect(globals.indexOf('.form-control:focus {')).toBeLessThan(
      globals.indexOf('.form-control.form-control-alt {'),
    );
    expect(globals.indexOf('.form-control.form-control-alt:focus {')).toBeLessThan(
      globals.indexOf('.form-control::placeholder {'),
    );
    expect(globals.indexOf('.form-control::placeholder {')).toBeLessThan(
      globals.indexOf('.form-control:disabled,'),
    );
    expect(globals).toContain('.block.block-rounded {\n  border-radius: 0.25rem;\n}');
    expect(globals).toContain(
      '.block-title {\n  flex: 1 1 auto;\n  min-height: 1.75rem;\n  margin: 0;\n  color: rgba(0, 0, 0, 0.85);',
    );
    expect(globals).toContain(
      '.block.block-transparent {\n  background-color: transparent;\n  box-shadow: none;\n}',
    );
    expect(globals).toContain(
      '.block.block-fx-pop {\n  box-shadow: 0 0.5rem 2rem var(--legacy-block-pop-shadow);\n  opacity: 1;\n}',
    );
    expect(globals).toContain(
      'a.block {\n  display: block;\n  color: #495057;\n  font-weight: 400;\n  transition:',
    );
    expect(globals).toContain('a.block:hover {\n  color: #495057;\n  opacity: 0.65;\n}');
  });

  it('matches the packaged custom control and badge cascade', () => {
    const globals = css();
    const badge = globals.match(/\.badge \{[\s\S]*?\n\}/)?.[0] ?? '';

    expect(globals).toContain(
      ".custom-control-label::before {\n  position: absolute;\n  top: 0.25rem;\n  left: -1.5rem;\n  display: block;\n  width: 1rem;\n  height: 1rem;\n  pointer-events: none;\n  content: '';\n  background-color: #e2e8f2;\n  border: none;\n}",
    );
    expect(globals).toContain(
      '.custom-control-input:checked ~ .custom-control-label::before {\n  color: #fff;\n  background-color: var(--color-brand-500);',
    );
    expect(globals).toContain(
      '.custom-control-primary .custom-control-input:focus ~ .custom-control-label::before {\n  box-shadow:\n    0 0 0 1px #fff,',
    );
    expect(globals).toContain(
      '.custom-control-label::after {\n  left: -1.25rem;\n}',
    );
    expect(globals).toContain(
      '.progress {\n  display: flex;\n  height: 1.25rem;\n  overflow: hidden;',
    );
    expect(globals).toContain(
      '.progress-bar {\n  display: flex;\n  flex-direction: column;\n  justify-content: center;',
    );
    expect(globals).toContain(
      '.progress-bar-striped {\n  background-image: linear-gradient(',
    );
    expect(globals).toContain(
      '.progress-bar-animated {\n  animation: progress-bar-stripes 1s linear infinite;\n}',
    );
    expect(globals).toContain(
      '@keyframes progress-bar-stripes {\n  from {\n    background-position: 1.25rem 0;\n  }',
    );
    expect(globals.indexOf('.progress {\n  display: flex;')).toBeLessThan(
      globals.indexOf('.progress-bar {\n  display: flex;'),
    );
    expect(globals.indexOf('.progress-bar-striped {')).toBeLessThan(
      globals.indexOf('.progress-bar-animated {'),
    );
    expect(globals.indexOf('.progress-bar-animated {')).toBeLessThan(
      globals.indexOf('@keyframes progress-bar-stripes'),
    );
    expect(globals).toContain('.bg-body-dark {\n  background-color: var(--legacy-sidebar-dark-color) !important;\n}');
    expect(globals).toContain('.form-group .custom-control-label {\n  margin-bottom: 0;\n}');
    expect(globals).toContain(
      '.text-uppercase {\n  text-transform: uppercase !important;\n  letter-spacing: 0.0625rem;\n}',
    );
    expect(badge).not.toContain('color: #fff;');
    expect(globals).toContain('.badge-danger {\n  color: #fff;\n  background-color: #e04f1a;\n}');
    expect(globals).toContain(
      '.alert {\n  position: relative;\n  padding: 0.75rem 1.25rem;\n  margin-bottom: 1rem;',
    );
    expect(globals).toContain(
      '.alert-warning {\n  color: #855c0d;\n  background-color: #ffefd1;\n  border-color: #ffe9bf;\n}',
    );
    expect(globals).toContain(
      '.alert-link {\n  color: inherit;\n  font-weight: 600;\n}',
    );
    expect(globals).toContain('.alert-warning .alert-link {\n  color: #573c08;\n}');
    expect(globals).toContain('.alert-danger .alert-link {\n  color: #461909;\n}');
    expect(globals.indexOf('.alert {\n  position: relative;')).toBeLessThan(
      globals.indexOf('.alert-danger {'),
    );
    expect(globals.indexOf('.alert-dark {')).toBeLessThan(
      globals.indexOf('.alert-link {\n  color: inherit;'),
    );
    expect(globals.indexOf('.alert-link {\n  color: inherit;')).toBeLessThan(
      globals.indexOf('.alert-warning .alert-link {'),
    );
  });

  it('keeps the packaged sidebar transition gate for the OneUI shell', () => {
    const globals = css();

    expect(globals).toContain(
      '#page-header .content-header {\n  padding-right: 0.875rem;\n  padding-left: 0.875rem;\n}',
    );
    expect(globals).toContain(
      '.content-header {\n  display: flex;\n  align-items: center;\n  justify-content: space-between;\n  height: 3.25rem;',
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
    expect(globals).toContain('.content.content-full {\n  padding-bottom: 0.875rem;\n}');
    expect(globals).toContain(
      '@media (min-width: 768px) {\n  .content {\n    padding: 1.75rem 1.75rem 1px;\n  }\n}',
    );
    expect(globals).toContain('.content > .pull {\n    margin: -1.75rem -1.75rem -1px;\n  }');
    expect(globals).toContain(
      '@media (min-width: 768px) {\n  .content.content-full {\n    padding-bottom: 1.75rem;\n  }',
    );
    expect(globals).toContain(
      '@media (min-width: 768px) {\n  .content .block,\n  .content .items-push > div,\n  .content .push,\n  .content p {\n    margin-bottom: 1.75rem;',
    );
    expect(globals.indexOf('@media (min-width: 768px) {\n  .content {')).toBeLessThan(
      globals.indexOf('@media (min-width: 768px) {\n  .content > .pull-t'),
    );
    expect(
      globals.indexOf('@media (min-width: 768px) {\n  .content.content-full {'),
    ).toBeLessThan(
      globals.indexOf(
        '@media (min-width: 768px) {\n  .content .block,\n  .content .items-push > div,',
      ),
    );
    expect(globals).toContain('.block-content.block-content-full {\n  padding-bottom: 1.25rem;\n}');
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
      '#page-container.page-header-dark #page-header {\n  color: var(--legacy-header-dark-color);\n  background-color: var(--color-brand-500);',
    );
    expect(globals).toContain(
      '#sidebar .content-header {\n  padding-right: 1.125rem;\n  padding-left: 1.125rem;\n}',
    );
    expect(globals).toContain('.smini-visible,\n.smini-visible-block {\n  display: none;\n}');
    expect(globals).toContain('.smini-show {\n  opacity: 0;\n}');
    expect(globals).toContain(
      '.smini-hide,\n.smini-show {\n  transition: opacity 0.45s cubic-bezier(0.2, 0.61, 0.42, 0.97);',
    );
    expect(globals).toContain(
      '#sidebar {\n  position: fixed;\n  top: 0;\n  bottom: 0;\n  left: 0;\n  z-index: 999;',
    );
    expect(globals).toContain(
      'transform: translateX(-100%) translateY(0) translateZ(0);\n  transition: transform 0.45s cubic-bezier(0.2, 0.61, 0.42, 0.97);',
    );
    expect(globals).toContain(
      '.sidebar-o-xs #sidebar {\n  transform: translateX(0) translateY(0) translateZ(0);\n}',
    );
    expect(globals).toContain(
      '#page-container.sidebar-dark #sidebar {\n  color: var(--legacy-sidebar-dark-color);\n  background-color: var(--legacy-sidebar-dark-bg);',
    );
    expect(globals).toContain(
      '.bg-header-dark {\n  background-color: var(--color-brand-500) !important;\n}',
    );
    expect(globals).toContain(
      '.bg-white-10 {\n  background-color: rgba(255, 255, 255, 0.1) !important;\n}',
    );
    expect(globals.indexOf('#sidebar .content-header {')).toBeLessThan(
      globals.indexOf('.smini-visible,\n.smini-visible-block {'),
    );
    expect(globals.indexOf('.smini-visible,\n.smini-visible-block {')).toBeLessThan(
      globals.indexOf('#sidebar {'),
    );
    expect(globals.indexOf('#sidebar {')).toBeLessThan(
      globals.indexOf('.side-trans-enabled #sidebar {'),
    );
    expect(globals.indexOf('.side-trans-enabled #side-overlay {')).toBeLessThan(
      globals.indexOf('.sidebar-o-xs #sidebar {'),
    );
    expect(globals.indexOf('.sidebar-o-xs #sidebar {')).toBeLessThan(
      globals.indexOf('#page-container.sidebar-dark #sidebar {'),
    );
    expect(globals.indexOf('#page-container.sidebar-dark #sidebar {')).toBeLessThan(
      globals.indexOf('.bg-header-dark {'),
    );
    expect(globals).toContain('.sidebar-toggle {\n  display: none;\n}');
    expect(globals).toContain('  .sidebar-toggle {\n    display: block !important;\n  }');
    expect(globals).toContain('  #sidebar {\n    width: 250px;\n  }');
    expect(globals).toContain(
      '  .sidebar-mini.sidebar-o #sidebar {\n    overflow-x: hidden;\n    transform: translateX(-186px) translateY(0) translateZ(0);',
    );
    expect(globals).toContain(
      '  .sidebar-mini.sidebar-o #sidebar:hover,\n  .sidebar-mini.sidebar-o #sidebar:hover .content-header,\n  .sidebar-mini.sidebar-o #sidebar:hover .content-side,',
    );
    expect(globals).toContain(
      '  .sidebar-mini.sidebar-o #sidebar:not(:hover) .smini-visible-block {\n    display: block;\n  }',
    );
    expect(globals).toContain(
      '.overlay-header {\n  position: absolute;\n  top: 0;\n  right: 0;\n  bottom: 0;\n  left: 0;\n  background-color: #fff;\n  opacity: 0;',
    );
    expect(globals).toContain(
      '.overlay-header.show {\n  opacity: 1;\n  transform: translateY(0);\n}',
    );
    expect(globals).toContain(
      '  #page-container.page-header-fixed.sidebar-o #page-header .overlay-header,\n  #page-container.page-header-glass.sidebar-o #page-header .overlay-header {\n    left: 250px;\n  }',
    );
    expect(globals).toContain(
      '  #page-container.page-header-fixed.sidebar-mini.sidebar-o #page-header .overlay-header,\n  #page-container.page-header-glass.sidebar-mini.sidebar-o #page-header .overlay-header {\n    left: 64px;\n  }',
    );
    expect(globals).toContain(
      '  .sidebar-mini.sidebar-o #sidebar:not(:hover) .nav-main > .nav-main-item > .nav-main-submenu {\n    display: none;\n  }',
    );
    expect(globals).toContain(
      '.side-trans-enabled #sidebar {\n  transition: transform 0.45s cubic-bezier(0.2, 0.61, 0.42, 0.97);\n}',
    );
    expect(globals).toContain(
      '.side-trans-enabled #side-overlay {\n  transition:\n    transform 0.45s cubic-bezier(0.2, 0.61, 0.42, 0.97),\n    opacity 0.45s cubic-bezier(0.2, 0.61, 0.42, 0.97);\n}',
    );
    expect(globals).toContain(
      '.dropdown-menu {\n  position: absolute;\n  top: 100%;\n  left: 0;\n  z-index: 1000;',
    );
    expect(globals).toContain('min-width: 12rem;\n  padding: 0.5rem 0;\n  margin: 0.125rem 0 0;');
    expect(globals).toContain('.dropdown-menu-right {\n  right: 0;\n  left: auto;\n}');
    expect(globals).toContain('.dropdown-menu.show {\n  display: block;\n}');
    expect(globals).toContain(
      '.dropdown-item {\n  display: block;\n  width: 100%;\n  clear: both;',
    );
    expect(globals).toContain(
      '.dropdown-item:focus,\n.dropdown-item:hover {\n  color: #495057;\n  text-decoration: none;',
    );
  });

  it('keeps packaged OneUI row, block, and pull spacing helpers', () => {
    const globals = css();

    expect(globals).toContain('.row {\n  display: flex;\n  flex-wrap: wrap;\n  margin-right: -14px;');
    expect(globals).toContain(
      ".row > .col,\n.row > [class*='col-'] {\n  position: relative;\n  width: 100%;\n  padding-right: 14px;",
    );
    expect(globals).toContain('.no-gutters {\n  margin-right: 0;\n  margin-left: 0;\n}');
    expect(globals).toContain(
      ".no-gutters > .col,\n.no-gutters > [class*='col-'] {\n  padding-right: 0;\n  padding-left: 0;\n}",
    );
    expect(globals).toContain(
      '.row.gutters-tiny {\n  margin-right: -0.125rem;\n  margin-left: -0.125rem;\n}',
    );
    expect(globals).toContain(
      "  .row:not(.gutters-tiny):not(.no-gutters) > .col,\n  .row:not(.gutters-tiny):not(.no-gutters) > [class*='col-'] {\n    padding-right: 0.4375rem;",
    );
    expect(globals).toContain(
      '.row.gutters-tiny .block,\n.row.gutters-tiny.items-push > div,\n.row.gutters-tiny .push {\n  margin-bottom: 0.25rem;\n}',
    );
    expect(globals.indexOf('.row {\n  display: flex;')).toBeLessThan(
      globals.indexOf('.no-gutters {\n  margin-right: 0;'),
    );
    expect(globals.indexOf('.no-gutters > .col')).toBeLessThan(
      globals.indexOf('.row.gutters-tiny {\n  margin-right: -0.125rem;'),
    );
    expect(globals.indexOf('.row.gutters-tiny .block')).toBeLessThan(
      globals.indexOf('@media (max-width: 767.98px) {\n  .row:not(.gutters-tiny):not(.no-gutters)'),
    );
    expect(globals).toContain(
      '.row.row-deck > div {\n  display: flex;\n  align-items: stretch;\n}',
    );
    expect(globals).toContain('.row.row-deck > div > .block {\n  min-width: 100%;\n}');
    expect(globals).toContain('.form-row {\n  display: flex;\n  flex-wrap: wrap;\n  margin-right: -5px;');
    expect(globals).toContain('.col-3 {\n  flex: 0 0 25%;\n  max-width: 25%;\n}');
    expect(globals).toContain('.col-9 {\n  flex: 0 0 75%;\n  max-width: 75%;\n}');
    expect(globals).toContain(
      '.col-12,\n.col-md-12,\n.col-lg-12,\n.col-xl-12 {\n  flex: 0 0 100%;\n  max-width: 100%;\n}',
    );
    expect(globals).toContain('  .col-sm-12 {\n    flex: 0 0 100%;\n    max-width: 100%;\n  }');
    expect(globals).toContain('  .col-md-4 {\n    flex: 0 0 33.333333%;\n    max-width: 33.333333%;\n  }');
    expect(globals).toContain('  .col-md-8 {\n    flex: 0 0 66.666667%;\n    max-width: 66.666667%;\n  }');
    expect(globals).toContain('  .col-xl-4 {\n    flex: 0 0 33.333333%;\n    max-width: 33.333333%;\n  }');
    expect(globals.indexOf('.col-3 {\n  flex: 0 0 25%;')).toBeLessThan(
      globals.indexOf('.col-12,\n.col-md-12,'),
    );
    expect(globals.indexOf('.col-sm-12 {\n    flex: 0 0 100%;')).toBeLessThan(
      globals.indexOf('.col-md-4 {\n    flex: 0 0 33.333333%;'),
    );
    expect(globals.indexOf('.col-md-8 {\n    flex: 0 0 66.666667%;')).toBeLessThan(
      globals.indexOf('.col-xl-4 {\n    flex: 0 0 33.333333%;'),
    );
    expect(globals).toContain('.block .block,\n.content-side .block {\n  box-shadow: none;\n}');
    expect(globals).toContain('.block.block-rounded {\n  border-radius: 0.25rem;\n}');
    expect(globals).toContain(
      '.block.block-rounded > .block-content:last-child {\n  border-bottom-right-radius: 0.2rem;\n  border-bottom-left-radius: 0.2rem;\n}',
    );
    expect(globals).toContain(
      '.block.block-fx-shadow {\n  box-shadow: 0 0 2.25rem var(--legacy-block-shadow-hover);\n  opacity: 1;\n}',
    );
    expect(globals).toContain('.block-content > .pull {\n  margin: -1.25rem -1.25rem -1px;\n}');
    expect(globals).toContain(
      '.block-content.block-content-full > .pull,\n.block-content.block-content-full > .pull-b,\n.block-content.block-content-full > .pull-y {\n  margin-bottom: -1.25rem;\n}',
    );
    expect(globals).toContain(
      '.content-heading {\n  padding-top: 1rem;\n  padding-bottom: 0.5rem;\n  margin-bottom: 0.875rem;',
    );
    expect(globals).toContain(
      '.block-content .block,\n.block-content .items-push > div,\n.block-content .push,\n.block-content p {\n  margin-bottom: 1.25rem;\n}',
    );
    expect(globals).toContain(
      '.content {\n  width: 100%;\n  margin: 0 auto;\n  padding: 0.875rem 0.875rem 1px;',
    );
    expect(globals).toContain('.content > .pull {\n  margin: -0.875rem -0.875rem -1px;\n}');
    expect(globals).toContain(
      '.content.content-full > .pull,\n.content.content-full > .pull-b,\n.content.content-full > .pull-y {\n  margin-bottom: -0.875rem;\n}',
    );
    expect(globals).toContain('  .content {\n    padding: 1.75rem 1.75rem 1px;\n  }');
    expect(globals).toContain('  .content > .pull {\n    margin: -1.75rem -1.75rem -1px;\n  }');
    expect(globals).toContain('#page-container .content {\n  background-color: var(--color-page) !important;\n}');
    expect(globals).toContain('  .content {\n    padding: 0 !important;\n  }');
    expect(globals).toContain(
      '.content .block,\n.content .items-push > div,\n.content .push,\n.content p {\n  margin-bottom: 0.875rem;\n}',
    );
    expect(globals).toContain('.content-side > .pull {\n  margin: -1.125rem -1.125rem -1px;\n}');
    expect(globals).toContain(
      '.content-side.content-side-full > .pull,\n.content-side.content-side-full > .pull-b,\n.content-side.content-side-full > .pull-y {\n  margin-bottom: -1.125rem;\n}',
    );
    expect(globals).toContain(
      '.content-side .block,\n.content-side .items-push > div,\n.content-side .push,\n.content-side p {\n  margin-bottom: 1.125rem;\n}',
    );
    expect(
      globals.indexOf('.block-content .block,\n.block-content .items-push > div'),
    ).toBeGreaterThan(globals.indexOf('.content .block,\n.content .items-push > div'));
  });

  it('keeps packaged OneUI block loading overlay and spinner icon', () => {
    const globals = css();

    expect(globals).toContain(
      '.block.block-mode-loading {\n  position: relative;\n  overflow: hidden;\n}',
    );
    expect(globals).toContain(
      ".block.block-mode-loading::before {\n  position: absolute;\n  inset: 0;\n  z-index: 9;\n  display: block;\n  content: ' ';",
    );
    expect(globals).toContain(
      ".block.block-mode-loading::after {\n  position: absolute;\n  top: 50%;\n  left: 50%;\n  z-index: 10;",
    );
    expect(globals).toContain("content: '\\e09a';");
    expect(globals).toContain('animation: fa-spin 1.75s linear infinite;');
    expect(globals).toContain(
      '@keyframes fa-spin {\n  0% {\n    transform: rotate(0deg);\n  }',
    );
    expect(globals.indexOf('animation: fa-spin 1.75s linear infinite;')).toBeLessThan(
      globals.indexOf('@keyframes fa-spin'),
    );
  });

  it('keeps packaged OneUI sidebar navigation link, submenu, and dark variants', () => {
    const globals = css();

    expect(globals).toContain(
      '.nav-main-heading {\n  padding-top: 1.75rem;\n  padding-bottom: 0.25rem;\n  padding-left: 0.625rem;',
    );
    expect(globals).toContain(
      '.nav-main-link {\n  position: relative;\n  display: flex;\n  align-items: center;\n  min-height: 2.25rem;',
    );
    expect(globals).toContain(
      '.nav-main-link .nav-main-link-icon {\n  display: inline-block;\n  flex: 0 0 auto;',
    );
    expect(globals).toContain(
      '.nav-main-link.active,\n.nav-main-link:hover {\n  color: #000;\n  background-color: var(--legacy-active-bg);\n}',
    );
    expect(globals).toContain(
      '.nav-main-submenu {\n  padding-left: 2.5rem;\n  height: 0;\n  overflow: hidden;',
    );
    expect(globals).toContain(
      '.nav-main-submenu .nav-main-link {\n  min-height: 2rem;\n  margin: 0;\n  padding-top: 0.375rem;',
    );
    expect(globals).toContain(
      '.nav-main-submenu .nav-main-link.active,\n.nav-main-submenu .nav-main-link:hover {\n  color: var(--legacy-nav-submenu-link-hover);',
    );
    expect(globals).toContain(
      '.nav-main-item.open > .nav-main-submenu {\n  height: auto;\n  margin-top: -2px;',
    );
    expect(globals).toContain(
      '.nav-main-submenu .nav-main-item.open .nav-main-link {\n  background-color: transparent;\n}',
    );
    expect(globals).toContain(
      '.nav-main-horizontal.nav-main-hover .nav-main-item:hover > .nav-main-link-submenu {\n  color: #000;\n  background-color: var(--legacy-active-bg);\n}',
    );
    expect(globals.indexOf('.nav-main-submenu {\n  padding-left: 2.5rem;')).toBeLessThan(
      globals.indexOf('.nav-main-submenu .nav-main-link {'),
    );
    expect(globals.indexOf('.nav-main-submenu .nav-main-link.active')).toBeLessThan(
      globals.indexOf('.nav-main-item.open > .nav-main-link-submenu {'),
    );
    expect(globals.indexOf('.nav-main-horizontal.nav-main-hover')).toBeLessThan(
      globals.indexOf('.sidebar-dark #sidebar .nav-main-link:hover'),
    );
    expect(globals).toContain(
      '.sidebar-dark #sidebar .nav-main-link:hover {\n  color: #fff;\n  background-color: var(--legacy-nav-dark-active-bg);\n}',
    );
    expect(globals).toContain(
      '.sidebar-dark #sidebar .nav-main-submenu .nav-main-link {\n  color: var(--legacy-nav-dark-submenu-link);\n}',
    );
    expect(globals).toContain(
      '.sidebar-dark #sidebar .nav-main-item.open > .nav-main-link-submenu {\n  color: #fff;\n  background-color: var(--legacy-nav-dark-active-bg);\n}',
    );
    expect(globals).toContain(
      '.sidebar-dark\n  #sidebar\n  .nav-main-horizontal.nav-main-hover\n  .nav-main-item:hover\n  > .nav-main-submenu {\n  background-color: var(--legacy-nav-dark-horizontal-bg);\n}',
    );
    expect(globals).toContain(
      '.v2board-nav-mask {\n  position: fixed;\n  inset: 0;\n  z-index: 999;',
    );
  });
});

describe('legacy antd feedback CSS', () => {
  it('keeps the restored antd Spin dot layout and motion from antd v3', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-spin {\n  position: absolute;\n  display: none;\n  color: #0665d0;',
    );
    expect(globals).toContain(
      '.ant-spin-nested-loading > div > .ant-spin {\n  position: absolute;\n  top: 0;\n  left: 0;\n  z-index: 4;',
    );
    expect(globals).toContain(
      '.ant-spin-container:after {\n  position: absolute;\n  top: 0;\n  right: 0;\n  bottom: 0;',
    );
    expect(globals).toContain(
      '.ant-spin-blur {\n  clear: both;\n  overflow: hidden;\n  background: #fff;',
    );
    expect(globals).toContain(
      '.ant-spin-dot {\n  position: relative;\n  display: inline-block;\n  width: 1em;\n  height: 1em;',
    );
    expect(globals).toContain(
      '.ant-spin-dot-item {\n  position: absolute;\n  display: block;\n  width: 9px;\n  height: 9px;',
    );
    expect(globals).toContain('animation: antSpinMove 1s linear infinite alternate;');
    expect(globals).toContain(
      '.ant-spin-dot-item:nth-child(4) {\n  bottom: 0;\n  left: 0;\n  animation-delay: 1.2s;\n}',
    );
    expect(globals).toContain(
      '.ant-spin-dot-spin {\n  transform: rotate(45deg);\n  animation: antRotate 1.2s linear infinite;\n}',
    );
    expect(globals.indexOf('.ant-spin-dot {\n  position: relative;')).toBeLessThan(
      globals.indexOf('.ant-spin-dot-item {\n  position: absolute;'),
    );
    expect(globals.indexOf('.ant-spin-dot-item {\n  position: absolute;')).toBeLessThan(
      globals.indexOf('.ant-spin-dot-item:first-child {'),
    );
    expect(globals.indexOf('.ant-spin-dot-item:nth-child(4)')).toBeLessThan(
      globals.indexOf('.ant-spin-dot-spin {'),
    );
    expect(globals.indexOf('.ant-spin-dot-spin {')).toBeLessThan(
      globals.indexOf('@keyframes antSpinMove'),
    );
  });

  it('keeps drawer placement, chrome, and mask motion aligned with antd v3', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-drawer-left,\n.ant-drawer-right {\n  top: 0;\n  width: 0;\n  height: 100%;\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-left .ant-drawer-content-wrapper {\n  left: 0;\n  transform: translateX(-100%);\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-right .ant-drawer-content-wrapper {\n  right: 0;\n  transform: translateX(100%);\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-left.ant-drawer-open,\n.ant-drawer-right.ant-drawer-open {\n  width: 100%;\n  transition: transform 0.3s cubic-bezier(0.7, 0.3, 0.1, 1);\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-left.ant-drawer-open.no-mask,\n.ant-drawer-right.ant-drawer-open.no-mask {\n  width: 0;\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-left.ant-drawer-open .ant-drawer-content-wrapper {\n  transform: translateX(0);\n  box-shadow: 2px 0 8px rgba(0, 0, 0, 0.15);\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-right.ant-drawer-open.no-mask {\n  right: 1px;\n  transform: translateX(1px);\n}',
    );
    expect(globals.indexOf('.ant-drawer-left,\n.ant-drawer-right {')).toBeLessThan(
      globals.indexOf('.ant-drawer-left .ant-drawer-content-wrapper {'),
    );
    expect(
      globals.indexOf('.ant-drawer-left.ant-drawer-open,\n.ant-drawer-right.ant-drawer-open {'),
    ).toBeLessThan(
      globals.indexOf('.ant-drawer-left.ant-drawer-open .ant-drawer-content-wrapper'),
    );
    expect(globals.indexOf('.ant-drawer-right {\n  right: 0;')).toBeLessThan(
      globals.indexOf('.ant-drawer-bottom,\n.ant-drawer-top {'),
    );
    expect(globals).toContain(
      '.ant-drawer-bottom,\n.ant-drawer-top {\n  left: 0;\n  width: 100%;\n  height: 0%;\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-bottom.ant-drawer-open.no-mask,\n.ant-drawer-top.ant-drawer-open.no-mask {\n  height: 0%;\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-top .ant-drawer-content-wrapper {\n  top: 0;\n  transform: translateY(-100%);\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-bottom.ant-drawer-open .ant-drawer-content-wrapper {\n  transform: translateY(0);\n  box-shadow: 0 -2px 8px rgba(0, 0, 0, 0.15);\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-bottom.ant-drawer-open.no-mask {\n  bottom: 1px;\n  transform: translateY(1px);\n}',
    );
    expect(globals.indexOf('.ant-drawer-bottom,\n.ant-drawer-top {')).toBeLessThan(
      globals.indexOf('.ant-drawer-top {'),
    );
    expect(globals.indexOf('.ant-drawer-top.ant-drawer-open .ant-drawer-content-wrapper')).toBeLessThan(
      globals.indexOf('.ant-drawer-bottom {'),
    );
    expect(globals).toContain(
      '.ant-drawer-title {\n  margin: 0;\n  color: rgba(0, 0, 0, 0.85);\n  font-weight: 500;',
    );
    expect(globals).toContain(
      '.ant-drawer-content {\n  position: relative;\n  z-index: 1;\n  overflow: auto;',
    );
    expect(globals).toContain(
      '.ant-drawer-close {\n  position: absolute;\n  top: 0;\n  right: 0;\n  z-index: 10;',
    );
    expect(globals).toContain(
      '.ant-drawer-header {\n  position: relative;\n  padding: 16px 24px;\n  color: rgba(0, 0, 0, 0.65);',
    );
    expect(globals).toContain(
      '.ant-drawer-wrapper-body {\n  height: 100%;\n  overflow: auto;\n}',
    );
    expect(globals).toContain(
      '.ant-drawer-mask {\n  position: absolute;\n  top: 0;\n  left: 0;\n  width: 100%;\n  height: 0;',
    );
    expect(globals).toContain(
      'animation: antdDrawerFadeIn 0.3s cubic-bezier(0.7, 0.3, 0.1, 1);',
    );
    expect(globals).toContain(
      '.ant-drawer.ant-drawer-open .ant-drawer-mask {\n  height: 100%;\n  opacity: 1;',
    );
  });

  it('keeps modal shell, section spacing, and confirm layout aligned with antd v3', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-modal {\n  box-sizing: border-box;\n  position: relative;\n  top: 100px;\n  width: auto;',
    );
    expect(globals).toContain(
      '.ant-modal-wrap {\n  position: fixed;\n  top: 0;\n  right: 0;\n  bottom: 0;\n  left: 0;',
    );
    expect(globals).toContain(
      '.ant-modal-mask {\n  position: fixed;\n  top: 0;\n  right: 0;\n  bottom: 0;',
    );
    expect(globals).toContain(
      '.ant-modal-content {\n  position: relative;\n  background-color: #fff;\n  background-clip: padding-box;\n  border: 0;',
    );
    expect(globals).toContain(
      '.ant-modal-close-x {\n  display: block;\n  width: 56px;\n  height: 56px;',
    );
    expect(globals).toContain(
      '.ant-modal-centered .ant-modal {\n  top: 0;\n  display: inline-block;\n  text-align: left;',
    );
    expect(globals).toContain('  .ant-modal {\n    max-width: calc(100vw - 16px);\n    margin: 8px auto;\n  }');
    expect(globals).toContain(
      '.ant-modal-footer {\n  padding: 10px 16px;\n  text-align: right;\n  background: transparent;',
    );
    expect(globals).toContain(
      '.ant-modal-confirm .ant-modal-body {\n  padding: 32px 32px 24px;\n}',
    );
    expect(globals).toContain('.ant-modal-confirm .ant-modal-header {\n  display: none;\n}');
    expect(globals).toContain(
      ".ant-modal-confirm-body-wrapper::before,\n.ant-modal-confirm-body-wrapper::after {\n  display: table;\n  content: '';",
    );
    expect(globals).toContain(
      '.ant-modal-confirm-body > .anticon + .ant-modal-confirm-title + .ant-modal-confirm-content {\n  margin-left: 38px;\n}',
    );
    expect(globals).toContain('.ant-modal-confirm-btns {\n  float: right;\n  margin-top: 24px;\n}');
    expect(globals).toContain(
      '.ant-modal-confirm-confirm .ant-modal-confirm-body > .anticon,\n.ant-modal-confirm-warning .ant-modal-confirm-body > .anticon {\n  color: #faad14;',
    );
  });

  it('keeps carousel slick track, arrows, and dots aligned with antd v3', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-carousel .slick-list {\n  position: relative;\n  display: block;\n  margin: 0;\n  padding: 0;\n  overflow: hidden;\n}',
    );
    expect(globals).toContain(
      '.ant-carousel .slick-list .slick-slide input.ant-checkbox-input,\n.ant-carousel .slick-list .slick-slide input.ant-radio-input {\n  visibility: hidden;\n}',
    );
    expect(globals).toContain(
      '.ant-carousel .slick-list .slick-slide.slick-active input.ant-checkbox-input,\n.ant-carousel .slick-list .slick-slide.slick-active input.ant-radio-input {\n  visibility: visible;\n}',
    );
    expect(globals).toContain(
      '.ant-carousel .slick-track::before,\n.ant-carousel .slick-track::after {\n  display: table;\n  content: \'\';\n}',
    );
    expect(globals).toContain(
      '.ant-carousel .slick-vertical .slick-slide {\n  display: block;\n  height: auto;\n  border: 1px solid transparent;\n}',
    );
    expect(globals).toContain(
      '.ant-carousel .slick-next,\n.ant-carousel .slick-prev {\n  position: absolute;\n  top: 50%;',
    );
    expect(globals).toContain(
      '.ant-carousel .slick-next.slick-disabled::before,\n.ant-carousel .slick-prev.slick-disabled::before {\n  opacity: 0.25;\n}',
    );
    expect(globals).toContain(
      ".ant-carousel .slick-prev::before {\n  content: '\\2190';\n}",
    );
    expect(globals).toContain(
      ".ant-carousel .slick-next::before {\n  content: '\\2192';\n}",
    );
    expect(globals).toContain(
      '.ant-carousel .slick-dots {\n  position: absolute;\n  bottom: 12px;\n  display: block;',
    );
    expect(globals).toContain(
      '.ant-carousel .slick-dots li button {\n  display: block;\n  width: 16px;\n  height: 3px;',
    );
    expect(globals).toContain(
      '.ant-carousel .slick-dots li.slick-active button {\n  width: 24px;\n  background: #fff;\n  opacity: 1;\n}',
    );
    expect(globals).toContain(
      '.ant-carousel .slick-dots li.slick-active button:hover,\n.ant-carousel .slick-dots li.slick-active button:focus {\n  opacity: 1;\n}',
    );
  });

  it('keeps antd-mobile list item, accessory, and retina hairline styles aligned', () => {
    const globals = css();

    expect(globals).toContain(
      '.am-list-item {\n  position: relative;\n  display: flex;\n  min-height: 44px;\n  padding-left: 15px;',
    );
    expect(globals).toContain(
      '.am-list-item .am-list-ripple.am-list-ripple-animate {\n  background-color: hsla(0, 0%, 62%, 0.2);\n  animation: ripple 1s linear;\n}',
    );
    expect(globals).toContain(
      '.am-list-item.am-list-item-disabled .am-list-line .am-list-content,\n.am-list-item.am-list-item-disabled .am-list-line .am-list-extra {\n  color: #bbb;\n}',
    );
    expect(globals).toContain('.am-list-item .am-list-thumb:first-child {\n  margin-right: 15px;\n}');
    expect(globals).toContain(
      '.am-list-line-multiple {\n  padding: 12.5px 15px 12.5px 0;\n}',
    );
    expect(globals).toContain(
      '.am-list-line {\n  position: relative;\n  display: flex;\n  flex: 1 1;',
    );
    expect(globals).toContain(
      '.am-list-content {\n  flex: 1 1;\n  width: auto;\n  padding-top: 7px;',
    );
    expect(globals).toContain('.am-list-extra {\n  flex-basis: 36%;\n  width: auto;');
    expect(globals).toContain('.am-list-brief {\n  width: auto;\n  margin-top: 6px;');
    expect(globals).toContain(
      '.am-list-arrow-horizontal {\n  visibility: visible;\n}',
    );
    expect(globals).toContain(
      '@media (-webkit-min-device-pixel-ratio: 2), (min-resolution: 2dppx) {\n  html:not([data-scale]) .am-list-body {\n    border-top: none;',
    );
    expect(globals).toContain(
      "html:not([data-scale]) .am-list-body::before,\n  html:not([data-scale]) .am-list-body::after,\n  html:not([data-scale]) .am-list-body div:not(:last-child) .am-list-line::after {\n    position: absolute;",
    );
    expect(globals).toContain(
      'html:not([data-scale]) .am-list-body::before {\n    top: 0;\n    right: auto;\n    bottom: auto;',
    );
    expect(globals).toContain(
      'html:not([data-scale]) .am-list-body::after {\n    top: auto;\n    right: auto;\n    bottom: 0;',
    );
    expect(globals).toContain(
      'html:not([data-scale]) .am-list-body div:not(:last-child) .am-list-line {\n    border-bottom: none;\n  }',
    );
    expect(globals).toContain(
      'html:not([data-scale]) .am-list-body div:not(:last-child) .am-list-line::after {\n    position: absolute;',
    );
    expect(globals).toContain(
      '@media (-webkit-min-device-pixel-ratio: 2) and (-webkit-min-device-pixel-ratio: 3),\n  (min-resolution: 2dppx) and (min-resolution: 3dppx) {',
    );
    expect(globals).toContain('transform: scaleY(0.33);');
    expect(globals).toContain('animation: ripple 1s linear;');
    expect(globals).toContain('@keyframes ripple {');
    expect(
      globals.indexOf('html:not([data-scale]) .am-list-body {\n    border-top: none;'),
    ).toBeGreaterThan(globals.indexOf('.am-list-arrow-horizontal {'));
    expect(
      globals.indexOf('html:not([data-scale]) .am-list-body::before {\n    top: 0;'),
    ).toBeGreaterThan(
      globals.indexOf(
        'html:not([data-scale]) .am-list-body div:not(:last-child) .am-list-line::after {\n    position: absolute;',
      ),
    );
    expect(globals.indexOf('transform: scaleY(0.33);')).toBeGreaterThan(
      globals.indexOf(
        'html:not([data-scale]) .am-list-body div:not(:last-child) .am-list-line {\n    border-bottom: none;',
      ),
    );
    expect(globals.indexOf('@keyframes ripple {')).toBeGreaterThan(
      globals.indexOf('transform: scaleY(0.33);'),
    );
    expect(globals).toContain('  .block {\n    margin-bottom: 0 !important;\n    background-color: unset;\n  }');
    expect(globals).toContain('  .v2board-select-radio {\n    display: unset;\n  }');
    expect(globals).toContain('  .ant-notification {\n    top: 0 !important;\n    right: 0;');
    expect(globals).toContain(
      '  #cashier .ant-radio-button-wrapper {\n    width: 100%;\n    margin-top: 10px;\n  }',
    );
    expect(globals).toContain('  .v2board-knowledge-search-bar button {\n    border-radius: 0;\n  }');
  });

  it('keeps message and notification feedback chrome aligned with antd v3', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-message {\n  box-sizing: border-box;\n  margin: 0;\n  padding: 0;',
    );
    expect(globals).toContain(
      '.ant-message-notice-content {\n  display: inline-block;\n  padding: 10px 16px;\n  background: #fff;',
    );
    expect(globals).toContain(
      '.ant-message .anticon {\n  position: relative;\n  top: 1px;\n  margin-right: 8px;',
    );
    expect(globals).toContain('.ant-message-success .anticon {\n  color: #52c41a;\n}');
    expect(globals).toContain(
      '.ant-message-info .anticon,\n.ant-message-loading .anticon {\n  color: #1890ff;\n}',
    );
    expect(globals).toContain(
      '.ant-notification {\n  box-sizing: border-box;\n  padding: 0;\n  color: rgba(0, 0, 0, 0.65);',
    );
    expect(globals).toContain(
      '.ant-notification-notice {\n  position: relative;\n  margin-bottom: 16px;\n  padding: 16px 24px;',
    );
    expect(globals).toContain(
      '.ant-notification-notice-with-icon .ant-notification-notice-message {\n  margin-bottom: 4px;\n  margin-left: 48px;',
    );
    expect(globals).toContain('.anticon.ant-notification-notice-icon-warning {\n  color: #faad14;\n}');
    expect(globals).toContain(
      '.ant-notification-notice-close {\n  position: absolute;\n  top: 16px;\n  right: 22px;',
    );
    expect(globals).toContain('.ant-result {\n  padding: 48px 32px;\n}');
    expect(globals).toContain(
      '.ant-result-icon > .anticon {\n  font-size: 72px;\n}',
    );
    expect(globals).toContain(
      '.ant-result-success .ant-result-icon > .anticon {\n  color: #52c41a;\n}',
    );
    expect(globals).toContain(
      '.ant-result-title {\n  color: rgba(0, 0, 0, 0.85);\n  font-size: 24px;',
    );
    expect(globals).toContain(
      '.ant-result-extra > :last-child {\n  margin-right: 0;\n}',
    );
  });

  it('keeps badge status layout aligned with the packaged cascade', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-badge {\n  box-sizing: border-box;\n  position: relative;\n  display: inline-block;\n  margin: 0;\n  padding: 0;\n  color: rgba(0, 0, 0, 0.65);\n  font-size: 14px;\n  font-variant: tabular-nums;\n  line-height: 1.5;',
    );
    expect(globals).toContain(
      '.ant-badge-status {\n  line-height: inherit;\n  vertical-align: baseline;\n}',
    );
    expect(globals).toContain(
      '.ant-badge-status-processing::after {\n  position: absolute;\n  top: 0;\n  left: 0;\n  width: 100%;\n  height: 100%;',
    );
    expect(globals).toContain('.ant-badge-status-text {\n  margin-left: 8px;');
    expect(globals).toContain('.ant-badge-status-warning {\n  background-color: #faad14;\n}');
    expect(globals).toContain('.ant-badge-not-a-wrapper .ant-badge-count {\n  transform: none;\n}');
    expect(globals).toContain(
      '.anticon {\n  display: inline-block;\n  color: inherit;\n  font-style: normal;\n  line-height: 0;',
    );
    expect(globals).toContain(
      '.anticon-spin {\n  display: inline-block;\n  animation: loadingCircle 1s linear infinite;\n}',
    );
    expect(globals).toContain('@keyframes loadingCircle {');
  });

  it('keeps tooltip bubble, arrow, and zoom-big-fast motion rules from antd v3', () => {
    const globals = css();

    expect(globals).toContain(
      '.ant-tooltip {\n  box-sizing: border-box;\n  position: absolute;\n  z-index: 1060;',
    );
    expect(globals).toContain(
      '.ant-tooltip-inner {\n  min-width: 30px;\n  min-height: 32px;\n  padding: 6px 8px;\n  color: #fff;',
    );
    expect(globals).toContain(
      '.ant-tooltip-arrow {\n  position: absolute;\n  display: block;\n  width: 13.07106781px;',
    );
    expect(globals).toContain(
      ".ant-tooltip-arrow::before {\n  position: absolute;\n  inset: 0;\n  display: block;",
    );
    expect(globals).toContain(
      '.ant-tooltip-placement-top .ant-tooltip-arrow::before,\n.ant-tooltip-placement-topLeft .ant-tooltip-arrow::before,\n.ant-tooltip-placement-topRight .ant-tooltip-arrow::before {\n  box-shadow: 3px 3px 7px rgba(0, 0, 0, 0.07);',
    );
    expect(globals).toContain(
      '.ant-tooltip-placement-top .ant-tooltip-arrow {\n  left: 50%;\n  transform: translateX(-50%);\n}',
    );
    expect(globals).toContain(
      '.ant-tooltip-placement-topRight .ant-tooltip-arrow {\n  right: 13px;\n}',
    );
    expect(globals.indexOf('.ant-tooltip-arrow {\n  position: absolute;')).toBeLessThan(
      globals.indexOf('.ant-tooltip-placement-top .ant-tooltip-arrow,'),
    );
    expect(globals.indexOf('.ant-tooltip-placement-top .ant-tooltip-arrow::before')).toBeLessThan(
      globals.indexOf('.ant-tooltip-placement-top .ant-tooltip-arrow {\n  left: 50%;'),
    );
    expect(globals.indexOf('.ant-tooltip-placement-topRight .ant-tooltip-arrow')).toBeLessThan(
      globals.indexOf('@keyframes antZoomBigIn'),
    );
    expect(globals).toContain(
      '@keyframes antZoomBigIn {\n  0% {\n    transform: scale(0.8);\n    opacity: 0;',
    );
    expect(globals).toContain(
      '@keyframes antZoomBigOut {\n  0% {\n    transform: scale(1);\n  }',
    );
    expect(globals).toContain(
      '.zoom-big-fast-enter,\n.zoom-big-fast-appear {\n  transform: scale(0);\n  opacity: 0;',
    );
    expect(globals).toContain(
      '.zoom-big-fast-leave {\n  animation-duration: 0.1s;\n  animation-fill-mode: both;\n  animation-play-state: paused;',
    );
    expect(globals).toContain(
      '.zoom-big-fast-enter.zoom-big-fast-enter-active,\n.zoom-big-fast-appear.zoom-big-fast-appear-active {\n  animation-name: antZoomBigIn;\n  animation-play-state: running;\n}',
    );
    expect(globals).toContain(
      '.zoom-big-fast-leave.zoom-big-fast-leave-active {\n  animation-name: antZoomBigOut;\n  animation-play-state: running;\n  pointer-events: none;\n}',
    );
    expect(globals.indexOf('@keyframes antZoomBigIn')).toBeLessThan(
      globals.indexOf('@keyframes antZoomBigOut'),
    );
    expect(globals.indexOf('@keyframes antZoomBigOut')).toBeLessThan(
      globals.indexOf('.zoom-big-fast-enter,\n.zoom-big-fast-appear {'),
    );
    expect(globals.indexOf('.zoom-big-fast-leave {')).toBeLessThan(
      globals.indexOf('.zoom-big-fast-enter.zoom-big-fast-enter-active'),
    );
    expect(globals.indexOf('.zoom-big-fast-enter.zoom-big-fast-enter-active')).toBeLessThan(
      globals.indexOf('.zoom-big-fast-leave.zoom-big-fast-leave-active'),
    );
  });

  it('keeps fade and zoom motion initial states aligned with antd v3', () => {
    const globals = css();

    expect(globals).toContain('@keyframes antFadeIn {\n  0% {\n    opacity: 0;\n  }');
    expect(globals).toContain(
      '@keyframes antZoomIn {\n  0% {\n    transform: scale(0.2);\n    opacity: 0;',
    );
    expect(globals).toContain(
      '@keyframes antSlideDownIn {\n  0% {\n    transform: scaleY(0.8);\n    transform-origin: 100% 100%;',
    );
    expect(globals).toContain('@keyframes antFadeOut {\n  0% {\n    opacity: 1;\n  }');
    expect(globals).toContain(
      '@keyframes antZoomOut {\n  0% {\n    transform: scale(1);\n  }',
    );
    expect(globals.indexOf('@keyframes antFadeIn')).toBeLessThan(
      globals.indexOf('@keyframes antZoomIn'),
    );
    expect(globals.indexOf('@keyframes antZoomIn')).toBeLessThan(
      globals.indexOf('@keyframes antSlideDownIn'),
    );
    expect(globals.indexOf('@keyframes antSlideDownIn')).toBeLessThan(
      globals.indexOf('@keyframes antFadeOut'),
    );
    expect(globals.indexOf('@keyframes antFadeOut')).toBeLessThan(
      globals.indexOf('@keyframes antZoomOut'),
    );
    expect(globals).toContain(
      '.fade-appear,\n.fade-enter {\n  opacity: 0;\n  animation-timing-function: linear;\n}',
    );
    expect(globals).toContain(
      '.fade-appear.fade-appear-active,\n.fade-enter.fade-enter-active {\n  animation-name: antFadeIn;\n  animation-play-state: running;\n}',
    );
    expect(globals).toContain('.fade-leave {\n  animation-timing-function: linear;\n}');
    expect(globals).toContain(
      '.zoom-appear,\n.zoom-enter {\n  transform: scale(0);\n  opacity: 0;\n  animation-timing-function: cubic-bezier(0.08, 0.82, 0.17, 1);\n}',
    );
    expect(globals).toContain(
      '.zoom-leave.zoom-leave-active {\n  animation-name: antZoomOut;\n  animation-play-state: running;\n  pointer-events: none;\n}',
    );
    expect(globals).toContain(
      '.zoom-leave {\n  animation-timing-function: cubic-bezier(0.78, 0.14, 0.15, 0.86);\n}',
    );
    expect(globals).toContain(
      '.ant-modal.zoom-appear,\n.ant-modal.zoom-enter {\n  transform: none;\n  opacity: 0;\n  animation-duration: 0.3s;',
    );
    expect(globals.indexOf('.ant-modal.zoom-appear')).toBeGreaterThan(
      globals.indexOf('.zoom-appear,\n.zoom-enter {\n  transform: scale(0);'),
    );
  });
});
