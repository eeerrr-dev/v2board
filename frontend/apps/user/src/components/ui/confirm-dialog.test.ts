import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { getConfirmDialogDefaultText } from './confirm-dialog';

const source = readFileSync(`${process.cwd()}/src/components/ui/confirm-dialog.tsx`, 'utf8');

describe('confirm dialog', () => {
  it('keeps localized confirm defaults', () => {
    expect(getConfirmDialogDefaultText('zh-CN')).toEqual({
      confirmText: '确 定',
      cancelText: '取 消',
    });
    expect(getConfirmDialogDefaultText('en-US')).toEqual({
      confirmText: 'OK',
      cancelText: 'Cancel',
    });
  });

  it('uses shadcn alert-dialog primitives without Ant modal compatibility behavior', () => {
    expect(source).toContain("from '@/components/ui/alert-dialog'");
    expect(source).toContain("from '@/components/ui/button'");
    expect(source).toContain('v2board-confirm-dialog');
    expect(source).toContain('v2board-confirm-primary');
    expect(source).toContain('confirmDialog');
    expect(source).not.toContain('legacyConfirm');
    expect(source).not.toContain('ActionButton');
    expect(source).not.toContain('ant-modal');
  });
});
