import { afterEach, describe, expect, it } from 'vitest';
import type { SupportedLocale } from '@v2board/i18n';
import { createI18n } from '@v2board/i18n/testing';
import { createRegisterSchema, forgetSchema, loginSchema } from './auth-validation';

describe('auth validation schemas', () => {
  it('normalizes login email while preserving an eight-character password exactly', () => {
    expect(loginSchema.parse({ email: '  user@example.com  ', password: ' pass123' })).toEqual({
      email: 'user@example.com',
      password: ' pass123',
    });
    expect(loginSchema.safeParse({ email: 'invalid', password: 'password' }).success).toBe(false);
    expect(loginSchema.safeParse({ email: 'user@example.com', password: 'short' }).success).toBe(
      false,
    );
    // Four emoji occupy eight UTF-16 code units but are only four backend-counted characters.
    expect(loginSchema.safeParse({ email: 'user@example.com', password: '😀😀😀😀' }).success).toBe(
      false,
    );
  });

  it('validates a full registration email when no whitelist is active', () => {
    const schema = createRegisterSchema({
      emailCodeRequired: false,
      inviteCodeRequired: false,
    });

    expect(
      schema.parse({
        email: '  user@example.com ',
        email_code: '',
        password: 'password',
        confirm_password: 'password',
        invite_code: '  INVITE  ',
      }),
    ).toEqual({
      email: 'user@example.com',
      email_code: '',
      password: 'password',
      confirm_password: 'password',
      invite_code: 'INVITE',
    });
    expect(
      schema.safeParse({
        email: 'local-part',
        email_code: '',
        password: 'password',
        confirm_password: 'password',
        invite_code: '',
      }).success,
    ).toBe(false);
  });

  it('validates the composed whitelist address and all conditional registration gates', () => {
    const schema = createRegisterSchema({
      emailSuffix: 'example.com',
      emailCodeRequired: true,
      inviteCodeRequired: true,
    });
    const valid = {
      email: 'local.part',
      email_code: '123456',
      password: 'password',
      confirm_password: 'password',
      invite_code: 'INVITE',
    };

    expect(schema.safeParse(valid).success).toBe(true);
    for (const input of [
      { ...valid, email: 'contains@domain' },
      { ...valid, email_code: '12345x' },
      { ...valid, password: 'short', confirm_password: 'short' },
      { ...valid, invite_code: '   ' },
      { ...valid, confirm_password: 'different' },
    ]) {
      expect(schema.safeParse(input).success).toBe(false);
    }
  });

  it('matches forget-password email, password-length, and six-digit code constraints', () => {
    expect(
      forgetSchema.parse({
        email: ' reset@example.com ',
        email_code: ' 123456 ',
        password: 'password',
        confirm_password: 'password',
      }),
    ).toEqual({
      email: 'reset@example.com',
      email_code: '123456',
      password: 'password',
      confirm_password: 'password',
    });
    expect(
      forgetSchema.safeParse({
        email: 'reset@example.com',
        email_code: '12345x',
        password: 'password',
        confirm_password: 'password',
      }).success,
    ).toBe(false);
  });
});

const localizedValidation: Array<
  [SupportedLocale, { email: string; password: string; emailCode: string }]
> = [
  [
    'zh-CN',
    {
      email: '请输入有效邮箱',
      password: '密码至少需要 8 个字符',
      emailCode: '请输入 6 位数字邮箱验证码',
    },
  ],
  [
    'zh-TW',
    {
      email: '請輸入有效的電子郵件地址',
      password: '密碼至少需要 8 個字元',
      emailCode: '請輸入 6 位數字郵箱驗證碼',
    },
  ],
  [
    'en-US',
    {
      email: 'Enter a valid email address.',
      password: 'Password must be at least 8 characters.',
      emailCode: 'Enter the 6-digit email verification code.',
    },
  ],
  [
    'ja-JP',
    {
      email: '有効なメールアドレスを入力してください。',
      password: 'パスワードは8文字以上で入力してください。',
      emailCode: '6桁のメール確認コードを入力してください。',
    },
  ],
  [
    'vi-VN',
    {
      email: 'Nhập địa chỉ email hợp lệ.',
      password: 'Mật khẩu phải có ít nhất 8 ký tự.',
      emailCode: 'Nhập mã xác minh email gồm 6 chữ số.',
    },
  ],
  [
    'ko-KR',
    {
      email: '올바른 이메일 주소를 입력하세요.',
      password: '비밀번호는 8자 이상이어야 합니다.',
      emailCode: '6자리 이메일 인증 코드를 입력하세요.',
    },
  ],
];

describe('auth validation localization', () => {
  afterEach(() => {
    window.localStorage.removeItem('umi_locale');
    window.g_lang = undefined;
  });

  it.each(localizedValidation)('localizes validation copy for %s', async (locale, expected) => {
    window.localStorage.setItem('umi_locale', locale);
    const instance = createI18n();
    await instance.changeLanguage(locale);

    expect(instance.t(($) => $.auth.email_invalid)).toBe(expected.email);
    expect(instance.t(($) => $.auth.password_min)).toBe(expected.password);
    expect(instance.t(($) => $.auth.email_code_invalid)).toBe(expected.emailCode);
  });
});
