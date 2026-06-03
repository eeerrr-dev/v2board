import enUS from './en-US';
import type { Translations } from './zh-CN';

const faIR: Translations = {
  ...enUS,
  common: {
    ...enUS.common,
    submit: 'ارسال',
    cancel: 'انصراف',
    confirm: 'تأیید',
    save: 'ذخیره کردن',
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
    items_per_page: '/ صفحه',
    prev_page: 'صفحه قبلی',
    next_page: 'صفحه بعدی',
    prev_5: '۵ صفحه قبلی',
    next_5: '۵ صفحه بعدی',
  },
  auth: {
    ...enUS.auth,
    sign_in: 'ورود',
    sign_up: 'ثبت‌نام',
    email: 'ایمیل',
    password: 'رمز عبور',
    forget_password: 'رمز عبور فراموش شده',
    submit_login: 'ورود',
    submit_register: 'ثبت‌نام',
  },
  dashboard: {
    ...enUS.dashboard,
    no_subscription: 'هیچ گره ای در دسترس نیست، اگر مشترک نیستید یا منقضی شده اید، لطفاً',
  },
  node: {
    ...enUS.node,
    simple_name: 'نام ویژگی محصول',
    status: 'وضعیت',
    status_tip: 'وضعیت آنلاین گره را در عرض پنج دقیقه ثبت کنید',
    rate: 'بزرگنمایی',
    rate_tip: 'جریان استفاده شده در ضریب برای کسر ضرب خواهد شد',
    tags: 'برچسب‌ها',
    renew: 'تمدید',
    subscribe: 'اشتراک',
  },
};

export default faIR;
