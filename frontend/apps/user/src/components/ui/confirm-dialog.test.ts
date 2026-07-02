import { describe, expect, it } from 'vitest';
import { getConfirmDialogDefaultText } from './confirm-dialog';

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
});
