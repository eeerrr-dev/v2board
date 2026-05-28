import enUS from './en-US';
import type { Translations } from './zh-CN';

const faIR: Translations = {
  ...enUS,
  common: {
    ...enUS.common,
    submit: 'ارسال',
    cancel: 'لغو',
    confirm: 'تأیید',
    save: 'ذخیره',
    delete: 'حذف',
    edit: 'ویرایش',
    search: 'جستجو',
    refresh: 'بازخوانی',
    loading: 'در حال بارگذاری...',
    empty: 'داده‌ای وجود ندارد',
    retry: 'تلاش مجدد',
    back: 'بازگشت',
    close: 'بستن',
    home: 'صفحه اصلی',
    logout: 'خروج',
    language: 'زبان',
    page_not_found: 'صفحه یافت نشد',
  },
  auth: {
    ...enUS.auth,
    sign_in: 'ورود',
    sign_up: 'ثبت‌نام',
    email: 'ایمیل',
    password: 'رمز عبور',
    forget_password: 'فراموشی رمز عبور',
    submit_login: 'ورود',
    submit_register: 'ایجاد حساب',
  },
};

export default faIR;
