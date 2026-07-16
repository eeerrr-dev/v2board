import { describe, expect, it } from 'vitest';
import { keyFromSelector } from 'i18next';
import { getConfirmDialogDefaultText } from './confirm-dialog';

describe('confirm dialog', () => {
  it('keeps localized confirm defaults', () => {
    const zhCN = {
      'common.confirm': '确定',
      'common.cancel': '取消',
    } as const;
    const enUS = {
      'common.confirm': 'Confirm',
      'common.cancel': 'Cancel',
    } as const;

    expect(
      getConfirmDialogDefaultText(
        (selector) => zhCN[keyFromSelector(selector) as keyof typeof zhCN],
      ),
    ).toEqual({
      confirmText: '确定',
      cancelText: '取消',
    });
    expect(
      getConfirmDialogDefaultText(
        (selector) => enUS[keyFromSelector(selector) as keyof typeof enUS],
      ),
    ).toEqual({
      confirmText: 'Confirm',
      cancelText: 'Cancel',
    });
  });
});
