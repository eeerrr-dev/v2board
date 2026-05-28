import zhCN from './zh-CN';
import type { Translations } from './zh-CN';

const SIMP_TO_TRAD: Record<string, string> = {
  '订': '訂',
  '阅': '閱',
  '账': '帳',
  '户': '戶',
  '识': '識',
  '种': '種',
  '号': '號',
  '码': '碼',
  '续': '續',
  '验': '驗',
  '页': '頁',
  '设': '設',
  '备': '備',
  '务': '務',
  '业': '業',
  '员': '員',
  '邮': '郵',
  '箱': '箱',
  '统': '統',
  '剩': '剩',
  '余': '餘',
  '关': '關',
  '闭': '閉',
  '请': '請',
  '稍': '稍',
  '过': '過',
  '据': '據',
  '点': '點',
  '处': '處',
  '现': '現',
  '提': '提',
  '历': '歷',
  '记': '記',
  '录': '錄',
  '钟': '鐘',
  '联': '聯',
  '础': '礎',
  '认': '認',
  '终': '終',
  '试': '試',
  '词': '詞',
  '语': '語',
  '简': '簡',
  '繁': '繁',
  '币': '幣',
  '种类': '種類',
  '余额': '餘額',
  '系统': '系統',
  '邮箱': '郵箱',
  '账号': '帳號',
  '订阅': '訂閱',
  '订单': '訂單',
  '页面': '頁面',
  '请求': '請求',
  '设置': '設置',
  '验证': '驗證',
  '登录': '登入',
  '注销': '登出',
  '继续': '繼續',
  '关闭': '關閉',
  '试用': '試用',
  '请稍后': '請稍後',
  '请稍候': '請稍候',
  '记住我': '記住我',
  '已购买': '已購買',
  '退出登录': '登出',
};

function toTraditional(value: string): string {
  let out = value;
  for (const [s, t] of Object.entries(SIMP_TO_TRAD)) {
    out = out.split(s).join(t);
  }
  return out;
}

function translateDeep<T>(input: T): T {
  if (typeof input === 'string') return toTraditional(input) as T;
  if (Array.isArray(input)) return input.map((v) => translateDeep(v)) as T;
  if (input && typeof input === 'object') {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(input as Record<string, unknown>)) {
      out[k] = translateDeep(v);
    }
    return out as T;
  }
  return input;
}

const zhTW: Translations = translateDeep(zhCN);

export default zhTW;
