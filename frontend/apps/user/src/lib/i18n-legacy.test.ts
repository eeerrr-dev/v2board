import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createI18n, legacyGetLocale } from '@v2board/i18n';

const originalNavigatorLanguage = window.navigator.language;

function installLocalStorageStub() {
  const store = new Map<string, string>();
  const storage = {
    clear: () => store.clear(),
    getItem: (key: string) => store.get(key) ?? null,
    removeItem: (key: string) => {
      store.delete(key);
    },
    setItem: (key: string, value: string) => {
      store.set(key, value);
    },
  };
  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    value: storage,
  });
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: storage,
  });
}

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
    installLocalStorageStub();
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

  it('keeps the bundled legacy zh-CN display copy when no dictionary is loaded', () => {
    setLegacyLocale('zh-CN');

    const i18n = createI18n();

    expect(i18n.t('order.processing')).toBe('订单系统正在进行处理，请等候 1-3 分钟。');
    expect(i18n.t('order.cancel_confirm')).toBe(
      '如果您已经付款，取消订单可能会导致支付失败，确定要取消订单吗？',
    );
    expect(i18n.t('order.credit_card_security')).toBe(
      '您的信用卡信息只会用于当次扣款，系统并不会保存，我们认为这是最安全的。',
    );
    expect(i18n.t('traffic.notice')).toBe('流量明细仅保留近一个月数据以供查询。');
    expect(i18n.t('dashboard.used_traffic', { used: '1 GB', total: '10 GB' })).toBe(
      '已用 1 GB / 总计 10 GB',
    );
    expect(i18n.t('dashboard.devices_online', { alive_ip: 1, device_limit: 3 })).toBe(
      '在线设备 1/3',
    );
    expect(i18n.t('dashboard.reset_in_days', { reset_day: 5 })).toBe(
      '已用流量将在 5 日后重置',
    );
    expect(i18n.t('dashboard.expires_in', { date: '2026/06/04', day: 7 })).toBe(
      '于 2026/06/04 到期，距离到期还有 7 天。',
    );
    expect(i18n.t('dashboard.alert_traffic_rate', { rate: 80 })).toBe('当前已使用流量达 80%');
    expect(i18n.t('invite.transfer_notice', { title: 'V2Board' })).toBe(
      '划转后的余额仅用于V2Board消费使用',
    );
    expect(i18n.t('knowledge.last_update', { date: '2026/06/04' })).toBe(
      '最后更新: 2026/06/04',
    );
    expect(i18n.t('auth.tos_html', { url: 'https://example.test' })).toContain(
      'href="{url}"',
    );
    expect(i18n.t('node.status_tip')).toBe('节点五分钟内节点在线情况');
    expect(i18n.t('ticket.message_placeholder')).toBe('请描述您遇到的问题');
    expect(i18n.t('invite.pending_hint')).toBe('佣金将会在确认后会到达你的佣金账户。');
    expect(i18n.t('plan.pick_title')).toBe('选择最适合您的计划');
    expect(i18n.t('plan.pick_best_for_you')).toBe('选择最适合您的计划');
    expect(i18n.t('plan.select_other')).toBe('选择其它订阅');
    expect(i18n.t('plan.change_warning')).toBe('请注意，变更订阅会导致当前订阅被新订阅覆盖。');
    expect(i18n.t('plan.unfinished_order_confirm')).toBe(
      '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
    );
    expect(i18n.t('profile.telegram_bind')).toBe('绑定 Telegram');
    expect(i18n.t('profile.telegram_search')).toBe('打开 Telegram 搜索');
    expect(i18n.t('profile.telegram_send')).toBe('向机器人发送您的');
    expect(i18n.t('profile.reset_subscribe_tip')).toBe(
      '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
    );
    expect(i18n.t('profile.reset_subscribe_warning')).toBe(
      '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
    );
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
        '流量明细仅保留近月数据以供查询。': '流量明细仅保留近一个月数据以供查询。',
        '已用 {used} / 总计 {total}': '已用 {used} / 总计 {total}',
        '在线设备 {alive_ip}/{device_limit}': '在线设备 {alive_ip}/{device_limit}',
        '已用流量将在 {reset_day} 日后重置': '已用流量将在 {reset_day} 日后重置',
        '于 {date} 到期，距离到期还有 {day} 天。': '于 {date} 到期，距离到期还有 {day} 天。',
        '当前已使用流量达{rate}%': '当前已使用流量达 {rate}%',
        '划转后的余额仅用于{title}消费使用': '划转后的余额仅用于{title}消费使用',
        '最后更新: {date}': '最后更新: {date}',
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
    expect(i18n.t('plan.pick_best_for_you')).toBe('选择最适合您的计划');
    expect(i18n.t('plan.select_other')).toBe('选择其它订阅');
    expect(i18n.t('plan.change_warning')).toBe('请注意，变更订阅会导致当前订阅被新订阅覆盖。');
    expect(i18n.t('plan.unfinished_order_confirm')).toBe(
      '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
    );
    expect(i18n.t('invite.pending_hint')).toBe('佣金将会在确认后到达您的佣金账户。');
    expect(i18n.t('node.status_tip')).toBe('五分钟内节点在线情况');
    expect(i18n.t('traffic.notice')).toBe('流量明细仅保留近一个月数据以供查询。');
    expect(i18n.t('dashboard.used_traffic', { used: '1 GB', total: '10 GB' })).toBe(
      '已用 1 GB / 总计 10 GB',
    );
    expect(i18n.t('dashboard.devices_online', { alive_ip: 1, device_limit: 3 })).toBe(
      '在线设备 1/3',
    );
    expect(i18n.t('dashboard.reset_in_days', { reset_day: 5 })).toBe(
      '已用流量将在 5 日后重置',
    );
    expect(i18n.t('dashboard.expires_in', { date: '2026/06/04', day: 7 })).toBe(
      '于 2026/06/04 到期，距离到期还有 7 天。',
    );
    expect(i18n.t('dashboard.alert_traffic_rate', { rate: 80 })).toBe('当前已使用流量达 80%');
    expect(i18n.t('invite.transfer_notice', { title: 'V2Board' })).toBe(
      '划转后的余额仅用于V2Board消费使用',
    );
    expect(i18n.t('knowledge.last_update', { date: '2026/06/04' })).toBe(
      '最后更新: 2026/06/04',
    );
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
        选择最适合你的计划: '选择最适合您的计划',
        '变更订阅会导致当前订阅被新订阅覆盖，请注意。':
          '请注意，变更订阅会导致当前订阅被新订阅覆盖。',
        选择其他订阅: '选择其它订阅',
        '你还有未完成的订单，购买前需要先进行取消，确定取消先前的订单吗？':
          '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
        '佣金将会在确认后会到达你的佣金账户。': '佣金将会在确认后到达您的佣金账户。',
        '如果你的订阅地址或信息泄露可以进行此操作。重置后你的UUID及订阅将会变更，需要重新进行订阅。':
          '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
        '流量明细仅保留近月数据以供查询。': '流量明细仅保留近一个月数据以供查询。',
        重置订阅提示信息:
          '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
        向机器人发送你的: '向机器人发送您的',
        '请描述你遇到的问题': '请描述您遇到的问题',
        '节点五分钟内节点在线情况': '五分钟内节点在线情况',
        '已用 {used} / 总计 {total}': '已用 {used} / 总计 {total}',
        '在线设备 {alive_ip}/{device_limit}': '在线设备 {alive_ip}/{device_limit}',
        '已用流量将在 {reset_day} 日后重置': '已用流量将在 {reset_day} 日后重置',
        '于 {date} 到期，距离到期还有 {day} 天。': '于 {date} 到期，距离到期还有 {day} 天。',
        '当前已使用流量达{rate}%': '当前已使用流量达 {rate}%',
        '划转后的余额仅用于{title}消费使用': '划转后的余额仅用于{title}消费使用',
        '最后更新: {date}': '最后更新: {date}',
      },
      'en-US': {
        '订单系统正在进行处理，请稍等1-3分钟。':
          'Order system is being processed, please wait 1 to 3 minutes.',
        选择最适合你的计划: 'Choose the right plan for you',
        '变更订阅会导致当前订阅被新订阅覆盖，请注意。':
          'Attention please, change subscription will overwrite your current subscription.',
        选择其他订阅: 'Choose another subscription',
        '你还有未完成的订单，购买前需要先进行取消，确定取消先前的订单吗？':
          'You still have an unpaid order. You need to cancel it before purchasing. Are you sure you want to cancel the previous order?',
        '佣金将会在确认后会到达你的佣金账户。':
          'The commission will reach your commission account after review.',
        '流量明细仅保留近月数据以供查询。':
          'Only keep the most recent month\'s usage for checking the transfer data details.',
        '如果你的订阅地址或信息泄露可以进行此操作。重置后你的UUID及订阅将会变更，需要重新进行订阅。':
          'In case of your account information or subscription leak, this option is for reset. After resetting your UUID and subscription will change, you need to re-subscribe.',
        重置订阅提示信息:
          'When your subscription or account is leaked and abused by unknown parties, you can reset your subscription information here to avoid unnecessary losses.',
        向机器人发送你的: 'Send the following command to bot',
        '请描述你遇到的问题': 'Please describe the problem you encountered',
        '节点五分钟内节点在线情况': 'Access Point online status in the last 5 minutes',
        '已用 {used} / 总计 {total}': '{used} Used / Total {total}',
        '在线设备 {alive_ip}/{device_limit}': '{alive_ip} Online / {device_limit} Device(s)',
        '已用流量将在 {reset_day} 日后重置':
          'Used data will reset after {reset_day} days',
        '于 {date} 到期，距离到期还有 {day} 天。':
          'Will expire on {date}, {day} days before expiration, ',
        '当前已使用流量达{rate}%': 'Currently used data up to {rate}%',
        '划转后的余额仅用于{title}消费使用':
          'The transferred balance will be used for {title} payments only',
        '最后更新: {date}': 'Last Updated: {date}',
      },
    });

    const i18n = createI18n();

    expect(i18n.t('order.processing')).toBe(
      'Order system is being processed, please wait 1 to 3 minutes.',
    );
    expect(i18n.t('plan.pick_best_for_you')).toBe('Choose the right plan for you');
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
    expect(i18n.t('traffic.notice')).toBe(
      'Only keep the most recent month\'s usage for checking the transfer data details.',
    );
    expect(i18n.t('node.status_tip')).toBe(
      'Access Point online status in the last 5 minutes',
    );
    expect(i18n.t('dashboard.used_traffic', { used: '1 GB', total: '10 GB' })).toBe(
      '1 GB Used / Total 10 GB',
    );
    expect(i18n.t('dashboard.devices_online', { alive_ip: 1, device_limit: 3 })).toBe(
      '1 Online / 3 Device(s)',
    );
    expect(i18n.t('dashboard.reset_in_days', { reset_day: 5 })).toBe(
      'Used data will reset after 5 days',
    );
    expect(i18n.t('dashboard.expires_in', { date: '2026/06/04', day: 7 })).toBe(
      'Will expire on 2026/06/04, 7 days before expiration, ',
    );
    expect(i18n.t('dashboard.alert_traffic_rate', { rate: 80 })).toBe(
      'Currently used data up to 80%',
    );
    expect(i18n.t('invite.transfer_notice', { title: 'V2Board' })).toBe(
      'The transferred balance will be used for V2Board payments only',
    );
    expect(i18n.t('knowledge.last_update', { date: '2026/06/04' })).toBe(
      'Last Updated: 2026/06/04',
    );
  });

  it('keeps localized fallback copy aligned with the bundled legacy dictionaries', () => {
    setLegacyLocale('zh-TW');
    let i18n = createI18n();
    expect(i18n.t('nav.dashboard')).toBe('儀表板');
    expect(i18n.t('nav.profile')).toBe('您的帳戸');
    expect(i18n.t('dashboard.copy_subscribe')).toBe('複製訂閲位址');
    expect(i18n.t('order.processing')).toBe('訂單系統正在進行處理，請稍等 1-3 分鐘。');
    expect(i18n.t('plan.pick_best_for_you')).toBe('選擇最適合您的計劃');
    expect(i18n.t('nav.node')).toBe('節點狀態');
    expect(i18n.t('node.simple_name')).toBe('名稱');
    expect(i18n.t('node.status')).toBe('狀態');
    expect(i18n.t('node.rate')).toBe('倍率');
    expect(i18n.t('node.tags')).toBe('標籤');
    expect(i18n.t('node.no_available')).toBe('沒有可用節點，如果您未訂閱或已過期請');
    expect(i18n.t('node.renew')).toBe('續費');
    expect(i18n.t('node.subscribe')).toBe('訂閱');
    expect(i18n.t('node.status_tip')).toBe('五分鐘內節點線上情況');
    expect(i18n.t('node.rate_tip')).toBe('使用的流量將乘以倍率進行扣除');
    expect(i18n.t('traffic.notice')).toBe('流量明細僅保留近一個月資料以供查詢。');
    expect(i18n.t('traffic.record_at')).toBe('記錄時間');
    expect(i18n.t('traffic.actual_upload')).toBe('實際上行');
    expect(i18n.t('traffic.actual_download')).toBe('實際下行');
    expect(i18n.t('traffic.deduct_rate')).toBe('扣費倍率');
    expect(i18n.t('traffic.total_formula')).toBe(
      '公式：(實際上行 + 實際下行) x 扣費倍率 = 扣除流量',
    );
    expect(i18n.t('dashboard.used_traffic', { used: '1 GB', total: '10 GB' })).toBe(
      '已用 1 GB / 總計 10 GB',
    );
    expect(i18n.t('dashboard.alert_traffic_rate', { rate: 80 })).toBe('當前已用流量達 80%');
    expect(i18n.t('ticket.new')).toBe('新的工單');
    expect(i18n.t('ticket.subject')).toBe('主題');
    expect(i18n.t('ticket.level')).toBe('工單級別');
    expect(i18n.t('ticket.level_form')).toBe('工單等級');
    expect(i18n.t('ticket.status')).toBe('工單狀態');
    expect(i18n.t('ticket.closed')).toBe('已關閉');
    expect(i18n.t('ticket.last_reply_col')).toBe('最新回復');
    expect(i18n.t('ticket.message_placeholder')).toBe('請描述您遇到的問題');
    expect(i18n.t('ticket.reply_placeholder')).toBe('輸入内容回復工單…');
    expect(i18n.t('ticket.subject_placeholder')).toBe('請輸入工單主題');
    expect(i18n.t('ticket.level_placeholder')).toBe('請選擇工單等級');
    expect(i18n.t('ticket.history')).toBe('工單歷史');
    expect(i18n.t('ticket.view')).toBe('檢視');
    expect(i18n.t('dashboard.transfer_to_balance')).toBe('推廣佣金劃轉至餘額');
    expect(i18n.t('invite.manage')).toBe('邀請碼管理');
    expect(i18n.t('invite.generate')).toBe('生成邀請碼');
    expect(i18n.t('invite.withdraw')).toBe('申請提現');
    expect(i18n.t('invite.withdraw_account_placeholder')).toBe('請輸入提現賬號');
    expect(i18n.t('invite.current_commission_balance')).toBe('當前推廣佣金餘額');
    expect(i18n.t('invite.transfer_placeholder')).toBe('請輸入需要劃轉到餘額的金額');
    expect(i18n.t('invite.withdraw_button')).toBe('推廣佣金提現');
    expect(i18n.t('invite.transfer_notice', { title: 'V2Board' })).toBe(
      '劃轉后的餘額僅用於 V2Board 消費使用',
    );
    expect(i18n.t('knowledge.last_update', { date: '2026/06/04' })).toBe(
      '最後更新: 2026/06/04',
    );

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
    expect(i18n.t('plan.pick_best_for_you')).toBe('Chọn kế hoạch phù hợp với bạn nhất');
    expect(i18n.t('nav.node')).toBe('Trạng thái node');
    expect(i18n.t('node.simple_name')).toBe('Tên');
    expect(i18n.t('node.status')).toBe('Trạng thái');
    expect(i18n.t('node.rate')).toBe('Bội số');
    expect(i18n.t('node.tags')).toBe('Nhãn');
    expect(i18n.t('node.no_available')).toBe(
      'Chưa có node khả dụng, nếu bạn chưa mua gói hoặc đã hết hạn hãy',
    );
    expect(i18n.t('node.renew')).toBe('Gia hạn');
    expect(i18n.t('node.subscribe')).toBe('Gói Dịch Vụ');
    expect(i18n.t('node.status_tip')).toBe('Node trạng thái online trong vòng 5 phút');
    expect(i18n.t('node.rate_tip')).toBe(
      'Dung lượng sử dụng nhân với bội số rồi khấu trừ',
    );
    expect(i18n.t('traffic.notice')).toBe(
      'Chi tiết dung lượng chỉ lưu dữ liệu của những tháng gần đây để truy vấn.',
    );
    expect(i18n.t('traffic.record_at')).toBe('Thời gian ghi');
    expect(i18n.t('traffic.actual_upload')).toBe('Upload thực tế');
    expect(i18n.t('traffic.actual_download')).toBe('Download thực tế');
    expect(i18n.t('traffic.deduct_rate')).toBe('Tỷ lệ khấu trừ');
    expect(i18n.t('traffic.total_formula')).toBe(
      'Công thức: (upload thực tế + download thực tế) x bội số trừ phí = Dung lượng khấu trừ',
    );
    expect(i18n.t('dashboard.used_traffic', { used: '1 GB', total: '10 GB' })).toBe(
      'Đã sử dụng 1 GB / Tổng dung lượng 10 GB',
    );
    expect(i18n.t('dashboard.alert_traffic_rate', { rate: 80 })).toBe(
      'Dữ liệu hiện đang sử dụng lên đến 80%',
    );
    expect(i18n.t('ticket.new')).toBe('Việc mới');
    expect(i18n.t('ticket.subject')).toBe('Chủ Đề');
    expect(i18n.t('ticket.level')).toBe('Cấp độ');
    expect(i18n.t('ticket.level_form')).toBe('Cấp độ công việc');
    expect(i18n.t('ticket.status')).toBe('Trạng thái');
    expect(i18n.t('ticket.closed')).toBe('Đã đóng');
    expect(i18n.t('ticket.last_reply_col')).toBe('Trả lời gần đây');
    expect(i18n.t('ticket.message_placeholder')).toBe('Hãy mô tả vấn đề gặp phải');
    expect(i18n.t('ticket.reply_placeholder')).toBe('Nhập nội dung trả lời công việc...');
    expect(i18n.t('ticket.subject_placeholder')).toBe('Hãy nhập chủ đề công việc');
    expect(i18n.t('ticket.level_placeholder')).toBe('Hãy chọn cấp độ công việc');
    expect(i18n.t('ticket.history')).toBe('Lịch sử đơn hàng');
    expect(i18n.t('ticket.view')).toBe('Xem');
    expect(i18n.t('dashboard.transfer_to_balance')).toBe(
      'Chuyển khoản hoa hồng giới thiệu đến số dư',
    );
    expect(i18n.t('invite.manage')).toBe('Quản lý mã mời');
    expect(i18n.t('invite.generate')).toBe('Tạo mã mời');
    expect(i18n.t('invite.withdraw')).toBe('Yêu cầu rút tiền');
    expect(i18n.t('invite.withdraw_account_placeholder')).toBe('Hãy chọn tài khoản rút tiền');
    expect(i18n.t('invite.current_commission_balance')).toBe(
      'Số dư hoa hồng giới thiệu hiện tại',
    );
    expect(i18n.t('invite.transfer_placeholder')).toBe(
      'Hãy nhậo số tiền muốn chuyển đến số dư',
    );
    expect(i18n.t('invite.withdraw_button')).toBe('Rút tiền hoa hồng giới thiệu');
    expect(i18n.t('invite.transfer_notice', { title: 'V2Board' })).toBe(
      'Số dư sau khi chuyển khoản chỉ dùng để tiêu dùng V2Board',
    );
    expect(i18n.t('knowledge.last_update', { date: '2026/06/04' })).toBe(
      'Cập nhật gần đây: 2026/06/04',
    );

    setLegacyLocale('ko-KR');
    i18n = createI18n();
    expect(i18n.t('nav.dashboard')).toBe('대시보드');
    expect(i18n.t('invite.pending_hint')).toBe(
      '수수료는 검토 후 수수료 계정에서 확인할 수 있습니다',
    );
    expect(i18n.t('order.processing')).toBe(
      '주문 시스템이 처리 중입니다. 1-3분 정도 기다려 주십시오.',
    );
    expect(i18n.t('plan.pick_best_for_you')).toBe('당신에게 맞는 플랜을 선택하세요');
    expect(i18n.t('node.status_tip')).toBe('지난 5분 동안의 액세스 포인트 온라인 상태');
    expect(i18n.t('traffic.notice')).toBe('귀하의 트래픽 세부 정보는 최근 몇 달 동안만 유지됩니다');
    expect(i18n.t('dashboard.alert_traffic_rate', { rate: 80 })).toBe('当前已使用流量达80%');
    expect(i18n.t('invite.transfer_notice', { title: 'V2Board' })).toBe(
      '이체된 잔액은 V2Board 결제에만 사용됩니다.',
    );
    expect(i18n.t('knowledge.last_update', { date: '2026/06/04' })).toBe(
      '마지막 업데이트2026/06/04',
    );
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

  it('normalizes old underscore i18n cookies before bootstrapping the provider', () => {
    Object.defineProperty(window.navigator, 'language', { value: 'fr-FR', configurable: true });
    document.cookie = 'i18n=zh_CN;path=/';
    const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => undefined);

    const i18n = createI18n();

    expect(i18n.language).toBe('zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBe('zh-CN');
    expect(window.g_lang).toBe('zh-CN');
    expect(reload).not.toHaveBeenCalled();
  });

  it('ignores invalid i18n cookies instead of throwing before the app mounts', () => {
    Object.defineProperty(window.navigator, 'language', { value: 'fr-FR', configurable: true });
    document.cookie = 'i18n=bad-locale;path=/';

    const i18n = createI18n();

    expect(i18n.language).toBe('zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBeNull();
    expect(window.g_lang).toBe('zh-CN');
  });

  it('ignores malformed i18n cookie encoding instead of throwing before the app mounts', () => {
    Object.defineProperty(window.navigator, 'language', { value: 'fr-FR', configurable: true });
    document.cookie = 'i18n=%E0%A4%A;path=/';

    const i18n = createI18n();

    expect(i18n.language).toBe('zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBeNull();
    expect(window.g_lang).toBe('zh-CN');
  });

  it('bootstraps the provider from the exact supported navigator language like the bundled app', () => {
    Object.defineProperty(window.navigator, 'language', { value: 'en-US@posix', configurable: true });

    const i18n = createI18n();

    expect(window.localStorage.getItem('umi_locale')).toBe('en-US');
    expect(i18n.language).toBe('en-US');
    expect(window.g_lang).toBe('en-US');
  });
});
