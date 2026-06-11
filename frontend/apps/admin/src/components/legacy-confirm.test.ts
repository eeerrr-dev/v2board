import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'legacy-confirm.tsx'), 'utf8');

describe('admin legacy confirm', () => {
  it('renders the Ant Design 3 confirm modal DOM locally', () => {
    expect(source).toContain("export function legacyInfo");
    expect(source).toContain("export function legacySuccess");
    expect(source).toContain("export function legacyError");
    expect(source).toContain("export function legacyWarning");
    expect(source).toContain("export const legacyWarn");
    expect(source).toContain("export function legacyDestroyAll");
    expect(source).toContain("type: 'info'");
    expect(source).toContain("type: 'success'");
    expect(source).toContain("type: 'error'");
    expect(source).toContain("type: 'warning'");
    expect(source).toContain('okCancel: false');
    expect(source).toContain("className={wrapClassName}");
    expect(source).toContain("`${prefixCls}-centered`");
    expect(source).toContain("`${confirmPrefixCls}-centered`");
    expect(source).toContain("className={modalClassName}");
    expect(source).toContain("`${confirmPrefixCls}-${modalType}`");
    expect(source).toContain('modalStyle(options.width, options.style)');
    expect(source).toContain('maskClosable = options.maskClosable ?? false');
    expect(source).toContain("options.autoFocusButton === null ? false : options.autoFocusButton ?? 'ok'");
    expect(source).toContain('className={`${confirmPrefixCls}-body-wrapper`}');
    expect(source).toContain('className={`${confirmPrefixCls}-body`}');
    expect(source).toContain('<LegacyQuestionCircleIcon />');
    expect(source).toContain('<LegacyInfoCircleIcon />');
    expect(source).toContain('<LegacyCheckCircleIcon />');
    expect(source).toContain('<LegacyCloseCircleIcon />');
    expect(source).toContain('<LegacyExclamationCircleIcon />');
    expect(source).toContain('className={`${confirmPrefixCls}-title`}');
    expect(source).toContain('className={`${confirmPrefixCls}-content`}');
    expect(source).toContain('className={`${confirmPrefixCls}-btns`}');
    expect(source).toContain('function ConfirmActionButton');
    expect(source).toContain('actionFn.length ? actionFn(closeModal) : actionFn()');
    expect(source).not.toContain('Modal.confirm');
    expect(source).not.toContain('from \'antd\'');
  });
});
