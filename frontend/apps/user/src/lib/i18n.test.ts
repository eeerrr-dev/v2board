import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getLocale, installLocaleDocumentEnvironment } from '@v2board/i18n';
import { createI18n } from '@v2board/i18n/testing';

const originalNavigatorLanguage = window.navigator.language;
const originalNavigatorLanguages = window.navigator.languages;

function setNavigatorLanguages(...languages: string[]) {
  Object.defineProperties(window.navigator, {
    language: { value: languages[0] ?? '', configurable: true },
    languages: { value: languages, configurable: true },
  });
}

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

function setLocalePreference(locale: string) {
  document.cookie = `i18n=${locale};path=/`;
  window.localStorage.setItem('umi_locale', locale);
}

describe('i18n resources', () => {
  beforeEach(() => {
    installLocalStorageStub();
    document.cookie = 'i18n=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    window.localStorage.clear();
    window.g_lang = undefined;
    window.g_langSeparator = undefined;
  });

  afterEach(() => {
    Object.defineProperty(window.navigator, 'language', {
      value: originalNavigatorLanguage,
      configurable: true,
    });
    Object.defineProperty(window.navigator, 'languages', {
      value: originalNavigatorLanguages,
      configurable: true,
    });
    document.documentElement.removeAttribute('lang');
    document.documentElement.removeAttribute('dir');
    delete document.documentElement.dataset.locale;
    delete document.documentElement.dataset.textDirection;
  });

  it('centralizes locale document environment for layout adaptation', async () => {
    setLocalePreference('ja-JP');
    const i18n = createI18n();
    const cleanup = installLocaleDocumentEnvironment(i18n);

    expect(document.documentElement.lang).toBe('ja-JP');
    expect(document.documentElement.dir).toBe('ltr');
    expect(document.documentElement.dataset.locale).toBe('ja-JP');
    expect(document.documentElement.dataset.textDirection).toBe('ltr');

    await i18n.changeLanguage('en-US');

    expect(document.documentElement.lang).toBe('en-US');
    expect(document.documentElement.dir).toBe('ltr');
    expect(document.documentElement.dataset.locale).toBe('en-US');
    expect(document.documentElement.dataset.textDirection).toBe('ltr');

    cleanup();
  });

  it('keeps the established zh-CN display copy in the static resource', () => {
    setLocalePreference('zh-CN');

    const i18n = createI18n();

    expect(i18n.t(($) => $.order.processing)).toBe('订单系统正在进行处理，请等候 1-3 分钟。');
    expect(i18n.t(($) => $.order.cancel_confirm)).toBe(
      '如果您已经付款，取消订单可能会导致支付失败，确定要取消订单吗？',
    );
    expect(i18n.t(($) => $.order.credit_card_security)).toBe(
      '您的信用卡信息只会用于当次扣款，系统并不会保存，我们认为这是最安全的。',
    );
    expect(i18n.t(($) => $.traffic.notice)).toBe('流量明细仅保留近一个月数据以供查询。');
    expect(i18n.t(($) => $.dashboard.used_traffic, { used: '1 GB', total: '10 GB' })).toBe(
      '已用 1 GB / 总计 10 GB',
    );
    expect(i18n.t(($) => $.dashboard.devices_online, { alive_ip: 1, device_limit: 3 })).toBe(
      '在线设备 1/3',
    );
    expect(i18n.t(($) => $.dashboard.reset_in_days, { reset_day: 5 })).toBe(
      '已用流量将在 5 日后重置',
    );
    expect(i18n.t(($) => $.dashboard.expires_in, { date: '2026/06/04', day: 7 })).toBe(
      '于 2026/06/04 到期，距离到期还有 7 天。',
    );
    expect(i18n.t(($) => $.dashboard.alert_traffic_rate, { rate: 80 })).toBe(
      '当前已使用流量达 80%',
    );
    expect(i18n.t(($) => $.invite.transfer_notice, { title: 'V2Board' })).toBe(
      '划转后的余额仅用于V2Board消费使用',
    );
    expect(i18n.t(($) => $.knowledge.last_update, { date: '2026/06/04' })).toBe(
      '最后更新: 2026/06/04',
    );
    expect(i18n.t(($) => $.auth.tos_html)).toContain('<terms>');
    expect(i18n.t(($) => $.node.status_tip)).toBe('五分钟内节点在线情况');
    expect(i18n.t(($) => $.ticket.message_placeholder)).toBe('请描述您遇到的问题');
    expect(i18n.t(($) => $.invite.pending_hint)).toBe('佣金将会在确认后到达您的佣金账户。');
    expect(i18n.t(($) => $.plan.pick_title)).toBe('选择最适合您的计划');
    expect(i18n.t(($) => $.plan.pick_best_for_you)).toBe('选择最适合您的计划');
    expect(i18n.t(($) => $.plan.select_other)).toBe('选择其它订阅');
    expect(i18n.t(($) => $.plan.change_warning)).toBe(
      '请注意，变更订阅会导致当前订阅被新订阅覆盖。',
    );
    expect(i18n.t(($) => $.plan.unfinished_order_confirm)).toBe(
      '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
    );
    expect(i18n.t(($) => $.profile.telegram_bind)).toBe('绑定 Telegram');
    expect(i18n.t(($) => $.profile.telegram_search)).toBe('打开 Telegram 搜索');
    expect(i18n.t(($) => $.profile.telegram_send)).toBe('向机器人发送您的');
    expect(i18n.t(($) => $.profile.reset_subscribe_tip)).toBe(
      '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
    );
    expect(i18n.t(($) => $.profile.reset_subscribe_warning)).toBe(
      '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
    );
  });

  it('localizes canonical common dialog and empty-state messages for every locale', () => {
    const expected = {
      'zh-CN': { cancel: '取消', confirm: '确定', empty: '暂无数据' },
      'zh-TW': { cancel: '取消', confirm: '確定', empty: '無此資料' },
      'en-US': { cancel: 'Cancel', confirm: 'Confirm', empty: 'No Data' },
      'ja-JP': { cancel: 'キャンセル', confirm: '確定', empty: 'データがありません' },
      'vi-VN': { cancel: 'Hủy', confirm: 'OK', empty: 'Trống' },
      'ko-KR': { cancel: '취소', confirm: '확인', empty: '데이터 없음' },
    } as const;

    for (const [locale, messages] of Object.entries(expected)) {
      setLocalePreference(locale);
      const i18n = createI18n();

      expect({
        cancel: i18n.t(($) => $.common.cancel),
        confirm: i18n.t(($) => $.common.confirm),
        empty: i18n.t(($) => $.common.empty),
      }).toEqual(messages);
    }
  });

  it('renders the statically owned zh-CN resource', () => {
    setLocalePreference('zh-CN');

    const i18n = createI18n();

    expect(i18n.t(($) => $.order.processing)).toBe('订单系统正在进行处理，请等候 1-3 分钟。');
    expect(i18n.t(($) => $.order.cancel_confirm)).toBe(
      '如果您已经付款，取消订单可能会导致支付失败，确定要取消订单吗？',
    );
    expect(i18n.t(($) => $.order.credit_card_security)).toBe(
      '您的信用卡信息只会用于当次扣款，系统并不会保存，我们认为这是最安全的。',
    );
    expect(i18n.t(($) => $.plan.pick_title)).toBe('选择最适合您的计划');
    expect(i18n.t(($) => $.plan.pick_best_for_you)).toBe('选择最适合您的计划');
    expect(i18n.t(($) => $.plan.select_other)).toBe('选择其它订阅');
    expect(i18n.t(($) => $.plan.change_warning)).toBe(
      '请注意，变更订阅会导致当前订阅被新订阅覆盖。',
    );
    expect(i18n.t(($) => $.plan.unfinished_order_confirm)).toBe(
      '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
    );
    expect(i18n.t(($) => $.invite.pending_hint)).toBe('佣金将会在确认后到达您的佣金账户。');
    expect(i18n.t(($) => $.node.status_tip)).toBe('五分钟内节点在线情况');
    expect(i18n.t(($) => $.traffic.notice)).toBe('流量明细仅保留近一个月数据以供查询。');
    expect(i18n.t(($) => $.dashboard.used_traffic, { used: '1 GB', total: '10 GB' })).toBe(
      '已用 1 GB / 总计 10 GB',
    );
    expect(i18n.t(($) => $.dashboard.devices_online, { alive_ip: 1, device_limit: 3 })).toBe(
      '在线设备 1/3',
    );
    expect(i18n.t(($) => $.dashboard.reset_in_days, { reset_day: 5 })).toBe(
      '已用流量将在 5 日后重置',
    );
    expect(i18n.t(($) => $.dashboard.expires_in, { date: '2026/06/04', day: 7 })).toBe(
      '于 2026/06/04 到期，距离到期还有 7 天。',
    );
    expect(i18n.t(($) => $.dashboard.alert_traffic_rate, { rate: 80 })).toBe(
      '当前已使用流量达 80%',
    );
    expect(i18n.t(($) => $.invite.transfer_notice, { title: 'V2Board' })).toBe(
      '划转后的余额仅用于V2Board消费使用',
    );
    expect(i18n.t(($) => $.knowledge.last_update, { date: '2026/06/04' })).toBe(
      '最后更新: 2026/06/04',
    );
    expect(i18n.t(($) => $.profile.telegram_bind)).toBe('绑定 Telegram');
    expect(i18n.t(($) => $.profile.telegram_search)).toBe('打开 Telegram 搜索');
    expect(i18n.t(($) => $.profile.telegram_send)).toBe('向机器人发送您的');
    expect(i18n.t(($) => $.profile.reset_subscribe_tip)).toBe(
      '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
    );
    expect(i18n.t(($) => $.profile.reset_subscribe_warning)).toBe(
      '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
    );
    expect(i18n.t(($) => $.ticket.message_placeholder)).toBe('请描述您遇到的问题');
  });

  it('renders the complete statically owned en-US resource', () => {
    setLocalePreference('en-US');

    const i18n = createI18n();

    expect(i18n.t(($) => $.order.processing)).toBe(
      'Order system is being processed, please wait 1 to 3 minutes.',
    );
    expect(i18n.t(($) => $.plan.pick_best_for_you)).toBe('Choose the right plan for you');
    expect(i18n.t(($) => $.ticket.message_placeholder)).toBe(
      'Please describe the problem you encountered',
    );
    expect(i18n.t(($) => $.auth.confirm_password)).toBe('Confirm password');
    expect(i18n.t(($) => $.plan.change_warning)).toBe(
      'Attention please, change subscription will overwrite your current subscription.',
    );
    expect(i18n.t(($) => $.plan.select_other)).toBe('Choose another subscription');
    expect(i18n.t(($) => $.plan.unfinished_order_confirm)).toBe(
      'You still have an unpaid order. You need to cancel it before purchasing. Are you sure you want to cancel the previous order?',
    );
    expect(i18n.t(($) => $.invite.pending_hint)).toBe(
      'The commission will reach your commission account after review.',
    );
    expect(i18n.t(($) => $.profile.reset_subscribe_tip)).toBe(
      'In case of your account information or subscription leak, this option is for reset. After resetting your UUID and subscription will change, you need to re-subscribe.',
    );
    expect(i18n.t(($) => $.profile.reset_subscribe_warning)).toBe(
      'When your subscription or account is leaked and abused by unknown parties, you can reset your subscription information here to avoid unnecessary losses.',
    );
    expect(i18n.t(($) => $.profile.telegram_send)).toBe('Send the following command to bot');
    expect(i18n.t(($) => $.traffic.notice)).toBe(
      "Only keep the most recent month's usage for checking the transfer data details.",
    );
    expect(i18n.t(($) => $.node.status_tip)).toBe(
      'Access Point online status in the last 5 minutes',
    );
    expect(i18n.t(($) => $.dashboard.used_traffic, { used: '1 GB', total: '10 GB' })).toBe(
      '1 GB Used / Total 10 GB',
    );
    expect(i18n.t(($) => $.dashboard.devices_online, { alive_ip: 1, device_limit: 3 })).toBe(
      '1 Online / 3 Device(s)',
    );
    expect(i18n.t(($) => $.dashboard.reset_in_days, { reset_day: 5 })).toBe(
      'Used data will reset after 5 days',
    );
    expect(i18n.t(($) => $.dashboard.expires_in, { date: '2026/06/04', day: 7 })).toBe(
      'Will expire on 2026/06/04, 7 days before expiration, ',
    );
    expect(i18n.t(($) => $.dashboard.alert_traffic_rate, { rate: 80 })).toBe(
      'Currently used data up to 80%',
    );
    expect(i18n.t(($) => $.invite.transfer_notice, { title: 'V2Board' })).toBe(
      'The transferred balance will be used for V2Board payments only',
    );
    expect(i18n.t(($) => $.knowledge.last_update, { date: '2026/06/04' })).toBe(
      'Last Updated: 2026/06/04',
    );
  });

  it('registers complete localized resources for every supported locale', () => {
    setLocalePreference('zh-TW');
    let i18n = createI18n();
    expect(i18n.t(($) => $.nav.dashboard)).toBe('儀表板');
    expect(i18n.t(($) => $.nav.profile)).toBe('您的帳戸');
    expect(i18n.t(($) => $.dashboard.copy_subscribe)).toBe('複製訂閲位址');
    expect(i18n.t(($) => $.order.processing)).toBe('訂單系統正在進行處理，請稍等 1-3 分鐘。');
    expect(i18n.t(($) => $.plan.pick_best_for_you)).toBe('選擇最適合您的計劃');
    expect(i18n.t(($) => $.nav.node)).toBe('節點狀態');
    expect(i18n.t(($) => $.node.simple_name)).toBe('名稱');
    expect(i18n.t(($) => $.node.status)).toBe('狀態');
    expect(i18n.t(($) => $.node.rate)).toBe('倍率');
    expect(i18n.t(($) => $.node.tags)).toBe('標籤');
    expect(i18n.t(($) => $.node.no_available)).toBe('沒有可用節點，如果您未訂閱或已過期請');
    expect(i18n.t(($) => $.node.renew)).toBe('續費');
    expect(i18n.t(($) => $.node.subscribe)).toBe('訂閱');
    expect(i18n.t(($) => $.node.status_tip)).toBe('五分鐘內節點線上情況');
    expect(i18n.t(($) => $.node.rate_tip)).toBe('使用的流量將乘以倍率進行扣除');
    expect(i18n.t(($) => $.traffic.notice)).toBe('流量明細僅保留近一個月資料以供查詢。');
    expect(i18n.t(($) => $.traffic.record_at)).toBe('記錄時間');
    expect(i18n.t(($) => $.traffic.actual_upload)).toBe('實際上行');
    expect(i18n.t(($) => $.traffic.actual_download)).toBe('實際下行');
    expect(i18n.t(($) => $.traffic.deduct_rate)).toBe('扣費倍率');
    expect(i18n.t(($) => $.traffic.total_formula)).toBe(
      '公式：(實際上行 + 實際下行) x 扣費倍率 = 扣除流量',
    );
    expect(i18n.t(($) => $.dashboard.used_traffic, { used: '1 GB', total: '10 GB' })).toBe(
      '已用 1 GB / 總計 10 GB',
    );
    expect(i18n.t(($) => $.dashboard.alert_traffic_rate, { rate: 80 })).toBe('當前已用流量達 80%');
    expect(i18n.t(($) => $.ticket.new)).toBe('新的工單');
    expect(i18n.t(($) => $.ticket.subject)).toBe('主題');
    expect(i18n.t(($) => $.ticket.level)).toBe('工單級別');
    expect(i18n.t(($) => $.ticket.level_form)).toBe('工單等級');
    expect(i18n.t(($) => $.ticket.status)).toBe('工單狀態');
    expect(i18n.t(($) => $.ticket.closed)).toBe('已關閉');
    expect(i18n.t(($) => $.ticket.last_reply_col)).toBe('最新回復');
    expect(i18n.t(($) => $.ticket.message_placeholder)).toBe('請描述您遇到的問題');
    expect(i18n.t(($) => $.ticket.reply_placeholder)).toBe('輸入内容回復工單…');
    expect(i18n.t(($) => $.ticket.subject_placeholder)).toBe('請輸入工單主題');
    expect(i18n.t(($) => $.ticket.level_placeholder)).toBe('請選擇工單等級');
    expect(i18n.t(($) => $.ticket.history)).toBe('工單歷史');
    expect(i18n.t(($) => $.ticket.view)).toBe('檢視');
    expect(i18n.t(($) => $.dashboard.transfer_to_balance)).toBe('推廣佣金劃轉至餘額');
    expect(i18n.t(($) => $.invite.manage)).toBe('邀請碼管理');
    expect(i18n.t(($) => $.invite.generate)).toBe('生成邀請碼');
    expect(i18n.t(($) => $.invite.withdraw)).toBe('申請提現');
    expect(i18n.t(($) => $.invite.withdraw_account_placeholder)).toBe('請輸入提現賬號');
    expect(i18n.t(($) => $.invite.current_commission_balance)).toBe('當前推廣佣金餘額');
    expect(i18n.t(($) => $.invite.transfer_placeholder)).toBe('請輸入需要劃轉到餘額的金額');
    expect(i18n.t(($) => $.invite.withdraw_button)).toBe('推廣佣金提現');
    expect(i18n.t(($) => $.invite.transfer_notice, { title: 'V2Board' })).toBe(
      '劃轉后的餘額僅用於 V2Board 消費使用',
    );
    expect(i18n.t(($) => $.knowledge.last_update, { date: '2026/06/04' })).toBe(
      '最後更新: 2026/06/04',
    );

    setLocalePreference('en-US');
    i18n = createI18n();
    expect(i18n.t(($) => $.common.operation)).toBe('Action');
    expect(i18n.t(($) => $.nav.notices)).toBe('Announcements');
    expect(i18n.t(($) => $.dashboard.balance)).toBe('Account Balance (For billing only)');
    expect(i18n.t(($) => $.node.name)).toBe('Access Point Name');
    expect(i18n.t(($) => $.plan.transfer_enable)).toBe('Product Transfer Data');
    expect(i18n.t(($) => $.order.paid_at)).toBe('Complete Time');

    setLocalePreference('ja-JP');
    i18n = createI18n();
    expect(i18n.t(($) => $.nav.dashboard)).toBe('ダッシュボード');
    expect(i18n.t(($) => $.nav.knowledge)).toBe('ナレッジベース');
    expect(i18n.t(($) => $.dashboard.plan)).toBe('マイプラン');
    expect(i18n.t(($) => $.dashboard.used_traffic, { used: '1 GB', total: '10 GB' })).toBe(
      '使用済み 1 GB / 合計 10 GB',
    );
    expect(i18n.t(($) => $.common.confirm)).toBe('確定');
    expect(i18n.t(($) => $.common.save)).toBe('変更を保存');
    expect(i18n.t(($) => $.auth.submit_register)).toBe('新規登録');

    setLocalePreference('vi-VN');
    i18n = createI18n();
    expect(i18n.t(($) => $.nav.dashboard)).toBe('Trang Chủ');
    expect(i18n.t(($) => $.invite.pending_hint)).toBe(
      'Sau khi xác nhận tiền hoa hồng sẽ gửi đến tài khoản hoa hồng của bạn.',
    );
    expect(i18n.t(($) => $.order.processing)).toBe(
      'Hệ thống đang xử lý đơn hàng, vui lòng đợi 1-3p.',
    );
    expect(i18n.t(($) => $.plan.pick_best_for_you)).toBe('Chọn kế hoạch phù hợp với bạn nhất');
    expect(i18n.t(($) => $.nav.node)).toBe('Trạng thái node');
    expect(i18n.t(($) => $.node.simple_name)).toBe('Tên');
    expect(i18n.t(($) => $.node.status)).toBe('Trạng thái');
    expect(i18n.t(($) => $.node.rate)).toBe('Bội số');
    expect(i18n.t(($) => $.node.tags)).toBe('Nhãn');
    expect(i18n.t(($) => $.node.no_available)).toBe(
      'Chưa có node khả dụng, nếu bạn chưa mua gói hoặc đã hết hạn hãy',
    );
    expect(i18n.t(($) => $.node.renew)).toBe('Gia hạn');
    expect(i18n.t(($) => $.node.subscribe)).toBe('Gói Dịch Vụ');
    expect(i18n.t(($) => $.node.status_tip)).toBe('Node trạng thái online trong vòng 5 phút');
    expect(i18n.t(($) => $.node.rate_tip)).toBe('Dung lượng sử dụng nhân với bội số rồi khấu trừ');
    expect(i18n.t(($) => $.traffic.notice)).toBe(
      'Chi tiết dung lượng chỉ lưu dữ liệu của những tháng gần đây để truy vấn.',
    );
    expect(i18n.t(($) => $.traffic.record_at)).toBe('Thời gian ghi');
    expect(i18n.t(($) => $.traffic.actual_upload)).toBe('Upload thực tế');
    expect(i18n.t(($) => $.traffic.actual_download)).toBe('Download thực tế');
    expect(i18n.t(($) => $.traffic.deduct_rate)).toBe('Tỷ lệ khấu trừ');
    expect(i18n.t(($) => $.traffic.total_formula)).toBe(
      'Công thức: (upload thực tế + download thực tế) x bội số trừ phí = Dung lượng khấu trừ',
    );
    expect(i18n.t(($) => $.dashboard.used_traffic, { used: '1 GB', total: '10 GB' })).toBe(
      'Đã sử dụng 1 GB / Tổng dung lượng 10 GB',
    );
    expect(i18n.t(($) => $.dashboard.alert_traffic_rate, { rate: 80 })).toBe(
      'Dữ liệu hiện đang sử dụng lên đến 80%',
    );
    expect(i18n.t(($) => $.ticket.new)).toBe('Việc mới');
    expect(i18n.t(($) => $.ticket.subject)).toBe('Chủ Đề');
    expect(i18n.t(($) => $.ticket.level)).toBe('Cấp độ');
    expect(i18n.t(($) => $.ticket.level_form)).toBe('Cấp độ công việc');
    expect(i18n.t(($) => $.ticket.status)).toBe('Trạng thái');
    expect(i18n.t(($) => $.ticket.closed)).toBe('Đã đóng');
    expect(i18n.t(($) => $.ticket.last_reply_col)).toBe('Trả lời gần đây');
    expect(i18n.t(($) => $.ticket.message_placeholder)).toBe('Hãy mô tả vấn đề gặp phải');
    expect(i18n.t(($) => $.ticket.reply_placeholder)).toBe('Nhập nội dung trả lời công việc...');
    expect(i18n.t(($) => $.ticket.subject_placeholder)).toBe('Hãy nhập chủ đề công việc');
    expect(i18n.t(($) => $.ticket.level_placeholder)).toBe('Hãy chọn cấp độ công việc');
    expect(i18n.t(($) => $.ticket.history)).toBe('Lịch sử đơn hàng');
    expect(i18n.t(($) => $.ticket.view)).toBe('Xem');
    expect(i18n.t(($) => $.dashboard.transfer_to_balance)).toBe(
      'Chuyển khoản hoa hồng giới thiệu đến số dư',
    );
    expect(i18n.t(($) => $.invite.manage)).toBe('Quản lý mã mời');
    expect(i18n.t(($) => $.invite.generate)).toBe('Tạo mã mời');
    expect(i18n.t(($) => $.invite.withdraw)).toBe('Yêu cầu rút tiền');
    expect(i18n.t(($) => $.invite.withdraw_account_placeholder)).toBe(
      'Hãy chọn tài khoản rút tiền',
    );
    expect(i18n.t(($) => $.invite.current_commission_balance)).toBe(
      'Số dư hoa hồng giới thiệu hiện tại',
    );
    expect(i18n.t(($) => $.invite.transfer_placeholder)).toBe(
      'Hãy nhậo số tiền muốn chuyển đến số dư',
    );
    expect(i18n.t(($) => $.invite.withdraw_button)).toBe('Rút tiền hoa hồng giới thiệu');
    expect(i18n.t(($) => $.invite.transfer_notice, { title: 'V2Board' })).toBe(
      'Số dư sau khi chuyển khoản chỉ dùng để tiêu dùng V2Board',
    );
    expect(i18n.t(($) => $.knowledge.last_update, { date: '2026/06/04' })).toBe(
      'Cập nhật gần đây: 2026/06/04',
    );

    setLocalePreference('ko-KR');
    i18n = createI18n();
    expect(i18n.t(($) => $.nav.dashboard)).toBe('대시보드');
    expect(i18n.t(($) => $.invite.pending_hint)).toBe(
      '수수료는 검토 후 수수료 계정에서 확인할 수 있습니다',
    );
    expect(i18n.t(($) => $.order.processing)).toBe(
      '주문 시스템이 처리 중입니다. 1-3분 정도 기다려 주십시오.',
    );
    expect(i18n.t(($) => $.plan.pick_best_for_you)).toBe('당신에게 맞는 플랜을 선택하세요');
    expect(i18n.t(($) => $.node.status_tip)).toBe('지난 5분 동안의 액세스 포인트 온라인 상태');
    expect(i18n.t(($) => $.traffic.notice)).toBe(
      '귀하의 트래픽 세부 정보는 최근 몇 달 동안만 유지됩니다',
    );
    expect(i18n.t(($) => $.dashboard.alert_traffic_rate, { rate: 80 })).toBe('当前已使用流量达80%');
    expect(i18n.t(($) => $.invite.transfer_notice, { title: 'V2Board' })).toBe(
      '이체된 잔액은 V2Board 결제에만 사용됩니다.',
    );
    expect(i18n.t(($) => $.knowledge.last_update, { date: '2026/06/04' })).toBe(
      '마지막 업데이트2026/06/04',
    );
  });

  it('uses the same complete resources for backend error copy', () => {
    setLocalePreference('en-US');
    let i18n = createI18n();
    expect(i18n.t(($) => $.errors['请求失败'])).toBe('Request failed');

    setLocalePreference('ja-JP');
    i18n = createI18n();
    expect(i18n.t(($) => $.errors['请求失败'])).toBe('Request failed');
  });

  it('includes corrected source copy directly in the locale resource', () => {
    setLocalePreference('en-US');

    const i18n = createI18n();

    expect(i18n.t(($) => $.ticket.replied)).toBe('Replied');
  });

  it('keeps unknown placeholders from translations literal', () => {
    setLocalePreference('ko-KR');

    const i18n = createI18n();

    expect(i18n.t(($) => $.dashboard.used_traffic, { used: '2/5', total: '5' })).toBe(
      '{date}에 만료됩니다, 만료 {day}이 전',
    );
    expect(i18n.t(($) => $.dashboard.devices_online, { alive_ip: 1, device_limit: 3 })).toBe(
      '온라인 1/3 장치',
    );
  });

  it('stamps the provider fallback into getLocale for unsupported navigators', () => {
    setNavigatorLanguages('fr-FR');

    const i18n = createI18n();

    expect(i18n.language).toBe('zh-CN');
    expect(window.g_lang).toBe('zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBe('zh-CN');
    expect(getLocale()).toBe('zh-CN');
  });

  it('repairs an unsupported persisted locale so UI and API language cannot diverge', () => {
    setNavigatorLanguages('fr-FR');
    window.localStorage.setItem('umi_locale', 'fr-FR');
    window.g_lang = 'fr-FR';

    const i18n = createI18n();

    expect(i18n.language).toBe('zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBe('zh-CN');
    expect(window.g_lang).toBe('zh-CN');
    expect(getLocale()).toBe('zh-CN');
  });

  it('normalizes old underscore i18n cookies before bootstrapping the provider', () => {
    setNavigatorLanguages('fr-FR');
    document.cookie = 'i18n=zh_CN;path=/';
    const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => undefined);

    const i18n = createI18n();

    expect(i18n.language).toBe('zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBe('zh-CN');
    expect(window.g_lang).toBe('zh-CN');
    expect(reload).not.toHaveBeenCalled();
  });

  it('ignores invalid i18n cookies instead of throwing before the app mounts', () => {
    setNavigatorLanguages('fr-FR');
    document.cookie = 'i18n=bad-locale;path=/';

    const i18n = createI18n();

    expect(i18n.language).toBe('zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBe('zh-CN');
    expect(window.g_lang).toBe('zh-CN');
  });

  it('ignores malformed i18n cookie encoding instead of throwing before the app mounts', () => {
    setNavigatorLanguages('fr-FR');
    document.cookie = 'i18n=%E0%A4%A;path=/';

    const i18n = createI18n();

    expect(i18n.language).toBe('zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBe('zh-CN');
    expect(window.g_lang).toBe('zh-CN');
  });

  it('bootstraps the provider from an exact supported navigator language', () => {
    setNavigatorLanguages('en-US@posix');

    const i18n = createI18n();

    expect(window.localStorage.getItem('umi_locale')).toBe('en-US');
    expect(i18n.language).toBe('en-US');
    expect(window.g_lang).toBe('en-US');
  });

  it('preserves a traditional-Chinese navigator region instead of collapsing it to zh-CN', () => {
    setNavigatorLanguages('zh-Hant-HK');

    const i18n = createI18n();

    expect(window.localStorage.getItem('umi_locale')).toBe('zh-TW');
    expect(i18n.language).toBe('zh-TW');
    expect(window.g_lang).toBe('zh-TW');
  });

  it('uses the first supported secondary navigator language', () => {
    setNavigatorLanguages('fr-FR', 'ja-JP', 'en-US');

    const i18n = createI18n();

    expect(window.localStorage.getItem('umi_locale')).toBe('ja-JP');
    expect(i18n.language).toBe('ja-JP');
    expect(window.g_lang).toBe('ja-JP');
  });
});
