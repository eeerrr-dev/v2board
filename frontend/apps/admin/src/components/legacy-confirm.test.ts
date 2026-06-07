import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'legacy-confirm.tsx'), 'utf8');

describe('admin legacy confirm', () => {
  it('renders the Ant Design 3 confirm modal DOM locally', () => {
    expect(source).toContain("className=\"ant-modal ant-modal-confirm ant-modal-confirm-confirm\"");
    expect(source).toContain('style={{ width: \'416px\' }}');
    expect(source).toContain('className="ant-modal-confirm-body-wrapper"');
    expect(source).toContain('className="ant-modal-confirm-body"');
    expect(source).toContain('<LegacyQuestionCircleIcon />');
    expect(source).toContain('className="ant-modal-confirm-title"');
    expect(source).toContain('className="ant-modal-confirm-content"');
    expect(source).toContain('className="ant-modal-confirm-btns"');
    expect(source).not.toContain('Modal.confirm');
    expect(source).not.toContain('from \'antd\'');
  });
});
