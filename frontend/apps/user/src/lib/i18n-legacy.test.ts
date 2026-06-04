import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createI18n, legacyGetLocale } from '@v2board/i18n';

const originalNavigatorLanguage = window.navigator.language;

function setLegacyLocale(locale: string) {
  document.cookie = `i18n=${locale};path=/`;
  window.localStorage.setItem('umi_locale', locale);
}

function setLegacyI18n(
  dictionaries: Record<string, Record<string, string>>,
) {
  const i18n = [] as unknown as string[] & Record<string, Record<string, string>>;
  for (const [locale, dict] of Object.entries(dictionaries)) i18n[locale] = dict;
  window.settings = { i18n };
}

describe('legacy i18n dictionaries', () => {
  beforeEach(() => {
    document.cookie = 'i18n=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    window.localStorage.clear();
    window.settings = undefined;
    window.g_lang = undefined;
    window.g_langSeparator = undefined;
  });

  afterEach(() => {
    Object.defineProperty(window.navigator, 'language', {
      value: originalNavigatorLanguage,
      configurable: true,
    });
  });

  it('keeps the legacy zh-CN source copy when no bundled dictionary is loaded', () => {
    setLegacyLocale('zh-CN');

    const i18n = createI18n();

    expect(i18n.t('order.processing')).toBe('订单系统正在进行处理，请稍等1-3分钟。');
    expect(i18n.t('order.cancel_confirm')).toBe(
      '如果你已经付款，取消订单可能会导致支付失败，确定取消订单吗？',
    );
    expect(i18n.t('order.credit_card_security')).toBe(
      '您的信用卡信息只会被用作当次扣款，系统并不会保存，这是我们认为最安全的。',
    );
    expect(i18n.t('traffic.notice')).toBe('流量明细仅保留近一个月数据以供查询。');
    expect(i18n.t('dashboard.alert_traffic_rate', { rate: 80 })).toBe('当前已使用流量达 80%');
    expect(i18n.t('node.status_tip')).toBe('节点五分钟内节点在线情况');
    expect(i18n.t('ticket.message_placeholder')).toBe('请描述你遇到的问题');
    expect(i18n.t('invite.pending_hint')).toBe('佣金将会在确认后会到达你的佣金账户。');
    expect(i18n.t('plan.pick_title')).toBe('选择最适合你的计划');
    expect(i18n.t('plan.select_other')).toBe('选择其他订阅');
    expect(i18n.t('plan.change_warning')).toBe('变更订阅会导致当前订阅被新订阅覆盖，请注意。');
    expect(i18n.t('plan.unfinished_order_confirm')).toBe(
      '你还有未完成的订单，购买前需要先进行取消，确定取消先前的订单吗？',
    );
    expect(i18n.t('profile.telegram_bind')).toBe('绑定Telegram');
    expect(i18n.t('profile.telegram_search')).toBe('打开Telegram搜索');
    expect(i18n.t('profile.telegram_send')).toBe('向机器人发送你的');
    expect(i18n.t('profile.reset_subscribe_tip')).toBe(
      '如果你的订阅地址或信息泄露可以进行此操作。重置后你的UUID及订阅将会变更，需要重新进行订阅。',
    );
    expect(i18n.t('profile.reset_subscribe_warning')).toBe('重置订阅提示信息');
  });

  it('renders zh-CN through the bundled legacy dictionary when it is loaded', () => {
    setLegacyLocale('zh-CN');
    setLegacyI18n({
      'zh-CN': {
        选择最适合你的计划: '选择最适合您的计划',
        绑定Telegram: '绑定 Telegram',
        打开Telegram搜索: '打开 Telegram 搜索',
        请描述你遇到的问题: '请描述您遇到的问题',
        '订单系统正在进行处理，请稍等1-3分钟。': '订单系统正在进行处理，请等候 1-3 分钟。',
        '如果你已经付款，取消订单可能会导致支付失败，确定取消订单吗？':
          '如果您已经付款，取消订单可能会导致支付失败，确定要取消订单吗？',
        '您的信用卡信息只会被用作当次扣款，系统并不会保存，这是我们认为最安全的。':
          '您的信用卡信息只会用于当次扣款，系统并不会保存，我们认为这是最安全的。',
        选择其他订阅: '选择其它订阅',
        '变更订阅会导致当前订阅被新订阅覆盖，请注意。':
          '请注意，变更订阅会导致当前订阅被新订阅覆盖。',
        '你还有未完成的订单，购买前需要先进行取消，确定取消先前的订单吗？':
          '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
        '佣金将会在确认后会到达你的佣金账户。': '佣金将会在确认后到达您的佣金账户。',
        '节点五分钟内节点在线情况': '五分钟内节点在线情况',
        '如果你的订阅地址或信息泄露可以进行此操作。重置后你的UUID及订阅将会变更，需要重新进行订阅。':
          '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
        重置订阅提示信息:
          '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
        向机器人发送你的: '向机器人发送您的',
      },
    });

    const i18n = createI18n();

    expect(i18n.t('order.processing')).toBe('订单系统正在进行处理，请等候 1-3 分钟。');
    expect(i18n.t('order.cancel_confirm')).toBe(
      '如果您已经付款，取消订单可能会导致支付失败，确定要取消订单吗？',
    );
    expect(i18n.t('order.credit_card_security')).toBe(
      '您的信用卡信息只会用于当次扣款，系统并不会保存，我们认为这是最安全的。',
    );
    expect(i18n.t('plan.pick_title')).toBe('选择最适合您的计划');
    expect(i18n.t('plan.select_other')).toBe('选择其它订阅');
    expect(i18n.t('plan.change_warning')).toBe('请注意，变更订阅会导致当前订阅被新订阅覆盖。');
    expect(i18n.t('plan.unfinished_order_confirm')).toBe(
      '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
    );
    expect(i18n.t('invite.pending_hint')).toBe('佣金将会在确认后到达您的佣金账户。');
    expect(i18n.t('node.status_tip')).toBe('五分钟内节点在线情况');
    expect(i18n.t('profile.telegram_bind')).toBe('绑定 Telegram');
    expect(i18n.t('profile.telegram_search')).toBe('打开 Telegram 搜索');
    expect(i18n.t('profile.telegram_send')).toBe('向机器人发送您的');
    expect(i18n.t('profile.reset_subscribe_tip')).toBe(
      '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
    );
    expect(i18n.t('profile.reset_subscribe_warning')).toBe(
      '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
    );
    expect(i18n.t('ticket.message_placeholder')).toBe('请描述您遇到的问题');
  });

  it('translates zh-CN fallback copy through the original legacy source key', () => {
    setLegacyLocale('en-US');
    setLegacyI18n({
      'zh-CN': {
        '订单系统正在进行处理，请稍等1-3分钟。': '订单系统正在进行处理，请等候 1-3 分钟。',
        '变更订阅会导致当前订阅被新订阅覆盖，请注意。':
          '请注意，变更订阅会导致当前订阅被新订阅覆盖。',
        选择其他订阅: '选择其它订阅',
        '你还有未完成的订单，购买前需要先进行取消，确定取消先前的订单吗？':
          '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
        '佣金将会在确认后会到达你的佣金账户。': '佣金将会在确认后到达您的佣金账户。',
        '如果你的订阅地址或信息泄露可以进行此操作。重置后你的UUID及订阅将会变更，需要重新进行订阅。':
          '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
        重置订阅提示信息:
          '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
        向机器人发送你的: '向机器人发送您的',
        '请描述你遇到的问题': '请描述您遇到的问题',
        '节点五分钟内节点在线情况': '五分钟内节点在线情况',
      },
      'en-US': {
        '订单系统正在进行处理，请稍等1-3分钟。':
          'Order system is being processed, please wait 1 to 3 minutes.',
        '变更订阅会导致当前订阅被新订阅覆盖，请注意。':
          'Attention please, change subscription will overwrite your current subscription.',
        选择其他订阅: 'Choose another subscription',
        '你还有未完成的订单，购买前需要先进行取消，确定取消先前的订单吗？':
          'You still have an unpaid order. You need to cancel it before purchasing. Are you sure you want to cancel the previous order?',
        '佣金将会在确认后会到达你的佣金账户。':
          'The commission will reach your commission account after review.',
        '如果你的订阅地址或信息泄露可以进行此操作。重置后你的UUID及订阅将会变更，需要重新进行订阅。':
          'In case of your account information or subscription leak, this option is for reset. After resetting your UUID and subscription will change, you need to re-subscribe.',
        重置订阅提示信息:
          'When your subscription or account is leaked and abused by unknown parties, you can reset your subscription information here to avoid unnecessary losses.',
        向机器人发送你的: 'Send the following command to bot',
        '请描述你遇到的问题': 'Please describe the problem you encountered',
        '节点五分钟内节点在线情况': 'Access Point online status in the last 5 minutes',
      },
    });

    const i18n = createI18n();

    expect(i18n.t('order.processing')).toBe(
      'Order system is being processed, please wait 1 to 3 minutes.',
    );
    expect(i18n.t('ticket.message_placeholder')).toBe(
      'Please describe the problem you encountered',
    );
    expect(i18n.t('plan.change_warning')).toBe(
      'Attention please, change subscription will overwrite your current subscription.',
    );
    expect(i18n.t('plan.select_other')).toBe('Choose another subscription');
    expect(i18n.t('plan.unfinished_order_confirm')).toBe(
      'You still have an unpaid order. You need to cancel it before purchasing. Are you sure you want to cancel the previous order?',
    );
    expect(i18n.t('invite.pending_hint')).toBe(
      'The commission will reach your commission account after review.',
    );
    expect(i18n.t('profile.reset_subscribe_tip')).toBe(
      'In case of your account information or subscription leak, this option is for reset. After resetting your UUID and subscription will change, you need to re-subscribe.',
    );
    expect(i18n.t('profile.reset_subscribe_warning')).toBe(
      'When your subscription or account is leaked and abused by unknown parties, you can reset your subscription information here to avoid unnecessary losses.',
    );
    expect(i18n.t('profile.telegram_send')).toBe('Send the following command to bot');
    expect(i18n.t('node.status_tip')).toBe(
      'Access Point online status in the last 5 minutes',
    );
  });

  it('keeps localized fallback copy aligned with the bundled legacy dictionaries', () => {
    setLegacyLocale('zh-TW');
    let i18n = createI18n();
    expect(i18n.t('nav.dashboard')).toBe('儀表板');
    expect(i18n.t('nav.profile')).toBe('您的帳戸');
    expect(i18n.t('dashboard.copy_subscribe')).toBe('複製訂閲位址');
    expect(i18n.t('order.processing')).toBe('訂單系統正在進行處理，請稍等 1-3 分鐘。');
    expect(i18n.t('node.status_tip')).toBe('五分鐘內節點線上情況');

    setLegacyLocale('en-US');
    i18n = createI18n();
    expect(i18n.t('common.operation')).toBe('Action');
    expect(i18n.t('nav.notices')).toBe('Announcements');
    expect(i18n.t('dashboard.balance')).toBe('Account Balance (For billing only)');
    expect(i18n.t('node.name')).toBe('Access Point Name');
    expect(i18n.t('plan.transfer_enable')).toBe('Product Transfer Data');
    expect(i18n.t('order.paid_at')).toBe('Complete Time');

    setLegacyLocale('ja-JP');
    i18n = createI18n();
    expect(i18n.t('common.confirm')).toBe('確定');
    expect(i18n.t('common.save')).toBe('変更を保存');
    expect(i18n.t('auth.submit_register')).toBe('新規登録');

    setLegacyLocale('fa-IR');
    i18n = createI18n();
    expect(i18n.t('common.cancel')).toBe('انصراف');
    expect(i18n.t('common.save')).toBe('ذخیره کردن');
    expect(i18n.t('auth.submit_register')).toBe('ثبت‌نام');

    setLegacyLocale('vi-VN');
    i18n = createI18n();
    expect(i18n.t('nav.dashboard')).toBe('Trang Chủ');
    expect(i18n.t('invite.pending_hint')).toBe(
      'Sau khi xác nhận tiền hoa hồng sẽ gửi đến tài khoản hoa hồng của bạn.',
    );
    expect(i18n.t('order.processing')).toBe(
      'Hệ thống đang xử lý đơn hàng, vui lòng đợi 1-3p.',
    );
    expect(i18n.t('node.status_tip')).toBe('Node trạng thái online trong vòng 5 phút');

    setLegacyLocale('ko-KR');
    i18n = createI18n();
    expect(i18n.t('nav.dashboard')).toBe('대시보드');
    expect(i18n.t('invite.pending_hint')).toBe(
      '수수료는 검토 후 수수료 계정에서 확인할 수 있습니다',
    );
    expect(i18n.t('order.processing')).toBe(
      '주문 시스템이 처리 중입니다. 1-3분 정도 기다려 주십시오.',
    );
    expect(i18n.t('node.status_tip')).toBe('지난 5분 동안의 액세스 포인트 온라인 상태');
  });

  it('keeps legacy source typos when the bundled dictionaries also miss them', () => {
    setLegacyLocale('en-US');
    setLegacyI18n({
      'zh-CN': { '已回复': '已回复' },
      'en-US': { '已回复': 'Replied' },
    });

    const i18n = createI18n();

    expect(i18n.t('ticket.replied')).toBe('已答复');
  });

  it('stamps the provider fallback into getLocale for unsupported navigators', () => {
    Object.defineProperty(window.navigator, 'language', { value: 'fr-FR', configurable: true });

    const i18n = createI18n();

    expect(i18n.language).toBe('zh-CN');
    expect(window.g_lang).toBe('zh-CN');
    expect(legacyGetLocale()).toBe('zh-CN');
  });

  it('does not use an exact supported navigator language without legacy storage', () => {
    Object.defineProperty(window.navigator, 'language', { value: 'en-US', configurable: true });

    const i18n = createI18n();

    expect(window.localStorage.getItem('umi_locale')).toBeNull();
    expect(i18n.language).toBe('zh-CN');
    expect(window.g_lang).toBe('zh-CN');
  });
});
