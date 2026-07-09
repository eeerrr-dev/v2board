import {
  cloneElement,
  useEffect,
  useMemo,
  useRef,
  useState,
  type Dispatch,
  type ReactElement,
  type ReactNode,
  type SetStateAction,
} from 'react';
import { useLocation } from 'react-router';
import {
  ArrowDown,
  ArrowUp,
  ChevronDown,
  Copy,
  Database,
  ExternalLink,
  ListFilter,
  Loader2,
  Pencil,
  Plus,
  Trash2,
  User,
  X,
} from 'lucide-react';
import { admin } from '@v2board/api-client';
import {
  useCopyServerMutation,
  useDropServerGroupMutation,
  useDropServerMutation,
  useDropServerRouteMutation,
  useSaveServerGroupMutation,
  useSaveServerRouteMutation,
  useServerGroups,
  useServerNodes,
  useServerRoutes,
  useSortServerNodesMutation,
  useUpdateServerMutation,
} from '@/lib/queries';
import { apiClient } from '@/lib/api';
import { toast } from '@/lib/toast';
import { cn } from '@/lib/cn';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { HeaderTooltip } from '@/components/ui/header-tooltip';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { PageHeader, PageShell } from '@/components/ui/page';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Sheet,
  SheetContent,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { TooltipProvider } from '@/components/ui/tooltip';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';

// The admin server manager is a redesigned shadcn island. The Tier-1 contract is
// the shared backend + the live proxy nodes that consume the per-protocol node
// config: every field key/default/coercion in the exported pure helpers below
// (getLegacyServerInitialValues, prepareLegacyServerPayload, the option/default
// tables, createServerSortPayload, applyServerNodeColumnControls, …) is preserved
// byte-for-byte. Only the UI layer moved from the legacy antd/Bootstrap replica to
// shadcn/Radix. Legacy DOM byte-pins are retired.

type SelectValueType = string | number | null | undefined;
type SelectOption = { value: string | number | null; label: string };

const SERVER_TYPES: admin.ServerTypeName[] = [
  'v2node',
  'shadowsocks',
  'vmess',
  'trojan',
  'hysteria',
  'tuic',
  'vless',
  'anytls',
];

const SERVER_TYPE_LABELS: Record<string, string> = {
  v2node: 'V2node',
  shadowsocks: 'Shadowsocks',
  vmess: 'VMess',
  trojan: 'Trojan',
  hysteria: 'Hysteria',
  tuic: 'Tuic',
  vless: 'VLess',
  anytls: 'AnyTLS',
};

const SERVER_TYPE_COLORS: Record<string, string> = {
  shadowsocks: '#489851',
  vmess: '#CB3180',
  trojan: '#EAB854',
  hysteria: '#1A1A1A',
  tuic: '#9400D3',
  vless: '#4080FF',
  anytls: '#FF8C00',
  v2node: '#FF0000',
};

const AVAILABLE_STATUS: Record<number, 'error' | 'warning' | 'processing'> = {
  0: 'error',
  1: 'warning',
  2: 'processing',
};

const AVAILABLE_STATUS_DOT: Record<'error' | 'warning' | 'processing', string> = {
  error: 'bg-destructive',
  warning: 'bg-amber-500',
  processing: 'bg-sky-500',
};

type RouteActionTextMap = Record<string, string> &
  Record<
    'block' | 'block_ip' | 'block_port' | 'protocol' | 'dns' | 'route' | 'route_ip' | 'default_out',
    string
  >;

const ROUTE_ACTION_TEXT: RouteActionTextMap = {
  block: '禁止访问(域名目标)',
  block_ip: '禁止访问(IP目标)',
  block_port: '禁止访问(端口目标)',
  protocol: '禁止访问(协议)',
  dns: '指定DNS服务器进行解析',
  route: '指定出站服务器(域名目标)',
  route_ip: '指定出站服务器(IP目标)',
  default_out: '自定义默认出站',
};

const LEGACY_BINARY_SELECT_OPTIONS: SelectOption[] = [
  { value: 0, label: '否' },
  { value: 1, label: '是' },
];

const LEGACY_TLS_SUPPORT_OPTIONS: SelectOption[] = [
  { value: 0, label: '不支持' },
  { value: 1, label: '支持' },
];

const LEGACY_SECURITY_NONE_OPTION: SelectOption = { value: 0, label: '无' };
const LEGACY_SECURITY_TLS_OPTION: SelectOption = { value: 1, label: 'TLS' };
const LEGACY_SECURITY_REALITY_OPTION: SelectOption = { value: 2, label: 'Reality' };
const LEGACY_STREAM_NETWORK_OPTIONS: SelectOption[] = [
  { value: 'tcp', label: 'TCP' },
  { value: 'ws', label: 'WebSocket' },
  { value: 'grpc', label: 'gRPC' },
  { value: 'kcp', label: 'mKCP' },
  { value: 'httpupgrade', label: 'HTTPUpgrade' },
  { value: 'xhttp', label: 'XHTTP' },
];
const LEGACY_TROJAN_NETWORK_OPTIONS: SelectOption[] = [
  { value: 'tcp', label: 'TCP' },
  { value: 'ws', label: 'WebSocket' },
  { value: 'grpc', label: 'gRPC' },
];
const LEGACY_V2NODE_PROTOCOL_OPTIONS: SelectOption[] = [
  { value: 'anytls', label: 'AnyTLS' },
  { value: 'hysteria2', label: 'Hysteria2' },
  { value: 'shadowsocks', label: 'Shadowsocks' },
  { value: 'trojan', label: 'Trojan' },
  { value: 'tuic', label: 'Tuic' },
  { value: 'vless', label: 'VLess' },
  { value: 'vmess', label: 'VMess' },
];
const LEGACY_V2NODE_SHADOWSOCKS_NETWORK_OPTIONS: SelectOption[] = [
  { value: 'tcp', label: 'TCP' },
  { value: 'http', label: 'HTTP伪装' },
];
const LEGACY_V2NODE_TRANSPORT_OPTIONS: SelectOption[] = [
  { value: 'tcp', label: 'TCP' },
  { value: 'ws', label: 'WebSocket' },
  { value: 'grpc', label: 'gRPC' },
  { value: 'httpupgrade', label: 'HTTPUpgrade' },
  { value: 'xhttp', label: 'XHTTP' },
];
const LEGACY_HYSTERIA2_OBFS_OPTIONS: SelectOption[] = [
  { value: null, label: '无' },
  { value: 'salamander', label: 'salamander' },
];
const LEGACY_TUIC_RELAY_MODE_OPTIONS: SelectOption[] = [
  { value: 'native', label: 'native' },
  { value: 'quic', label: 'quic' },
];
const LEGACY_TUIC_CONGESTION_CONTROL_OPTIONS: SelectOption[] = [
  { value: 'cubic', label: 'cubic' },
  { value: 'new_reno', label: 'new_reno' },
  { value: 'bbr', label: 'bbr' },
];
const LEGACY_SHADOWSOCKS_CIPHER_OPTIONS: SelectOption[] = [
  { value: 'aes-128-gcm', label: 'aes-128-gcm' },
  { value: 'aes-192-gcm', label: 'aes-192-gcm' },
  { value: 'aes-256-gcm', label: 'aes-256-gcm' },
  { value: 'chacha20-ietf-poly1305', label: 'chacha20-ietf-poly1305' },
  { value: '2022-blake3-aes-128-gcm', label: '2022-blake3-aes-128-gcm' },
  { value: '2022-blake3-aes-256-gcm', label: '2022-blake3-aes-256-gcm' },
];
const LEGACY_SHADOWSOCKS_OBFS_OPTIONS: SelectOption[] = [
  { value: '', label: '无' },
  { value: 'http', label: 'HTTP' },
];
const LEGACY_VLESS_ENCRYPTION_OPTIONS: SelectOption[] = [
  { value: null, label: '无' },
  { value: 'mlkem768x25519plus', label: 'MLKEM768X25519PLUS' },
];
const LEGACY_VLESS_FLOW_NONE_OPTIONS: SelectOption[] = [{ value: null, label: '无' }];
const LEGACY_VLESS_FLOW_OPTIONS: SelectOption[] = [
  ...LEGACY_VLESS_FLOW_NONE_OPTIONS,
  { value: 'xtls-rprx-vision', label: 'xtls-rprx-vision' },
];
const LEGACY_HYSTERIA_VERSION_OPTIONS: SelectOption[] = [
  { value: 1, label: 'v1' },
  { value: 2, label: 'v2' },
];
const LEGACY_HYSTERIA_V1_OBFS_OPTIONS: SelectOption[] = [
  { value: null, label: '无' },
  { value: 'xplus', label: 'xplus' },
];
const LEGACY_TLS_CERT_MODE_OPTIONS: SelectOption[] = [
  { value: 'self', label: '自签名' },
  { value: 'http', label: 'HTTP申请' },
  { value: 'dns', label: 'DNS申请' },
  { value: 'none', label: '无证书(关闭TLS)' },
];
const LEGACY_PROXY_PROTOCOL_OPTIONS: SelectOption[] = [
  { value: 0, label: '0' },
  { value: 1, label: '1' },
  { value: 2, label: '2' },
];
const LEGACY_TLS_FINGERPRINT_OPTIONS: SelectOption[] = [
  { value: 'chrome', label: 'Chrome' },
  { value: 'firefox', label: 'Firefox' },
  { value: 'safari', label: 'Safari' },
  { value: 'ios', label: 'IOS' },
  { value: 'android', label: 'Android' },
  { value: 'edge', label: 'Edge' },
  { value: '360', label: '360' },
  { value: 'qq', label: 'QQ' },
];
const LEGACY_ECH_MODE_OPTIONS: SelectOption[] = [
  { value: '', label: '无' },
  { value: 'cloudflare', label: 'Cloudflare' },
  { value: 'custom', label: '自定义 SNI' },
];
const LEGACY_ENCRYPTION_MODE_OPTIONS: SelectOption[] = [
  { value: 'native', label: 'native' },
  { value: 'xorpub', label: 'xorpub' },
  { value: 'random', label: 'random' },
];
const LEGACY_ENCRYPTION_RTT_OPTIONS: SelectOption[] = [
  { value: '0rtt', label: '0rtt' },
  { value: '1rtt', label: '1rtt' },
];

const LEGACY_HABIT_KEY = 'habit';
const LEGACY_SERVER_PAGE_SIZE_KEY = 'server_manage_page_size';
const LEGACY_SERVER_SORT_PROMPT = '节点排序还没有保存，是否离开';
const ANYTLS_PADDING_SCHEME_PLACEHOLDER = JSON.stringify(
  [
    'stop=8',
    '0=30-30',
    '1=100-400',
    '2=400-500,c,500-1000,c,500-1000,c,500-1000,c,500-1000',
    '3=9-9,500-1000',
    '4=500-1000',
    '5=500-1000',
    '6=500-1000',
    '7=500-1000',
  ],
  null,
  4,
);
const LEGACY_TLS_FORCED_PROTOCOLS = ['anytls', 'hysteria2', 'trojan', 'tuic'];
const LEGACY_V2NODE_SECURITY_FALLBACK_PROTOCOLS = ['hysteria2', 'trojan', 'tuic'];
const LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: JSON.stringify(
    {
      header: {
        type: 'http',
        request: {
          path: ['/'],
          headers: {
            Host: ['www.baidu.com', 'www.bing.com'],
          },
        },
        response: {},
      },
    },
    null,
    4,
  ),
  ws: JSON.stringify(
    {
      path: '/',
      headers: {
        Host: 'v2ray.com',
      },
    },
    null,
    4,
  ),
  grpc: JSON.stringify(
    {
      serviceName: 'GunService',
    },
    null,
    4,
  ),
  kcp: JSON.stringify(
    {
      header: {
        type: 'none',
      },
      seed: '',
    },
    null,
    4,
  ),
  httpupgrade: JSON.stringify(
    {
      path: '/',
      host: 'xtls.github.io',
    },
    null,
    4,
  ),
  xhttp: JSON.stringify(
    {
      path: '/',
      host: 'xtls.github.io',
    },
    null,
    4,
  ),
};
const LEGACY_VLESS_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.tcp!,
  ws: JSON.stringify(
    {
      security: 'auto',
      path: '/',
      headers: {
        Host: 'xtls.github.io',
      },
    },
    null,
    4,
  ),
  grpc: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.grpc!,
  kcp: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.kcp!,
  httpupgrade: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.httpupgrade!,
  xhttp: JSON.stringify(
    {
      path: '/',
      host: 'xtls.github.io',
      mode: 'auto',
      extra: {},
    },
    null,
    4,
  ),
};
const LEGACY_TROJAN_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: '',
  ws: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.ws!,
  grpc: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.grpc!,
};
const LEGACY_V2NODE_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: JSON.stringify(
    {
      acceptProxyProtocol: false,
      header: {
        type: 'http',
        request: {
          path: ['/'],
          headers: {
            Host: ['www.baidu.com', 'www.bing.com'],
          },
        },
        response: {},
      },
    },
    null,
    4,
  ),
  http: JSON.stringify(
    {
      acceptProxyProtocol: false,
      path: '/',
      Host: 'xtls.github.io',
    },
    null,
    4,
  ),
  ws: JSON.stringify(
    {
      acceptProxyProtocol: false,
      path: '/',
      headers: {
        Host: 'xtls.github.io',
      },
    },
    null,
    4,
  ),
  grpc: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.grpc!,
  httpupgrade: JSON.stringify(
    {
      acceptProxyProtocol: false,
      path: '/',
      host: 'xtls.github.io',
    },
    null,
    4,
  ),
  xhttp: LEGACY_VLESS_NETWORK_SETTINGS_PLACEHOLDERS.xhttp!,
};
const LEGACY_NETWORK_SETTINGS_PLACEHOLDERS: Partial<
  Record<admin.ServerTypeName, Record<string, string>>
> = {
  vmess: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS,
  vless: LEGACY_VLESS_NETWORK_SETTINGS_PLACEHOLDERS,
  trojan: LEGACY_TROJAN_NETWORK_SETTINGS_PLACEHOLDERS,
  v2node: LEGACY_V2NODE_NETWORK_SETTINGS_PLACEHOLDERS,
};
const LEGACY_TLS_SETTINGS_DEFAULTS: Record<string, unknown> = {
  server_name: '',
  cert_mode: 'self',
  provider: '',
  dns_env: '',
  reject_unknown_sni: '0',
  allow_insecure: '0',
};
const LEGACY_ENCRYPTION_SETTINGS_DEFAULTS: Record<string, unknown> = {
  mode: 'native',
  rtt: '0rtt',
  ticket: '600s',
  server_padding: null,
  client_padding: null,
  private_key: null,
  password: null,
};

function readLegacyHabit(key: string): unknown {
  if (typeof window === 'undefined') return undefined;
  try {
    const stored = window.localStorage.getItem(LEGACY_HABIT_KEY);
    if (!stored) return undefined;
    const parsed = JSON.parse(stored) as Record<string, unknown>;
    return parsed?.[key];
  } catch {
    return undefined;
  }
}

function writeLegacyHabit(key: string, value: unknown) {
  if (typeof window === 'undefined') return;
  try {
    const stored = window.localStorage.getItem(LEGACY_HABIT_KEY);
    if (stored) {
      const legacyHabit = JSON.parse(stored) as Record<string, unknown>;
      legacyHabit[key] = value;
      window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify(legacyHabit));
    } else {
      window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify({ [key]: value }));
    }
  } catch {
    window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify({ [key]: value }));
  }
}

// Node ID column filter values; the legacy onFilter matched `node.type === value.toLowerCase()`.
const NODE_TYPE_FILTERS = [
  'V2node',
  'Shadowsocks',
  'Vmess',
  'Trojan',
  'Hysteria',
  'Tuic',
  'Vless',
  'AnyTLS',
].map((value) => ({ text: value, value }));

interface NodeFilterItem {
  text: string;
  value: string;
}

function readLegacyServerPageSize() {
  const pageSize = Number(readLegacyHabit(LEGACY_SERVER_PAGE_SIZE_KEY));
  return Number.isFinite(pageSize) && pageSize > 0 ? pageSize : 10;
}

// ---------------------------------------------------------------------------
// Pure contract helpers (Tier-1). Names, signatures and behavior are preserved
// byte-for-byte from the legacy replica; only the UI around them changed.
// ---------------------------------------------------------------------------

function getRouteMatchLabel(value: admin.ServerRoute['match'] | undefined) {
  if (!value || value.length === 0) return '无规则时默认';
  const rules = typeof value === 'string' ? value.split(',').filter(Boolean) : value;
  return `匹配 ${rules.length} 条规则`;
}

function getRouteMatchTextareaValue(value: admin.ServerRoute['match'] | undefined) {
  if (Array.isArray(value)) return value.join('\n');
  return value?.split(',').join('\n');
}

function getRouteMatchPlaceholder(action: string | undefined) {
  if (action === 'protocol') return 'http\ntls\nquic\nbittorrent';
  if (action === 'block_port') return '53\n443\n1000-2000';
  if (action && ['route_ip', 'block_ip'].includes(action)) {
    return '127.0.0.1(单一匹配)\n10.0.0.0/8(范围匹配)\ngeoip:cn(预定义列表匹配)';
  }
  return 'example.com(关键字匹配)\ndomain:example.com(子域名匹配)\ngeosite:netflix(预定义域名列表)';
}

function getLegacyAvailableStatus(status?: number | null) {
  return status == null ? undefined : AVAILABLE_STATUS[status];
}

// Applies the legacy antd Table column controls to the node list: the 节点ID type filter
// (`node.type === label.toLowerCase()`), the 权限组 group filter (string membership, OR across the
// selected groups), then the 人数 online sorter — matching the order antd uses (filter, then sort).
export function applyServerNodeColumnControls(
  nodes: admin.ServerNode[],
  controls: { typeFilter: string[]; groupFilter: string[]; onlineSort: '' | 'ascend' | 'descend' },
): admin.ServerNode[] {
  let result = nodes;
  if (controls.typeFilter.length) {
    result = result.filter((node) =>
      controls.typeFilter.some((value) => node.type === value.toLowerCase()),
    );
  }
  if (controls.groupFilter.length) {
    result = result.filter((node) =>
      node.group_id.map(String).some((id) => controls.groupFilter.includes(id)),
    );
  }
  if (controls.onlineSort) {
    const direction = controls.onlineSort === 'ascend' ? 1 : -1;
    result = [...result].sort((a, b) => (a.online - b.online) * direction);
  }
  return result;
}

export function createServerSortPayload(nodes: admin.ServerNode[]) {
  return nodes.reduce<Record<string, Record<string | number, number>>>((payload, node, index) => {
    const typePayload = payload[node.type] ?? {};
    typePayload[node.id] = index;
    payload[node.type] = typePayload;
    return payload;
  }, {});
}

export function moveServerNodeByLegacyDragIndexes(
  nodes: admin.ServerNode[],
  fromIndex: number,
  toIndex: number,
) {
  const next = [...nodes];
  const moved = next[fromIndex];
  if (!moved || fromIndex === toIndex) return next;
  if (fromIndex < toIndex) {
    next.splice(toIndex + 1, 0, moved);
    next.splice(fromIndex, 1);
  } else {
    next.splice(toIndex, 0, moved);
    next.splice(fromIndex + 1, 1);
  }
  return next;
}

export function shouldPromptLegacyServerSortClick(target: EventTarget | null) {
  if (!(target instanceof Element)) return false;
  if (target.closest('.nav-main-link')) return true;
  if (target.closest('.dropdown-item')) return true;

  const anchor = target.closest('a');
  if (!anchor) return false;
  const href = anchor.getAttribute('href');
  return Boolean(href && !href.startsWith('javascript:'));
}

export function installLegacyServerSortPrompt(message = LEGACY_SERVER_SORT_PROMPT) {
  if (typeof window === 'undefined') return () => {};

  let lastHash = window.location.hash;
  let restoringHash = false;

  const confirmLeave = () => window.confirm(message);

  const warnBeforeUnload = (event: BeforeUnloadEvent) => {
    event.preventDefault();
    // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
    event.returnValue = message;
    return message;
  };

  const warnBeforeRouteClick = (event: MouseEvent) => {
    if (!shouldPromptLegacyServerSortClick(event.target)) return;
    if (confirmLeave()) return;
    event.preventDefault();
    event.stopImmediatePropagation();
  };

  const warnBeforeHashChange = () => {
    if (restoringHash) {
      restoringHash = false;
      return;
    }
    if (window.location.hash === lastHash) return;
    if (confirmLeave()) {
      lastHash = window.location.hash;
      return;
    }
    restoringHash = true;
    window.location.hash = lastHash || '#/server/manage';
  };

  window.addEventListener('beforeunload', warnBeforeUnload);
  document.addEventListener('click', warnBeforeRouteClick, true);
  window.addEventListener('hashchange', warnBeforeHashChange);

  return () => {
    window.removeEventListener('beforeunload', warnBeforeUnload);
    document.removeEventListener('click', warnBeforeRouteClick, true);
    window.removeEventListener('hashchange', warnBeforeHashChange);
  };
}

function parseLegacyJsonPayloadField(payload: Record<string, unknown>, field: string) {
  const value = payload[field];
  if (!value) {
    payload[field] = null;
    return;
  }
  payload[field] = typeof value === 'string' ? JSON.parse(value) : value;
}

function normalizeLegacyNullableArray(value: unknown) {
  return Array.isArray(value) && value.length === 0 ? null : value;
}

function prepareLegacyServerPayload(
  type: admin.ServerTypeName,
  values: Record<string, unknown>,
  id?: number,
) {
  const payload: Record<string, unknown> = { ...values, id };
  if (type === 'vmess') {
    parseLegacyJsonPayloadField(payload, 'networkSettings');
    const dnsSettings = payload.dnsSettings as { servers?: unknown[] } | undefined;
    if (!dnsSettings?.servers?.length) payload.dnsSettings = null;
  }
  if (type === 'trojan' || type === 'vless' || type === 'v2node') {
    parseLegacyJsonPayloadField(payload, 'network_settings');
  }
  if (type === 'v2node') {
    delete payload.install_command;
  }
  return payload;
}

export function getLegacyServerInitialValues(
  type: admin.ServerTypeName,
  record?: Partial<admin.ServerNode>,
): Record<string, unknown> {
  const editing = Boolean(record);
  const normalizedRecord: Record<string, unknown> = record
    ? { ...(record as Record<string, unknown>) }
    : {};
  if (normalizedRecord.networkSettings && typeof normalizedRecord.networkSettings === 'object') {
    normalizedRecord.networkSettings = JSON.stringify(normalizedRecord.networkSettings, null, 2);
  }
  if (normalizedRecord.network_settings && typeof normalizedRecord.network_settings === 'object') {
    normalizedRecord.network_settings = JSON.stringify(normalizedRecord.network_settings, null, 2);
  }
  const tuicDefaults =
    type === 'tuic'
      ? {
          insecure: 0,
          disable_sni: 0,
          udp_relay_mode: 'native',
          zero_rtt_handshake: 0,
          congestion_control: 'cubic',
        }
      : {};
  const shadowsocksDefaults = type === 'shadowsocks' ? { cipher: 'chacha20-ietf-poly1305' } : {};
  const vmessDefaults = type === 'vmess' ? { tls: 0 } : {};
  const trojanDefaults = type === 'trojan' ? { tls: 0 } : {};
  const hysteriaDefaults = type === 'hysteria' ? { insecure: 0, version: 1 } : {};
  const vlessDefaults = type === 'vless' ? { tls: 0, flow: null } : {};
  const anyTlsDefaults = type === 'anytls' ? { insecure: 0 } : {};
  const v2nodeDefaults =
    type === 'v2node'
      ? {
          tls: 0,
          network: 'tcp',
          disable_sni: 0,
          zero_rtt_handshake: 0,
          flow: null,
        }
      : {};

  return {
    ...(editing
      ? {}
      : {
          rate: 1,
          ...tuicDefaults,
          ...shadowsocksDefaults,
          ...vmessDefaults,
          ...trojanDefaults,
          ...hysteriaDefaults,
          ...vlessDefaults,
          ...anyTlsDefaults,
          ...v2nodeDefaults,
        }),
    ...normalizedRecord,
  };
}

export function getLegacyNetworkSettingsPlaceholder(type: admin.ServerTypeName, network: unknown) {
  return LEGACY_NETWORK_SETTINGS_PLACEHOLDERS[type]?.[String(network)] || '';
}

export function getLegacyV2nodeSecurityValue(protocol: unknown, tls: unknown) {
  const parsedTls = parseInt(String(tls ?? 0), 10);
  if (parsedTls) return parsedTls;
  const protocolValue = protocol == null ? null : String(protocol);
  return protocolValue && LEGACY_V2NODE_SECURITY_FALLBACK_PROTOCOLS.includes(protocolValue) ? 1 : 0;
}

function getLegacyV2nodeSecurityOptions(protocol: unknown): SelectOption[] {
  const protocolValue = protocol == null ? null : String(protocol);
  return [
    ...(protocolValue === 'vless' || protocolValue === 'vmess'
      ? [LEGACY_SECURITY_NONE_OPTION]
      : []),
    LEGACY_SECURITY_TLS_OPTION,
    ...(protocolValue === 'vless' || protocolValue === 'anytls'
      ? [LEGACY_SECURITY_REALITY_OPTION]
      : []),
  ];
}

function getLegacyV2nodeTransportOptions(protocol: unknown): SelectOption[] {
  return protocol === 'trojan' ? LEGACY_TROJAN_NETWORK_OPTIONS : LEGACY_V2NODE_TRANSPORT_OPTIONS;
}

function getLegacyVlessFlowOptions(network: unknown): SelectOption[] {
  return String(network) === 'tcp' ? LEGACY_VLESS_FLOW_OPTIONS : LEGACY_VLESS_FLOW_NONE_OPTIONS;
}

export function getLegacyNumericSelectValue(value: unknown, fallback = 0) {
  return parseInt(String(value ?? fallback), 10) || fallback;
}

export function getLegacyBinarySelectValue(value: unknown) {
  return getLegacyNumericSelectValue(value) ? 1 : 0;
}

function normalizeLegacySettings(
  value: unknown,
  defaults: Record<string, unknown>,
): Record<string, unknown> {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    return { ...defaults, ...(value as Record<string, unknown>) };
  }
  if (typeof value === 'string' && value.trim()) {
    try {
      const parsed = JSON.parse(value) as unknown;
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return { ...defaults, ...(parsed as Record<string, unknown>) };
      }
    } catch {
      return { ...defaults };
    }
  }
  return { ...defaults };
}

// The item-level defaults antd's <Form.Item initialValue> registered on mount but
// that getLegacyServerInitialValues intentionally does not carry (its unit test
// pins the exact shapes). Applied only when the store has no value yet, exactly
// like antd. Form-level initial values still win.
function getNodeInitialValues(
  type: admin.ServerTypeName,
  record?: Partial<admin.ServerNode>,
): Record<string, unknown> {
  const base = getLegacyServerInitialValues(type, record);
  const itemDefaults: Record<string, unknown> = {};
  if (type === 'trojan') itemDefaults.allow_insecure = 0;
  if (type === 'shadowsocks') itemDefaults.obfs = '';
  if (type === 'v2node') {
    const protocol = (record as Record<string, unknown> | undefined)?.protocol;
    if (protocol === 'shadowsocks') itemDefaults.cipher = 'aes-128-gcm';
    if (protocol === 'tuic') {
      itemDefaults.udp_relay_mode = 'native';
      itemDefaults.congestion_control = 'cubic';
    }
  }
  return { ...itemDefaults, ...base };
}

function legacyText(value: unknown) {
  return value == null ? '' : String(value);
}

function inputValue(value: unknown) {
  return value == null ? '' : (value as string | number);
}

function legacyBool(value: unknown) {
  return parseInt(String(value ?? 0), 10) !== 0;
}

// ---------------------------------------------------------------------------
// Shared shadcn form primitives.
// ---------------------------------------------------------------------------

interface NodeForm {
  values: Record<string, unknown>;
  setField: (name: string | [string, string], value: unknown) => void;
  setFields: (partial: Record<string, unknown>) => void;
  setValues: Dispatch<SetStateAction<Record<string, unknown>>>;
}

// Round-trips a typed (string | number | null) select value through Radix Select,
// which only speaks non-empty strings, by keying options on their index and mapping
// back to the original typed value on change so prepareLegacyServerPayload keeps
// receiving the exact type it always did.
function NodeSelect({
  value,
  options,
  placeholder,
  onChange,
  className,
  id,
  testId,
}: {
  value: SelectValueType;
  options: SelectOption[];
  placeholder?: string;
  onChange: (value: string | number | null) => void;
  className?: string;
  id?: string;
  testId?: string;
}) {
  const selectedIndex = options.findIndex((option) => option.value === value);
  return (
    <Select
      value={selectedIndex >= 0 ? String(selectedIndex) : undefined}
      onValueChange={(next) => {
        const option = options[Number(next)];
        onChange(option ? option.value : null);
      }}
    >
      <SelectTrigger id={id} className={cn('w-full', className)} data-testid={testId}>
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
        {options.map((option, index) => (
          <SelectItem key={index} value={String(index)}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function MultiCheckboxField({
  options,
  value,
  onChange,
  testId,
  emptyText,
}: {
  options: { value: string; label: string }[];
  value: string[];
  onChange: (value: string[]) => void;
  testId?: string;
  emptyText?: string;
}) {
  if (!options.length) {
    return <p className="text-sm text-muted-foreground">{emptyText ?? '暂无可选项'}</p>;
  }
  const toggle = (option: string, checked: boolean) => {
    onChange(checked ? [...value, option] : value.filter((item) => item !== option));
  };
  return (
    <div
      className="flex flex-wrap gap-x-4 gap-y-2 rounded-md border border-input p-3"
      data-testid={testId}
    >
      {options.map((option) => {
        const checked = value.includes(option.value);
        return (
          <label
            key={option.value}
            className="flex cursor-pointer items-center gap-2 text-sm text-foreground"
          >
            <Checkbox
              checked={checked}
              onCheckedChange={(next) => toggle(option.value, next === true)}
            />
            {option.label}
          </label>
        );
      })}
    </div>
  );
}

function TagsInput({
  value,
  onChange,
  placeholder,
  testId,
}: {
  value: string[];
  onChange: (value: string[]) => void;
  placeholder?: string;
  testId?: string;
}) {
  const [draft, setDraft] = useState('');
  const add = () => {
    const tag = draft.trim();
    setDraft('');
    if (!tag || value.includes(tag)) return;
    onChange([...value, tag]);
  };
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border border-input p-2">
      {value.map((tag) => (
        <Badge key={tag} variant="secondary" className="gap-1">
          {tag}
          <button
            type="button"
            aria-label={`移除标签 ${tag}`}
            onClick={() => onChange(value.filter((item) => item !== tag))}
          >
            <X className="size-3" />
          </button>
        </Badge>
      ))}
      <input
        className="min-w-24 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
        value={draft}
        data-testid={testId}
        placeholder={value.length ? '' : placeholder}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            event.preventDefault();
            add();
          }
        }}
        onBlur={add}
      />
    </div>
  );
}

function ServerTypeTag({ type, children }: { type: string; children: ReactNode }) {
  return (
    <Badge
      className="border-transparent text-white"
      style={{ backgroundColor: SERVER_TYPE_COLORS[type] }}
    >
      {children}
    </Badge>
  );
}

function AvailabilityDot({ status }: { status?: number | null }) {
  const tone = getLegacyAvailableStatus(status);
  if (!tone) return null;
  return (
    <span
      aria-hidden="true"
      className={cn('inline-block size-2 shrink-0 rounded-full', AVAILABLE_STATUS_DOT[tone])}
    />
  );
}

// ---------------------------------------------------------------------------
// Page dispatch.
// ---------------------------------------------------------------------------

export default function ServersPage() {
  const location = useLocation();
  if (location.pathname === '/server/group') return <ServerGroupPage />;
  if (location.pathname === '/server/route') return <ServerRoutePage />;
  if (location.pathname === '/server/manage') return <ServerManagePage />;

  return null;
}

// ---------------------------------------------------------------------------
// Server groups.
// ---------------------------------------------------------------------------

function ServerGroupPage() {
  const groups = useServerGroups();
  const save = useSaveServerGroupMutation();
  const drop = useDropServerGroupMutation();
  const data = groups.data ?? [];

  const saveGroup = async (payload: Partial<admin.ServerGroup>) => {
    await save.mutateAsync({ ...payload });
    await groups.refetch();
  };

  const removeGroup = async (record: admin.ServerGroup) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该权限组吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    drop.mutate(record.id, {
      onSuccess: () => {
        void groups.refetch();
      },
    });
  };

  const columns: DataTableColumn<admin.ServerGroup>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>组ID</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>组名称</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'user_count',
      header: () => <span>用户数量</span>,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-1.5 tabular-nums">
          <User className="size-4 text-muted-foreground" /> {row.original.user_count}
        </span>
      ),
    },
    {
      id: 'server_count',
      header: () => <span>节点数量</span>,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-1.5 tabular-nums">
          <Database className="size-4 text-muted-foreground" /> {row.original.server_count}
        </span>
      ),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <ServerGroupModal record={row.original} pending={save.isPending} onSave={saveGroup}>
            <Button variant="ghost" size="sm" data-testid={`server-group-edit-${row.original.id}`}>
              <Pencil className="size-4" />
              编辑
            </Button>
          </ServerGroupModal>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive"
            onClick={() => void removeGroup(row.original)}
            data-testid={`server-group-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="server-group-page">
      <PageHeader
        title="权限组管理"
        actions={
          <ServerGroupModal pending={save.isPending} onSave={saveGroup}>
            <Button data-testid="server-group-create">
              <Plus className="size-4" />
              添加权限组
            </Button>
          </ServerGroupModal>
        }
      />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <DataTable
            columns={columns}
            data={data}
            getRowKey={(row) => row.id}
            className="min-w-[720px]"
            data-testid="server-groups-table"
            empty={data.length === 0 ? '暂无权限组' : undefined}
            emptyTestId="server-groups-empty"
          />
        </CardContent>
      </Card>

      {groups.isFetching ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

function ServerGroupModal({
  record,
  pending,
  onSave,
  children,
}: {
  record?: admin.ServerGroup;
  pending: boolean;
  onSave: (payload: Partial<admin.ServerGroup>) => Promise<unknown>;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const [submit, setSubmit] = useState<Partial<admin.ServerGroup>>(record ?? {});

  const openModal = () => {
    setSubmit(record ?? {});
    setOpen(true);
  };

  const saveGroup = async () => {
    await onSave({ ...submit });
    setOpen(false);
  };

  return (
    <>
      {cloneElement(children, { onClick: openModal })}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent data-testid="server-group-editor">
          <DialogHeader>
            <DialogTitle>{submit.id ? '编辑组' : '创建组'}</DialogTitle>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="server-group-name">组名</Label>
            <Input
              id="server-group-name"
              placeholder="请输入组名"
              value={inputValue(submit.name)}
              onChange={(event) => setSubmit((value) => ({ ...value, name: event.target.value }))}
              data-testid="server-group-name"
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              onClick={() => void saveGroup()}
              disabled={pending}
              data-testid="server-group-submit"
            >
              {pending ? <Loader2 className="size-4 animate-spin" /> : null}
              提交
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

// ---------------------------------------------------------------------------
// Server routes.
// ---------------------------------------------------------------------------

function ServerRoutePage() {
  const routes = useServerRoutes();
  const save = useSaveServerRouteMutation();
  const drop = useDropServerRouteMutation();
  const data = routes.data ?? [];

  const saveRoute = async (route: Partial<admin.ServerRoute>) => {
    const payload = { ...route };
    if (Array.isArray(payload.match)) {
      payload.match = payload.match.filter(Boolean);
    } else if (payload.match && typeof payload.match === 'string') {
      payload.match = payload.match.split(',').filter(Boolean);
    } else {
      payload.match = [];
    }
    await save.mutateAsync(payload);
    await routes.refetch();
  };

  const removeRoute = async (record: admin.ServerRoute) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该路由吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    drop.mutate(record.id, {
      onSuccess: () => {
        void routes.refetch();
      },
    });
  };

  const columns: DataTableColumn<admin.ServerRoute>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>ID</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'remarks',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>备注</span>,
      cell: ({ row }) => row.original.remarks,
    },
    {
      id: 'match',
      header: () => <span>匹配数量</span>,
      cell: ({ row }) => getRouteMatchLabel(row.original.match),
    },
    {
      id: 'action',
      header: () => <span>动作</span>,
      cell: ({ row }) => ROUTE_ACTION_TEXT[row.original.action],
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <ServerRouteModal route={row.original} pending={save.isPending} onSave={saveRoute}>
            <Button variant="ghost" size="sm" data-testid={`server-route-edit-${row.original.id}`}>
              <Pencil className="size-4" />
              编辑
            </Button>
          </ServerRouteModal>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive"
            onClick={() => void removeRoute(row.original)}
            data-testid={`server-route-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="server-route-page">
      <PageHeader
        title="路由管理"
        actions={
          <ServerRouteModal pending={save.isPending} onSave={saveRoute}>
            <Button data-testid="server-route-create">
              <Plus className="size-4" />
              添加路由
            </Button>
          </ServerRouteModal>
        }
      />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <DataTable
            columns={columns}
            data={data}
            getRowKey={(row) => row.id}
            className="min-w-[720px]"
            data-testid="server-routes-table"
            empty={data.length === 0 ? '暂无路由' : undefined}
            emptyTestId="server-routes-empty"
          />
        </CardContent>
      </Card>

      {routes.isFetching ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

function ServerRouteModal({
  route: initialRoute,
  pending,
  onSave,
  children,
}: {
  route?: admin.ServerRoute;
  pending: boolean;
  onSave: (route: Partial<admin.ServerRoute>) => Promise<unknown>;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const [route, setRoute] = useState<Partial<admin.ServerRoute>>(initialRoute ?? {});
  const routeActionOptions: SelectOption[] = [
    { value: 'block', label: ROUTE_ACTION_TEXT.block },
    { value: 'block_ip', label: ROUTE_ACTION_TEXT.block_ip },
    { value: 'block_port', label: ROUTE_ACTION_TEXT.block_port },
    { value: 'protocol', label: ROUTE_ACTION_TEXT.protocol },
    { value: 'dns', label: ROUTE_ACTION_TEXT.dns },
    { value: 'route', label: ROUTE_ACTION_TEXT.route },
    { value: 'route_ip', label: ROUTE_ACTION_TEXT.route_ip },
    { value: 'default_out', label: ROUTE_ACTION_TEXT.default_out },
  ];

  const openModal = () => {
    setRoute(initialRoute ?? {});
    setOpen(true);
  };

  const saveRoute = async () => {
    await onSave(route);
    setOpen(false);
  };

  return (
    <>
      {cloneElement(children, { onClick: openModal })}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-h-[calc(100vh-4rem)] overflow-y-auto" data-testid="server-route-editor">
          <DialogHeader>
            <DialogTitle>{route.id ? '编辑路由' : '创建路由'}</DialogTitle>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="server-route-remarks">备注</Label>
              <Input
                id="server-route-remarks"
                placeholder="请输入备注"
                value={inputValue(route.remarks)}
                onChange={(event) => setRoute((value) => ({ ...value, remarks: event.target.value }))}
                data-testid="server-route-remarks"
              />
            </div>

            {route.action !== 'default_out' ? (
              <div className="space-y-2">
                <Label htmlFor="server-route-match" className="flex items-center gap-2">
                  匹配值
                  <a
                    className="inline-flex items-center gap-1 text-primary"
                    href="https://xtls.github.io/config/routing.html#ruleobject"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <ExternalLink className="size-3.5" />
                    填写参考
                  </a>
                </Label>
                <Textarea
                  id="server-route-match"
                  rows={5}
                  className="font-mono text-xs"
                  placeholder={getRouteMatchPlaceholder(route.action)}
                  value={getRouteMatchTextareaValue(route.match)}
                  onChange={(event) =>
                    setRoute((value) => ({ ...value, match: event.target.value?.split('\n') }))
                  }
                  data-testid="server-route-match"
                />
              </div>
            ) : null}

            <div className="space-y-2">
              <Label>动作</Label>
              <NodeSelect
                value={route.action}
                placeholder="请选择动作"
                options={routeActionOptions}
                onChange={(value) => setRoute((current) => ({ ...current, action: value as string }))}
                testId="server-route-action"
              />
            </div>

            {route.action === 'dns' ? (
              <div className="space-y-2">
                <Label htmlFor="server-route-dns">DNS服务器</Label>
                <Input
                  id="server-route-dns"
                  placeholder="请输入用于解析的DNS服务器地址"
                  value={inputValue(route.action_value)}
                  onChange={(event) =>
                    setRoute((value) => ({ ...value, action_value: event.target.value }))
                  }
                  data-testid="server-route-action-value"
                />
              </div>
            ) : null}

            {route.action === 'route' ||
            route.action === 'route_ip' ||
            route.action === 'default_out' ? (
              <div className="space-y-2">
                <Label htmlFor="server-route-outbound" className="flex items-center gap-2">
                  Xray出站配置
                  <a
                    className="inline-flex items-center gap-1 text-primary"
                    href="https://xtls.github.io/config/outbound.html"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <ExternalLink className="size-3.5" />
                    填写参考
                  </a>
                </Label>
                <Textarea
                  id="server-route-outbound"
                  rows={8}
                  className="font-mono text-xs"
                  placeholder={JSON.stringify(
                    {
                      tag: 'ss_out',
                      sendThrough: '0.0.0.0',
                      protocol: 'shadowsocks',
                      settings: {
                        email: 'love@xray.com',
                        address: '8.8.8.8',
                        port: 5555,
                        method: 'chacha20-ietf-poly1305',
                        password: 'abcdefghijklmnopqrstuvwxyz',
                        level: 0,
                      },
                    },
                    null,
                    4,
                  )}
                  value={inputValue(route.action_value)}
                  onChange={(event) =>
                    setRoute((value) => ({ ...value, action_value: event.target.value }))
                  }
                  data-testid="server-route-action-value"
                />
              </div>
            ) : null}
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              onClick={() => void saveRoute()}
              disabled={pending}
              data-testid="server-route-submit"
            >
              {pending ? <Loader2 className="size-4 animate-spin" /> : null}
              提交
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

// ---------------------------------------------------------------------------
// Server nodes.
// ---------------------------------------------------------------------------

function ServerSortPrompt({ when }: { when: boolean }) {
  useEffect(() => {
    if (!when) return undefined;
    return installLegacyServerSortPrompt();
  }, [when]);
  return null;
}

function NodeFilterMenu({
  items,
  value,
  active,
  onApply,
}: {
  items: NodeFilterItem[];
  value: string[];
  active: boolean;
  onApply: (next: string[]) => void;
}) {
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState<string[]>(value);
  useEffect(() => {
    if (open) setPending(value);
  }, [open, value]);
  const toggle = (target: string) =>
    setPending((prev) =>
      prev.includes(target) ? prev.filter((item) => item !== target) : [...prev, target],
    );
  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label="筛选"
          className={cn(
            'ml-1 inline-flex size-6 items-center justify-center rounded-sm outline-none transition-colors hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50',
            active ? 'text-primary' : 'text-muted-foreground',
          )}
        >
          <ListFilter className="size-3.5" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-40">
        <div className="max-h-64 overflow-y-auto py-1">
          {items.map((item) => (
            <label
              key={item.value}
              className="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
            >
              <Checkbox
                checked={pending.includes(item.value)}
                onCheckedChange={() => toggle(item.value)}
              />
              <span>{item.text}</span>
            </label>
          ))}
        </div>
        <DropdownMenuSeparator />
        <div className="flex items-center justify-between px-2 py-1">
          <button
            type="button"
            className="text-sm text-primary"
            onClick={() => {
              onApply(pending);
              setOpen(false);
            }}
          >
            确定
          </button>
          <button
            type="button"
            className="text-sm text-muted-foreground"
            onClick={() => {
              setPending([]);
              onApply([]);
              setOpen(false);
            }}
          >
            重置
          </button>
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function OnlineSortHeader({
  sort,
  onCycle,
}: {
  sort: '' | 'ascend' | 'descend';
  onCycle: () => void;
}) {
  return (
    <button
      type="button"
      className="inline-flex items-center gap-1.5 rounded-sm outline-none transition-colors select-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
      onClick={onCycle}
    >
      <HeaderTooltip title="在线人数">人数</HeaderTooltip>
      {sort === 'ascend' ? (
        <ArrowUp className="size-3.5" />
      ) : sort === 'descend' ? (
        <ArrowDown className="size-3.5" />
      ) : (
        <ArrowUp className="size-3.5 opacity-40" />
      )}
    </button>
  );
}

function ServerManagePage() {
  const nodes = useServerNodes();
  const groups = useServerGroups();
  const routes = useServerRoutes();
  const update = useUpdateServerMutation();
  const drop = useDropServerMutation();
  const copy = useCopyServerMutation();
  const sort = useSortServerNodesMutation();
  const [searchKey, setSearchKey] = useState<string | undefined>();
  const [sortMode, setSortMode] = useState(false);
  const [onlineSort, setOnlineSort] = useState<'' | 'ascend' | 'descend'>('');
  const [typeFilter, setTypeFilter] = useState<string[]>([]);
  const [groupFilter, setGroupFilter] = useState<string[]>([]);
  const [orderedNodes, setOrderedNodes] = useState<admin.ServerNode[]>(() => nodes.data ?? []);
  const [sortingLoading, setSortingLoading] = useState(false);
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(readLegacyServerPageSize);
  const [editing, setEditing] = useState<{
    type: admin.ServerTypeName;
    record?: admin.ServerNode;
    key: number;
  } | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const orderRef = useRef(orderedNodes);

  useEffect(() => {
    if (nodes.data) {
      setOrderedNodes(nodes.data);
      setSortingLoading(false);
      setSortMode(false);
    }
  }, [nodes.data]);

  orderRef.current = orderedNodes;

  const searchedNodes =
    searchKey && orderedNodes
      ? orderedNodes.filter((node) => JSON.stringify(node).includes(searchKey))
      : orderedNodes;
  // The legacy column sorter/filters lived on the antd Table, which is hidden in
  // sort mode; reorder operates on the raw list there, so the column controls only
  // apply when browsing.
  const filteredNodes = sortMode
    ? searchedNodes
    : applyServerNodeColumnControls(searchedNodes, { typeFilter, groupFilter, onlineSort });

  const pageCount = Math.max(1, Math.ceil(filteredNodes.length / pageSize));
  const activePage = Math.min(currentPage, pageCount);
  const visibleNodes = sortMode
    ? filteredNodes
    : filteredNodes.slice((activePage - 1) * pageSize, activePage * pageSize);

  const groupName = (ids: admin.ServerNode['group_id']) =>
    ids.map((id) => groups.data?.find((group) => group.id === Number(id))?.name).filter(Boolean);

  const openEditor = (type: admin.ServerTypeName, record?: admin.ServerNode) => {
    setEditing({ type, record, key: Date.now() });
    setDrawerOpen(true);
  };

  const toggleNodeShow = (row: admin.ServerNode) => {
    const checked = parseInt(String(row.show), 10);
    update.mutate(
      {
        type: row.type as admin.ServerTypeName,
        id: row.id,
        key: 'show',
        value: checked ? 0 : 1,
      },
      {
        onSuccess: () => {
          void nodes.refetch();
        },
      },
    );
  };

  const copyNode = (row: admin.ServerNode) => {
    copy.mutate(
      { type: row.type as admin.ServerTypeName, id: row.id },
      {
        onSuccess: () => {
          void nodes.refetch();
        },
      },
    );
  };

  const removeNode = async (row: admin.ServerNode) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该节点吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    drop.mutate(
      { type: row.type as admin.ServerTypeName, id: row.id },
      {
        onSuccess: () => {
          void nodes.refetch();
        },
      },
    );
  };

  const copyHost = (host: string) => {
    void navigator.clipboard?.writeText(host);
    toast.success('复制成功');
  };

  const moveNode = (id: number, direction: -1 | 1) => {
    const list = orderRef.current;
    const index = list.findIndex((node) => node.id === id);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= list.length) return;
    setOrderedNodes(moveServerNodeByLegacyDragIndexes(list, index, target));
  };

  const cycleOnlineSort = () => {
    setCurrentPage(1);
    setOnlineSort((current) => (current === '' ? 'ascend' : current === 'ascend' ? 'descend' : ''));
  };

  const applyTypeFilter = (next: string[]) => {
    setCurrentPage(1);
    setTypeFilter(next);
  };

  const applyGroupFilter = (next: string[]) => {
    setCurrentPage(1);
    setGroupFilter(next);
  };

  const saveSort = () => {
    if (!sortMode) {
      setSortMode(true);
      return;
    }
    setSortingLoading(true);
    sort.mutate(createServerSortPayload(orderedNodes), {
      onSuccess: () => {
        void nodes.refetch();
      },
      onSettled: () => setSortingLoading(false),
    });
  };

  const changePage = (page: number, nextSize: number) => {
    setCurrentPage(page);
    if (nextSize !== pageSize) {
      setPageSize(nextSize);
      writeLegacyHabit(LEGACY_SERVER_PAGE_SIZE_KEY, nextSize);
    }
  };

  const idColumn: DataTableColumn<admin.ServerNode> = {
    id: 'node_id',
    header: () => (
      <span className="inline-flex items-center">
        节点ID
        <NodeFilterMenu
          items={NODE_TYPE_FILTERS}
          value={typeFilter}
          active={typeFilter.length > 0}
          onApply={applyTypeFilter}
        />
      </span>
    ),
    cell: ({ row }) => (
      <ServerTypeTag type={row.original.type}>
        {row.original.parent_id ? `${row.original.id} => ${row.original.parent_id}` : row.original.id}
      </ServerTypeTag>
    ),
  };

  const sortColumns: DataTableColumn<admin.ServerNode>[] = [
    {
      id: 'sort',
      meta: { align: 'center' },
      header: () => <span>排序</span>,
      cell: ({ row }) => {
        const index = orderedNodes.findIndex((node) => node.id === row.original.id);
        return (
          <div className="flex items-center justify-center gap-0.5">
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index <= 0}
              onClick={() => moveNode(row.original.id, -1)}
              aria-label="上移"
            >
              <ArrowUp className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index < 0 || index >= orderedNodes.length - 1}
              onClick={() => moveNode(row.original.id, 1)}
              aria-label="下移"
            >
              <ArrowDown className="size-4" />
            </Button>
          </div>
        );
      },
    },
    idColumn,
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>节点</span>,
      cell: ({ row }) => row.original.name,
    },
  ];

  const browseColumns: DataTableColumn<admin.ServerNode>[] = [
    idColumn,
    {
      id: 'show',
      meta: { align: 'center' },
      header: () => <span>显隐</span>,
      cell: ({ row }) => (
        <Switch
          checked={Boolean(parseInt(String(row.original.show), 10))}
          onCheckedChange={() => toggleNodeShow(row.original)}
          aria-label={`切换「${row.original.name}」显隐`}
        />
      ),
    },
    {
      id: 'node',
      meta: { className: 'font-medium text-foreground' },
      header: () => <HeaderTooltip title="节点名称">节点</HeaderTooltip>,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-2">
          <AvailabilityDot status={row.original.available_status} />
          {row.original.name}
        </span>
      ),
    },
    {
      id: 'host',
      header: () => <span>地址</span>,
      cell: ({ row }) => (
        <button
          type="button"
          className="cursor-pointer text-left tabular-nums"
          onClick={() => copyHost(row.original.host)}
        >
          {row.original.host}:{row.original.port}
        </button>
      ),
    },
    {
      id: 'online',
      header: () => <OnlineSortHeader sort={onlineSort} onCycle={cycleOnlineSort} />,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-1.5 tabular-nums">
          <User className="size-4 text-muted-foreground" /> {row.original.online || 0}
        </span>
      ),
    },
    {
      id: 'rate',
      meta: { align: 'center' },
      header: () => <HeaderTooltip title="流量倍率" className="justify-center">倍率</HeaderTooltip>,
      cell: ({ row }) => (
        <Badge variant="secondary" className="min-w-14 justify-center tabular-nums">
          {row.original.rate} x
        </Badge>
      ),
    },
    {
      id: 'group',
      header: () => (
        <span className="inline-flex items-center">
          权限组
          <NodeFilterMenu
            items={(groups.data ?? []).map((group) => ({
              text: group.name,
              value: String(group.id),
            }))}
            value={groupFilter}
            active={groupFilter.length > 0}
            onApply={applyGroupFilter}
          />
        </span>
      ),
      cell: ({ row }) => (
        <div className="flex flex-wrap gap-1">
          {groupName(row.original.group_id).map((name) => (
            <Badge key={name} variant="secondary">
              {name}
            </Badge>
          ))}
        </div>
      ),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm" data-testid={`node-actions-${row.original.id}`}>
              操作
              <ChevronDown className="size-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem
              onClick={() => openEditor(row.original.type as admin.ServerTypeName, row.original)}
              data-testid={`node-edit-${row.original.id}`}
            >
              <Pencil className="size-4" />
              编辑
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => copyNode(row.original)}
              data-testid={`node-copy-${row.original.id}`}
            >
              <Copy className="size-4" />
              复制
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              variant="destructive"
              onClick={() => void removeNode(row.original)}
              data-testid={`node-delete-${row.original.id}`}
            >
              <Trash2 className="size-4" />
              删除
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ),
    },
  ];

  const columns = sortMode ? sortColumns : browseColumns;
  const emptyText = filteredNodes.length === 0 ? '暂无节点' : undefined;

  return (
    <PageShell data-testid="server-manage-page">
      <ServerSortPrompt when={sortMode} />
      <PageHeader
        title="节点管理"
        actions={
          <>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button data-testid="node-add">
                  <Plus className="size-4" />
                  添加节点
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                {SERVER_TYPES.map((type) => (
                  <DropdownMenuItem
                    key={type}
                    onClick={() => openEditor(type)}
                    data-testid={`node-add-${type}`}
                  >
                    <ServerTypeTag type={type}>{SERVER_TYPE_LABELS[type]}</ServerTypeTag>
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
            <Button
              variant={sortMode ? 'default' : 'outline'}
              onClick={saveSort}
              data-testid="node-sort-toggle"
            >
              {sortMode ? '保存排序' : '编辑排序'}
            </Button>
          </>
        }
      />

      <div className="w-full sm:max-w-xs">
        <Input
          placeholder="输入任意关键字搜索"
          onChange={(event) => {
            setSearchKey(event.target.value);
            setCurrentPage(1);
          }}
          data-testid="node-search"
        />
      </div>

      <TooltipProvider delayDuration={100}>
        <Card className="overflow-hidden py-0">
          <CardContent className="p-0">
            <DataTable
              columns={columns}
              data={visibleNodes}
              getRowKey={(row) => row.id}
              className="min-w-[1080px]"
              data-testid="server-nodes-table"
              empty={emptyText}
              emptyTestId="server-nodes-empty"
              virtualizer={{ enabled: visibleNodes.length > VIRTUALIZE_MIN_ROWS }}
            />
          </CardContent>
        </Card>
      </TooltipProvider>

      {!sortMode && filteredNodes.length > 0 ? (
        <ServerPagination
          current={activePage}
          pageSize={pageSize}
          total={filteredNodes.length}
          onChange={changePage}
        />
      ) : null}

      {nodes.isFetching || sortingLoading ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}

      <NodeEditDrawer
        open={drawerOpen}
        editKey={editing?.key ?? 0}
        type={editing?.type ?? 'v2node'}
        record={editing?.record}
        nodes={nodes.data ?? []}
        groups={groups.data ?? []}
        routes={routes.data ?? []}
        onSaved={() => nodes.refetch()}
        onClose={() => setDrawerOpen(false)}
      />
    </PageShell>
  );
}

const SERVER_PAGE_SIZE_OPTIONS = [10, 50, 100, 500];

function ServerPagination({
  current,
  pageSize,
  total,
  onChange,
}: {
  current: number;
  pageSize: number;
  total: number;
  onChange: (page: number, pageSize: number) => void;
}) {
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  return (
    <div className="flex flex-wrap items-center justify-end gap-3">
      <span className="text-sm text-muted-foreground">共 {total} 条</span>
      <Select
        value={String(pageSize)}
        onValueChange={(value) => onChange(1, Number(value))}
      >
        <SelectTrigger className="h-9 w-28" data-testid="node-page-size">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SERVER_PAGE_SIZE_OPTIONS.map((size) => (
            <SelectItem key={size} value={String(size)}>
              {size} 条/页
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <div className="flex items-center gap-1">
        <Button
          variant="outline"
          size="sm"
          disabled={current <= 1}
          onClick={() => onChange(current - 1, pageSize)}
        >
          上一页
        </Button>
        <span className="px-2 text-sm tabular-nums" data-testid="node-page">
          {current} / {pageCount}
        </span>
        <Button
          variant="outline"
          size="sm"
          disabled={current >= pageCount}
          onClick={() => onChange(current + 1, pageSize)}
        >
          下一页
        </Button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Node editor drawer.
// ---------------------------------------------------------------------------

function NodeEditDrawer({
  open,
  editKey,
  type,
  record,
  nodes,
  groups,
  routes,
  onSaved,
  onClose,
}: {
  open: boolean;
  editKey: number;
  type: admin.ServerTypeName;
  record?: admin.ServerNode;
  nodes: admin.ServerNode[];
  groups: admin.ServerGroup[];
  routes: admin.ServerRoute[];
  onSaved?: () => void | Promise<unknown>;
  onClose: () => void;
}) {
  const id = record?.id;
  const [values, setValues] = useState<Record<string, unknown>>(() =>
    getNodeInitialValues(type, record),
  );
  const [saving, setSaving] = useState(false);
  const [childDrawer, setChildDrawer] = useState<{ open: boolean; title?: string; field?: string }>(
    { open: false },
  );

  // Reseed the form to the exact legacy initial values every time the drawer is
  // (re)opened for a node/type, mirroring antd's fresh Form initialValues.
  useEffect(() => {
    if (open) {
      setValues(getNodeInitialValues(type, record));
      setChildDrawer({ open: false });
    }
    // editKey changes on every open; type/record are captured with it.
  }, [editKey, open]);

  const setField = (name: string | [string, string], value: unknown) => {
    setValues((prev) => {
      if (Array.isArray(name)) {
        const [parent, child] = name;
        const parentValue =
          prev[parent] && typeof prev[parent] === 'object'
            ? { ...(prev[parent] as Record<string, unknown>) }
            : {};
        parentValue[child] = value;
        return { ...prev, [parent]: parentValue };
      }
      return { ...prev, [name]: value };
    });
  };
  const setFields = (partial: Record<string, unknown>) =>
    setValues((prev) => ({ ...prev, ...partial }));
  const form: NodeForm = { values, setField, setFields, setValues };

  const parentCandidates = nodes.filter((node) => node.type === type && node.id !== id);
  const parentOptions: SelectOption[] = [
    { value: '', label: '无' },
    ...parentCandidates.map((node) => ({ value: node.id, label: node.name })),
  ];
  const groupOptions = groups.map((group) => ({ value: String(group.id), label: group.name }));
  const routeOptions = routes.map((route) => ({
    value: String(route.id),
    label: String(route.id),
  }));

  const showChildDrawer = (title?: string, field?: string) => {
    setChildDrawer((current) => ({ open: !current.open, title, field }));
  };

  const submit = async () => {
    setSaving(true);
    try {
      const payload = prepareLegacyServerPayload(type, values, id);
      await admin.saveServer(apiClient, type, payload);
      await onSaved?.();
      onClose();
    } catch (e) {
      // Client-side payload validation stays inline; backend API errors are
      // surfaced by the global onError handler (legacy parity).
      if (e instanceof SyntaxError) {
        // Legacy parity: invalid transport-config JSON surfaced an error toast.
        toast.error('传输协议配置格式有误');
      }
    } finally {
      setSaving(false);
    }
  };

  const selectedGroups = Array.isArray(values.group_id)
    ? (values.group_id as unknown[]).map(String)
    : [];
  const selectedRoutes = Array.isArray(values.route_id)
    ? (values.route_id as unknown[]).map(String)
    : [];
  const tags = Array.isArray(values.tags) ? (values.tags as string[]) : [];

  return (
    <Sheet open={open} onOpenChange={(next) => (next ? undefined : onClose())}>
      <SheetContent
        side="right"
        className="w-full gap-0 overflow-y-auto sm:max-w-3xl"
        data-testid="node-editor"
      >
        <SheetHeader>
          <SheetTitle>{id ? '编辑节点' : '新建节点'}</SheetTitle>
        </SheetHeader>

        <div className="space-y-5 px-4 pb-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div className="space-y-2 sm:col-span-2">
              <Label htmlFor="node-name">节点名称</Label>
              <Input
                id="node-name"
                placeholder="请输入节点名称"
                value={inputValue(values.name)}
                onChange={(event) => setField('name', event.target.value)}
                data-testid="node-name"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="node-rate">倍率</Label>
              <div className="relative">
                <Input
                  id="node-rate"
                  className="pr-8"
                  placeholder="请输入节点倍率"
                  value={inputValue(values.rate)}
                  onChange={(event) => setField('rate', event.target.value)}
                  data-testid="node-rate"
                />
                <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                  x
                </span>
              </div>
            </div>
          </div>

          <div className="space-y-2">
            <Label>节点标签</Label>
            <TagsInput
              value={tags}
              onChange={(next) => setField('tags', normalizeLegacyNullableArray(next))}
              placeholder="输入后回车添加标签"
              testId="node-tags"
            />
          </div>

          <div className="space-y-2">
            <Label>权限组</Label>
            <MultiCheckboxField
              options={groupOptions}
              value={selectedGroups}
              onChange={(next) => setField('group_id', next)}
              testId="node-group-ids"
              emptyText="暂无可选权限组"
            />
          </div>

          <NodeAddressFields form={form} type={type} showChildDrawer={showChildDrawer} />
          <NodePortFields form={form} type={type} />

          <ServerTypeFields
            editing={Boolean(id)}
            type={type}
            form={form}
            showChildDrawer={showChildDrawer}
          />

          <div className="space-y-2">
            <Label className="flex items-center gap-2">
              父节点
              <a
                className="inline-flex items-center gap-1 text-sm text-primary"
                target="_blank"
                href="https://docs.v2board.com/use/node.html#父节点与子节点关系"
                rel="noopener noreferrer"
              >
                <ExternalLink className="size-3.5" />
                更多解答
              </a>
            </Label>
            <NodeSelect
              value={(values.parent_id as SelectValueType) || ''}
              options={parentOptions}
              onChange={(value) => setField('parent_id', value)}
              testId="node-parent"
            />
          </div>

          <div className="space-y-2">
            <Label>路由组</Label>
            <MultiCheckboxField
              options={routeOptions}
              value={selectedRoutes}
              onChange={(next) =>
                setField('route_id', normalizeLegacyNullableArray(next.map(Number)))
              }
              testId="node-route-ids"
              emptyText="暂无可选路由组"
            />
          </div>

          {type === 'v2node' ? (
            <div className="space-y-2">
              <Label htmlFor="node-install-command">一键安装指令</Label>
              <Textarea
                id="node-install-command"
                rows={4}
                readOnly
                className="cursor-text bg-muted/40 font-mono text-xs"
                value={inputValue(values.install_command)}
                data-testid="node-install-command"
              />
            </div>
          ) : null}
        </div>

        <SheetFooter>
          <Button onClick={() => void submit()} disabled={saving} data-testid="node-submit">
            {saving ? <Loader2 className="size-4 animate-spin" /> : null}
            提交
          </Button>
          <Button variant="outline" onClick={onClose}>
            取消
          </Button>
        </SheetFooter>
      </SheetContent>

      {childDrawer.field ? (
        <Sheet open={childDrawer.open} onOpenChange={(next) => (next ? undefined : showChildDrawer())}>
          <SheetContent
            side="right"
            className="w-full gap-0 overflow-y-auto sm:max-w-2xl"
            data-testid="node-child-editor"
          >
            <SheetHeader>
              <SheetTitle>{childDrawer.title}</SheetTitle>
            </SheetHeader>
            <div className="space-y-4 px-4 pb-4">
              <NodeChildField type={type} field={childDrawer.field} form={form} />
            </div>
            <SheetFooter>
              <Button onClick={() => showChildDrawer()}>完成</Button>
            </SheetFooter>
          </SheetContent>
        </Sheet>
      ) : null}
    </Sheet>
  );
}

function NodeAddressFields({
  form,
  type,
  showChildDrawer,
}: {
  form: NodeForm;
  type: admin.ServerTypeName;
  showChildDrawer: (title?: string, field?: string) => void;
}) {
  const { values, setField } = form;
  if (type === 'v2node') {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className="space-y-2">
          <Label htmlFor="node-host">连接地址</Label>
          <Input
            id="node-host"
            placeholder="地址或IP"
            value={inputValue(values.host)}
            onChange={(event) => setField('host', event.target.value)}
            data-testid="node-host"
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="node-listen-ip">监听地址</Label>
          <Input
            id="node-listen-ip"
            placeholder="地址或IP默认为0.0.0.0"
            value={inputValue(values.listen_ip)}
            onChange={(event) => setField('listen_ip', event.target.value)}
            data-testid="node-listen-ip"
          />
        </div>
      </div>
    );
  }
  if (type === 'vmess' || type === 'vless') {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <div className="space-y-2 sm:col-span-2">
          <Label htmlFor="node-host">节点地址</Label>
          <Input
            id="node-host"
            placeholder="请输入连接地址"
            value={inputValue(values.host)}
            onChange={(event) => setField('host', event.target.value)}
            data-testid="node-host"
          />
        </div>
        {type === 'vmess' ? (
          <VmessTlsField form={form} showChildDrawer={showChildDrawer} />
        ) : (
          <VlessSecurityField form={form} showChildDrawer={showChildDrawer} />
        )}
      </div>
    );
  }
  return (
    <div className="space-y-2">
      <Label htmlFor="node-host">节点地址</Label>
      <Input
        id="node-host"
        placeholder="地址或IP"
        value={inputValue(values.host)}
        onChange={(event) => setField('host', event.target.value)}
        data-testid="node-host"
      />
    </div>
  );
}

function NodePortFields({ form, type }: { form: NodeForm; type: admin.ServerTypeName }) {
  const { values, setField } = form;
  const portInput = (name: string, label: string, placeholder: string, testId: string) => (
    <div className="space-y-2">
      <Label htmlFor={testId}>{label}</Label>
      <Input
        id={testId}
        placeholder={placeholder}
        value={inputValue(values[name])}
        onChange={(event) => setField(name, event.target.value)}
        data-testid={testId}
      />
    </div>
  );

  if (type === 'trojan' || type === 'hysteria' || type === 'tuic' || type === 'anytls') {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        {portInput('port', '连接端口', '用户连接端口', 'node-port')}
        {portInput('server_port', '服务端口', '服务端开放端口', 'node-server-port')}
        {type === 'trojan' ? <TrojanAllowInsecureField form={form} /> : <ServerInsecureField form={form} />}
      </div>
    );
  }
  if (type === 'v2node') {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        {portInput('port', '连接端口', '用户连接端口', 'node-port')}
        {portInput('server_port', '服务端口', '服务端开放端口', 'node-server-port')}
      </div>
    );
  }
  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
      {portInput('port', '连接端口', '用户连接端口', 'node-port')}
      {portInput('server_port', '服务端口', '非NAT同连接端口', 'node-server-port')}
    </div>
  );
}

function ChildFieldLink({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button type="button" className="text-sm text-primary" onClick={onClick}>
      {label}
    </button>
  );
}

function TrojanAllowInsecureField({ form }: { form: NodeForm }) {
  return (
    <div className="space-y-2">
      <Label>
        <HeaderTooltip title="使用自签名证书需要允许不安全，用户才可以连接">允许不安全</HeaderTooltip>
      </Label>
      <NodeSelect
        value={getLegacyBinarySelectValue(form.values.allow_insecure)}
        options={LEGACY_BINARY_SELECT_OPTIONS}
        placeholder="允许不安全"
        onChange={(value) => form.setField('allow_insecure', value)}
        testId="node-allow-insecure"
      />
    </div>
  );
}

function ServerInsecureField({ form }: { form: NodeForm }) {
  return (
    <div className="space-y-2">
      <Label>
        <HeaderTooltip title="使用自签名证书需要允许不安全，用户才可以连接">允许不安全</HeaderTooltip>
      </Label>
      <NodeSelect
        value={getLegacyBinarySelectValue(form.values.insecure)}
        options={LEGACY_BINARY_SELECT_OPTIONS}
        placeholder="允许不安全"
        onChange={(value) => form.setField('insecure', value)}
        testId="node-insecure"
      />
    </div>
  );
}

function VmessTlsField({
  form,
  showChildDrawer,
}: {
  form: NodeForm;
  showChildDrawer: (title?: string, field?: string) => void;
}) {
  return (
    <div className="space-y-2">
      <Label className="flex items-center gap-2">
        TLS
        <ChildFieldLink label="编辑配置" onClick={() => showChildDrawer('编辑TLS配置', 'tlsSettings')} />
      </Label>
      <NodeSelect
        value={getLegacyBinarySelectValue(form.values.tls)}
        options={LEGACY_TLS_SUPPORT_OPTIONS}
        placeholder="是否支持TLS"
        onChange={(value) => form.setField('tls', value)}
        testId="node-tls"
      />
    </div>
  );
}

function VlessSecurityField({
  form,
  showChildDrawer,
}: {
  form: NodeForm;
  showChildDrawer: (title?: string, field?: string) => void;
}) {
  const security = form.values.tls;
  return (
    <div className="space-y-2">
      <Label className="flex items-center gap-2">
        安全性
        {parseInt(String(security ?? 0), 10) !== 0 ? (
          <ChildFieldLink
            label="编辑配置"
            onClick={() => showChildDrawer('编辑安全性配置', 'tls_settings')}
          />
        ) : null}
      </Label>
      <NodeSelect
        value={getLegacyNumericSelectValue(form.values.tls)}
        options={[LEGACY_SECURITY_NONE_OPTION, LEGACY_SECURITY_TLS_OPTION, LEGACY_SECURITY_REALITY_OPTION]}
        onChange={(value) => form.setField('tls', value)}
        testId="node-vless-security"
      />
    </div>
  );
}

function V2nodeFields({
  form,
  showChildDrawer,
}: {
  form: NodeForm;
  showChildDrawer: (title?: string, field?: string) => void;
}) {
  const { values, setField, setFields, setValues } = form;
  const protocol = values.protocol;
  const tls = values.tls;
  const obfs = values.obfs;
  const encryption = values.encryption;
  const protocolValue = protocol == null ? null : String(protocol);
  const securityValue = getLegacyV2nodeSecurityValue(protocolValue, tls);

  // Mirror antd Form.Item initialValue registering on mount when a protocol's
  // fields appear: seed the item-level defaults getLegacyServerInitialValues
  // intentionally omits, only when the store has no value yet.
  useEffect(() => {
    setValues((prev) => {
      const next = { ...prev };
      let changed = false;
      if (protocolValue === 'shadowsocks' && next.cipher == null) {
        next.cipher = 'aes-128-gcm';
        changed = true;
      }
      if (protocolValue === 'tuic') {
        if (next.udp_relay_mode == null) {
          next.udp_relay_mode = 'native';
          changed = true;
        }
        if (next.congestion_control == null) {
          next.congestion_control = 'cubic';
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [protocolValue, setValues]);

  const changeProtocol = (value: string | number | null) => {
    const nextProtocol = value == null ? '' : String(value);
    setFields({
      protocol: nextProtocol,
      ...(LEGACY_TLS_FORCED_PROTOCOLS.includes(nextProtocol) ? { tls: 1 } : {}),
    });
  };

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className="space-y-2">
          <Label>节点协议</Label>
          <NodeSelect
            value={protocolValue}
            options={LEGACY_V2NODE_PROTOCOL_OPTIONS}
            onChange={changeProtocol}
            testId="node-protocol"
          />
        </div>
        {protocolValue != null && protocolValue !== 'shadowsocks' ? (
          <div className="space-y-2">
            <Label className="flex items-center gap-2">
              安全性
              {securityValue ? (
                <ChildFieldLink
                  label="编辑配置"
                  onClick={() => showChildDrawer('编辑安全性配置', 'tls_settings')}
                />
              ) : null}
            </Label>
            <NodeSelect
              value={getLegacyV2nodeSecurityValue(protocolValue, tls)}
              options={getLegacyV2nodeSecurityOptions(protocolValue)}
              onChange={(value) => setField('tls', value)}
              testId="node-v2node-security"
            />
          </div>
        ) : null}
      </div>

      {protocolValue === 'shadowsocks' ? (
        <div className="space-y-2">
          <Label className="flex items-center gap-2">
            传输协议
            <ChildFieldLink
              label="编辑配置"
              onClick={() => showChildDrawer('编辑协议配置', 'network_settings')}
            />
          </Label>
          <NodeSelect
            value={(values.network as SelectValueType) ?? 'tcp'}
            options={LEGACY_V2NODE_SHADOWSOCKS_NETWORK_OPTIONS}
            placeholder="选择传输协议"
            onChange={(value) => setField('network', value)}
            testId="node-v2node-network"
          />
        </div>
      ) : null}

      {protocolValue != null &&
      protocolValue !== 'hysteria2' &&
      protocolValue !== 'shadowsocks' &&
      protocolValue !== 'tuic' ? (
        <div className="space-y-2">
          <Label className="flex items-center gap-2">
            传输协议
            <ChildFieldLink
              label="编辑配置"
              onClick={() => showChildDrawer('编辑协议配置', 'network_settings')}
            />
          </Label>
          <NodeSelect
            value={(values.network as SelectValueType) ?? 'tcp'}
            options={getLegacyV2nodeTransportOptions(protocolValue)}
            placeholder="选择传输协议"
            onChange={(value) => setField('network', value)}
            testId="node-v2node-network"
          />
        </div>
      ) : null}

      {protocolValue === 'anytls' ? (
        <div>
          <ChildFieldLink
            label="编辑填充方案"
            onClick={() => showChildDrawer('编辑填充方案', 'padding_scheme')}
          />
        </div>
      ) : null}

      {protocolValue === 'hysteria2' ? (
        <>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label>混淆方式obfs</Label>
              <NodeSelect
                value={(values.obfs as SelectValueType) ?? null}
                options={LEGACY_HYSTERIA2_OBFS_OPTIONS}
                onChange={(value) => setField('obfs', value)}
                testId="node-obfs"
              />
            </div>
            {obfs === 'salamander' ? (
              <div className="space-y-2">
                <Label htmlFor="node-obfs-password">混淆密码obfs_password</Label>
                <Input
                  id="node-obfs-password"
                  placeholder="留空自动生成"
                  value={inputValue(values.obfs_password)}
                  onChange={(event) => setField('obfs_password', event.target.value)}
                />
              </div>
            ) : null}
          </div>
          <BandwidthFields form={form} />
        </>
      ) : null}

      {protocolValue === 'tuic' ? (
        <>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label>禁用SNI</Label>
              <NodeSelect
                value={getLegacyBinarySelectValue(values.disable_sni)}
                options={LEGACY_BINARY_SELECT_OPTIONS}
                onChange={(value) => setField('disable_sni', value)}
                testId="node-disable-sni"
              />
            </div>
            <div className="space-y-2">
              <Label>数据包中继模式</Label>
              <NodeSelect
                value={(values.udp_relay_mode as SelectValueType) ?? 'native'}
                options={LEGACY_TUIC_RELAY_MODE_OPTIONS}
                onChange={(value) => setField('udp_relay_mode', value)}
                testId="node-udp-relay-mode"
              />
            </div>
          </div>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label>拥塞控制算法</Label>
              <NodeSelect
                value={(values.congestion_control as SelectValueType) ?? 'cubic'}
                options={LEGACY_TUIC_CONGESTION_CONTROL_OPTIONS}
                onChange={(value) => setField('congestion_control', value)}
                testId="node-congestion-control"
              />
            </div>
            <div className="space-y-2">
              <Label>客户端启用 0-RTT</Label>
              <NodeSelect
                value={getLegacyBinarySelectValue(values.zero_rtt_handshake)}
                options={LEGACY_BINARY_SELECT_OPTIONS}
                onChange={(value) => setField('zero_rtt_handshake', value)}
                testId="node-zero-rtt"
              />
            </div>
          </div>
        </>
      ) : null}

      {protocolValue === 'shadowsocks' ? (
        <div className="space-y-2">
          <Label>加密算法</Label>
          <NodeSelect
            value={(values.cipher as SelectValueType) ?? 'aes-128-gcm'}
            options={LEGACY_SHADOWSOCKS_CIPHER_OPTIONS}
            onChange={(value) => setField('cipher', value)}
            testId="node-cipher"
          />
        </div>
      ) : null}

      {protocolValue === 'vless' ? (
        <>
          <div className="space-y-2">
            <Label className="flex items-center gap-2">
              加密方式
              {encryption ? (
                <ChildFieldLink
                  label="编辑配置"
                  onClick={() => showChildDrawer('编辑加密配置', 'encryption_settings')}
                />
              ) : null}
            </Label>
            <NodeSelect
              value={(values.encryption as SelectValueType) ?? null}
              options={LEGACY_VLESS_ENCRYPTION_OPTIONS}
              placeholder="选择加密方式"
              onChange={(value) => setField('encryption', value)}
              testId="node-encryption"
            />
          </div>
          <div className="space-y-2">
            <Label>XTLS流控算法</Label>
            <NodeSelect
              value={(values.flow as SelectValueType) ?? null}
              options={LEGACY_VLESS_FLOW_OPTIONS}
              placeholder="选择XTLS流控算法"
              onChange={(value) => setField('flow', value)}
              testId="node-flow"
            />
          </div>
        </>
      ) : null}
    </div>
  );
}

function BandwidthFields({ form }: { form: NodeForm }) {
  const { values, setField } = form;
  const field = (name: string, label: string, placeholder: string) => (
    <div className="space-y-2">
      <Label htmlFor={`node-${name}`}>{label}</Label>
      <div className="relative">
        <Input
          id={`node-${name}`}
          className="pr-16"
          placeholder={placeholder}
          value={inputValue(values[name])}
          onChange={(event) => setField(name, event.target.value)}
        />
        <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
          Mbps
        </span>
      </div>
    </div>
  );
  return (
    <>
      {field('up_mbps', '上行带宽', '服务端发送带宽,留空或填0使用BBR')}
      {field('down_mbps', '下行带宽', '服务端接收带宽,留空或填0使用BBR')}
    </>
  );
}

function ServerTypeFields({
  editing,
  type,
  form,
  showChildDrawer,
}: {
  editing: boolean;
  type: admin.ServerTypeName;
  form: NodeForm;
  showChildDrawer: (title?: string, field?: string) => void;
}) {
  const { values, setField } = form;

  if (type === 'v2node') {
    return <V2nodeFields form={form} showChildDrawer={showChildDrawer} />;
  }

  if (type === 'shadowsocks') {
    const shadowsocksObfs = values.obfs;
    const obfsSettings = (values.obfs_settings as Record<string, unknown> | undefined) ?? {};
    return (
      <div className="space-y-5">
        <div className="space-y-2">
          <Label>加密算法</Label>
          <NodeSelect
            value={(values.cipher as SelectValueType) ?? (editing ? undefined : 'chacha20-ietf-poly1305')}
            options={LEGACY_SHADOWSOCKS_CIPHER_OPTIONS}
            onChange={(value) => setField('cipher', value)}
            testId="node-cipher"
          />
        </div>
        <div className="space-y-2">
          <Label>混淆</Label>
          <NodeSelect
            value={(values.obfs as SelectValueType) ?? ''}
            options={LEGACY_SHADOWSOCKS_OBFS_OPTIONS}
            onChange={(value) => setField('obfs', value)}
            testId="node-obfs"
          />
          {shadowsocksObfs === 'http' ? (
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              <div className="space-y-2">
                <Input
                  placeholder="路径"
                  value={inputValue(obfsSettings.path)}
                  onChange={(event) => setField(['obfs_settings', 'path'], event.target.value)}
                  data-testid="node-obfs-path"
                />
              </div>
              <div className="space-y-2 sm:col-span-2">
                <Input
                  placeholder="Host"
                  value={inputValue(obfsSettings.host)}
                  onChange={(event) => setField(['obfs_settings', 'host'], event.target.value)}
                  data-testid="node-obfs-host"
                />
              </div>
            </div>
          ) : null}
        </div>
      </div>
    );
  }

  if (type === 'vmess') {
    return (
      <div className="space-y-2">
        <Label className="flex items-center gap-2">
          传输协议
          <ChildFieldLink
            label="编辑配置"
            onClick={() => showChildDrawer('编辑协议配置', 'networkSettings')}
          />
        </Label>
        <NodeSelect
          value={values.network as SelectValueType}
          options={LEGACY_STREAM_NETWORK_OPTIONS}
          placeholder="选择传输协议"
          onChange={(value) => setField('network', value)}
          testId="node-network"
        />
      </div>
    );
  }

  if (type === 'trojan') {
    return (
      <div className="space-y-5">
        <div className="space-y-2">
          <Label htmlFor="node-server-name">服务器名称指示(sni)</Label>
          <Input
            id="node-server-name"
            placeholder="当节点地址与证书不一致时用于证书验证"
            value={inputValue(values.server_name)}
            onChange={(event) => setField('server_name', event.target.value)}
            data-testid="node-server-name"
          />
        </div>
        <div className="space-y-2">
          <Label className="flex items-center gap-2">
            传输协议
            <ChildFieldLink
              label="编辑配置"
              onClick={() => showChildDrawer('编辑协议配置', 'network_settings')}
            />
          </Label>
          <NodeSelect
            value={values.network as SelectValueType}
            options={LEGACY_TROJAN_NETWORK_OPTIONS}
            placeholder="选择传输协议"
            onChange={(value) => setField('network', value)}
            testId="node-network"
          />
        </div>
      </div>
    );
  }

  if (type === 'tuic') {
    const tuicDisableSni = values.disable_sni;
    return (
      <div className="space-y-5">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <Label>禁用SNI</Label>
            <NodeSelect
              value={getLegacyBinarySelectValue(values.disable_sni)}
              options={LEGACY_BINARY_SELECT_OPTIONS}
              onChange={(value) => setField('disable_sni', value)}
              testId="node-disable-sni"
            />
          </div>
          <div className="space-y-2">
            <Label>数据包中继模式</Label>
            <NodeSelect
              value={(values.udp_relay_mode as SelectValueType) ?? 'native'}
              options={LEGACY_TUIC_RELAY_MODE_OPTIONS}
              onChange={(value) => setField('udp_relay_mode', value)}
              testId="node-udp-relay-mode"
            />
          </div>
        </div>
        {parseInt(String(tuicDisableSni ?? 0), 10) ? null : (
          <div className="space-y-2">
            <Label htmlFor="node-server-name">服务器名称指示(sni)</Label>
            <Input
              id="node-server-name"
              placeholder="当节点地址与证书不一致时用于证书验证"
              value={inputValue(values.server_name)}
              onChange={(event) => setField('server_name', event.target.value)}
              data-testid="node-server-name"
            />
          </div>
        )}
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <Label>拥塞控制算法</Label>
            <NodeSelect
              value={(values.congestion_control as SelectValueType) ?? 'cubic'}
              options={LEGACY_TUIC_CONGESTION_CONTROL_OPTIONS}
              onChange={(value) => setField('congestion_control', value)}
              testId="node-congestion-control"
            />
          </div>
          <div className="space-y-2">
            <Label>客户端启用 0-RTT</Label>
            <NodeSelect
              value={getLegacyBinarySelectValue(values.zero_rtt_handshake)}
              options={LEGACY_BINARY_SELECT_OPTIONS}
              onChange={(value) => setField('zero_rtt_handshake', value)}
              testId="node-zero-rtt"
            />
          </div>
        </div>
      </div>
    );
  }

  if (type === 'vless') {
    const vlessNetwork = values.network;
    const vlessEncryption = values.encryption;
    return (
      <div className="space-y-5">
        <div className="space-y-2">
          <Label className="flex items-center gap-2">
            传输协议
            <ChildFieldLink
              label="编辑配置"
              onClick={() => showChildDrawer('编辑协议配置', 'network_settings')}
            />
          </Label>
          <NodeSelect
            value={values.network as SelectValueType}
            options={LEGACY_STREAM_NETWORK_OPTIONS}
            placeholder="选择传输协议"
            onChange={(value) => setField('network', value)}
            testId="node-network"
          />
        </div>
        <div className="space-y-2">
          <Label className="flex items-center gap-2">
            加密方式
            {vlessEncryption ? (
              <ChildFieldLink
                label="编辑配置"
                onClick={() => showChildDrawer('编辑加密配置', 'encryption_settings')}
              />
            ) : null}
          </Label>
          <NodeSelect
            value={(values.encryption as SelectValueType) ?? null}
            options={LEGACY_VLESS_ENCRYPTION_OPTIONS}
            placeholder="选择加密方式"
            onChange={(value) => setField('encryption', value)}
            testId="node-encryption"
          />
        </div>
        <div className="space-y-2">
          <Label>XTLS流控算法</Label>
          <NodeSelect
            value={(values.flow as SelectValueType) ?? null}
            options={getLegacyVlessFlowOptions(vlessNetwork)}
            placeholder="选择XTLS流控算法"
            onChange={(value) => setField('flow', value)}
            testId="node-flow"
          />
        </div>
      </div>
    );
  }

  if (type === 'hysteria') {
    const version = parseInt(String(values.version ?? 1), 10);
    const obfs = values.obfs == null ? null : String(values.obfs);
    return (
      <div className="space-y-5">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-4">
          <div className="space-y-2">
            <Label>HYSTERIA版本</Label>
            <NodeSelect
              value={getLegacyNumericSelectValue(values.version, 1)}
              options={LEGACY_HYSTERIA_VERSION_OPTIONS}
              onChange={(value) => setField('version', value)}
              testId="node-version"
            />
          </div>
        </div>
        <div className="space-y-2">
          <Label htmlFor="node-server-name">服务器名称指示(sni)</Label>
          <Input
            id="node-server-name"
            placeholder="当节点地址与证书不一致时用于证书验证"
            value={inputValue(values.server_name)}
            onChange={(event) => setField('server_name', event.target.value)}
            data-testid="node-server-name"
          />
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          {version === 1 ? (
            <div className="space-y-2">
              <Label>混淆方式obfs</Label>
              <NodeSelect
                value={(values.obfs as SelectValueType) ?? null}
                options={LEGACY_HYSTERIA_V1_OBFS_OPTIONS}
                onChange={(value) => setField('obfs', value)}
                testId="node-obfs"
              />
            </div>
          ) : null}
          {version === 1 && obfs === 'xplus' ? (
            <div className="space-y-2">
              <Label htmlFor="node-obfs-password">混淆密码obfsParam</Label>
              <Input
                id="node-obfs-password"
                placeholder="留空自动生成"
                value={inputValue(values.obfs_password)}
                onChange={(event) => setField('obfs_password', event.target.value)}
              />
            </div>
          ) : null}
          {version === 2 ? (
            <div className="space-y-2">
              <Label>混淆方式obfs</Label>
              <NodeSelect
                value={(values.obfs as SelectValueType) ?? null}
                options={LEGACY_HYSTERIA2_OBFS_OPTIONS}
                onChange={(value) => setField('obfs', value)}
                testId="node-obfs"
              />
            </div>
          ) : null}
          {version === 2 && obfs === 'salamander' ? (
            <div className="space-y-2">
              <Label htmlFor="node-obfs-password">混淆密码obfs_password</Label>
              <Input
                id="node-obfs-password"
                placeholder="留空自动生成"
                value={inputValue(values.obfs_password)}
                onChange={(event) => setField('obfs_password', event.target.value)}
              />
            </div>
          ) : null}
        </div>
        <BandwidthFields form={form} />
      </div>
    );
  }

  if (type === 'anytls') {
    return (
      <div className="space-y-5">
        <div className="space-y-2">
          <Label htmlFor="node-server-name">服务器名称指示(sni)</Label>
          <Input
            id="node-server-name"
            placeholder="当节点地址与证书不一致时用于证书验证"
            value={inputValue(values.server_name)}
            onChange={(event) => setField('server_name', event.target.value)}
            data-testid="node-server-name"
          />
        </div>
        <div>
          <ChildFieldLink
            label="编辑填充方案"
            onClick={() => showChildDrawer('编辑填充方案', 'padding_scheme')}
          />
        </div>
      </div>
    );
  }

  return null;
}

// ---------------------------------------------------------------------------
// Child config drawers.
// ---------------------------------------------------------------------------

function NodeChildField({
  type,
  field,
  form,
}: {
  type: admin.ServerTypeName;
  field: string;
  form: NodeForm;
}) {
  const { values, setField } = form;

  if (field === 'network_settings' || field === 'networkSettings') {
    return (
      <div className="space-y-2">
        <Label className="flex items-center gap-2">
          协议详细配置
          <a
            className="inline-flex items-center gap-1 text-sm text-primary"
            href="https://www.v2ray.com/chapter_02/05_transport.html"
            target="_blank"
            rel="noopener noreferrer"
          >
            <ExternalLink className="size-3.5" />
            参考
          </a>
        </Label>
        <Textarea
          rows={12}
          className="font-mono text-xs"
          placeholder={getLegacyNetworkSettingsPlaceholder(type, values.network)}
          value={inputValue(values[field])}
          onChange={(event) => setField(field, event.target.value)}
          data-testid="node-network-settings"
        />
      </div>
    );
  }

  if (field === 'padding_scheme') {
    return (
      <div className="space-y-2">
        <Textarea
          rows={12}
          className="font-mono text-xs"
          placeholder={ANYTLS_PADDING_SCHEME_PLACEHOLDER}
          value={inputValue(values.padding_scheme)}
          onChange={(event) => setField('padding_scheme', event.target.value)}
          data-testid="node-padding-scheme"
        />
      </div>
    );
  }

  if (field === 'tls_settings' || field === 'tlsSettings') {
    return <TlsSettingsField field={field} form={form} certApply={field === 'tls_settings'} />;
  }

  if (field === 'encryption_settings') {
    return <EncryptionSettingsField form={form} />;
  }

  return (
    <Textarea
      rows={12}
      className="font-mono text-xs"
      value={inputValue(values[field])}
      onChange={(event) => setField(field, event.target.value)}
    />
  );
}

function TlsSettingsField({
  field,
  form,
  certApply,
}: {
  field: string;
  form: NodeForm;
  certApply: boolean;
}) {
  const settings = form.values[field];
  const tls = form.values.tls;
  const value = normalizeLegacySettings(settings, LEGACY_TLS_SETTINGS_DEFAULTS);
  const tlsValue = parseInt(String(tls ?? 0), 10);
  const change = (key: string, next: unknown) => {
    form.setFields({ [field]: { ...value, [key]: next } });
  };

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label>Server Name(SNI)</Label>
        <Input
          value={legacyText(value.server_name)}
          onChange={(event) => change('server_name', event.target.value)}
          placeholder={tlsValue === 2 ? 'REALITY必填，与后端保持一致' : ''}
        />
      </div>
      {tlsValue === 1 && certApply ? (
        <div className="space-y-2">
          <Label>证书模式Cert Mode</Label>
          <NodeSelect
            value={(value.cert_mode as SelectValueType) ?? 'self'}
            options={LEGACY_TLS_CERT_MODE_OPTIONS}
            onChange={(next) => change('cert_mode', next)}
          />
        </div>
      ) : null}
      {value.cert_mode === 'dns' && certApply ? (
        <div className="space-y-2">
          <Label className="flex items-center gap-2">
            DNS解析提供商Provider
            <a
              className="text-sm text-primary"
              target="_blank"
              href="https://go-acme.github.io/lego/dns/index.html"
              rel="noopener noreferrer"
            >
              填写参考
            </a>
          </Label>
          <Input
            value={legacyText(value.provider)}
            onChange={(event) => change('provider', event.target.value)}
            placeholder="书写格式cloudflare"
          />
        </div>
      ) : null}
      {value.cert_mode === 'dns' && certApply ? (
        <div className="space-y-2">
          <Label>DNS env</Label>
          <Input
            value={legacyText(value.dns_env)}
            onChange={(event) => change('dns_env', event.target.value)}
            placeholder="书写格式CF_DNS_API_TOKEN=xxxxxxx如有多条使用逗号,分隔"
          />
        </div>
      ) : null}
      {tlsValue === 1 && value.cert_mode !== 'none' && certApply ? (
        <div className="space-y-2">
          <Label>证书公钥文件地址Cert File Path</Label>
          <Input
            value={legacyText(value.cert_file)}
            onChange={(event) => change('cert_file', event.target.value)}
            placeholder="留空在/etc/v2node/目录自动生成"
          />
        </div>
      ) : null}
      {tlsValue === 1 && value.cert_mode !== 'none' && certApply ? (
        <div className="space-y-2">
          <Label>证书私钥文件地址Key File Path</Label>
          <Input
            value={legacyText(value.key_file)}
            onChange={(event) => change('key_file', event.target.value)}
            placeholder="留空在/etc/v2node/目录自动生成"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label>Server Address</Label>
          <Input
            value={legacyText(value.dest)}
            onChange={(event) => change('dest', event.target.value)}
            placeholder="REALITY目标地址,默认使用SNI"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label>Server Port</Label>
          <Input
            value={legacyText(value.server_port)}
            onChange={(event) => change('server_port', event.target.value)}
            placeholder="REALITY目标端口,默认443"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label>Proxy Protocol</Label>
          <NodeSelect
            value={parseInt(String(value.xver ?? 0), 10) || 0}
            options={LEGACY_PROXY_PROTOCOL_OPTIONS}
            onChange={(next) => change('xver', next)}
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label>Private Key</Label>
          <Input
            value={legacyText(value.private_key)}
            onChange={(event) => change('private_key', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label>Public Key</Label>
          <Input
            value={legacyText(value.public_key)}
            onChange={(event) => change('public_key', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label>ShortId</Label>
          <Input
            value={legacyText(value.short_id)}
            onChange={(event) => change('short_id', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
      <div className="space-y-2">
        <Label>FingerPrint</Label>
        <NodeSelect
          value={value.fingerprint as SelectValueType}
          options={LEGACY_TLS_FINGERPRINT_OPTIONS}
          onChange={(next) => change('fingerprint', next)}
          placeholder="TLS指纹默认Chrome"
        />
      </div>
      {tlsValue === 1 && certApply ? (
        <div className="space-y-2">
          <Label>Reject unknown sni</Label>
          <div>
            <Switch
              checked={legacyBool(value.reject_unknown_sni)}
              onCheckedChange={(checked) => change('reject_unknown_sni', checked ? '1' : '0')}
            />
          </div>
        </div>
      ) : null}
      <div className="space-y-2">
        <Label>Allow Insecure</Label>
        <div>
          <Switch
            checked={legacyBool(value.allow_insecure)}
            onCheckedChange={(checked) => change('allow_insecure', checked ? '1' : '0')}
          />
        </div>
      </div>
      <div className="space-y-2">
        <Label>ECH (Encrypted Client Hello)</Label>
        <NodeSelect
          value={legacyText(value.ech)}
          options={LEGACY_ECH_MODE_OPTIONS}
          onChange={(next) => change('ech', next)}
          placeholder="选择 ECH 模式"
        />
      </div>
      {value.ech === 'cloudflare' ? (
        <div className="rounded-md border border-emerald-300 bg-emerald-50 px-3 py-2 text-sm text-emerald-600 dark:border-emerald-900 dark:bg-emerald-950/40">
          ✓ Cloudflare 托管 ECH，密钥由 Cloudflare 自动管理，客户端从 DNS 自动获取配置，服务端无需配置
        </div>
      ) : null}
      {value.ech === 'custom' ? (
        <div className="space-y-2">
          <Label>ECH Server Name (伪装域名/外层SNI)</Label>
          <Input
            value={legacyText(value.ech_server_name)}
            onChange={(event) => change('ech_server_name', event.target.value)}
            placeholder="必填"
          />
        </div>
      ) : null}
      {value.ech === 'custom' ? (
        <div className="space-y-2">
          <Label>ECH Key (服务端私钥)</Label>
          <Input
            value={legacyText(value.ech_key)}
            onChange={(event) => change('ech_key', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
      {value.ech === 'custom' ? (
        <div className="space-y-2">
          <Label>ECH Config (客户端配置)</Label>
          <Input
            value={legacyText(value.ech_config)}
            onChange={(event) => change('ech_config', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
    </div>
  );
}

function EncryptionSettingsField({ form }: { form: NodeForm }) {
  const settings = form.values.encryption_settings;
  const value = useMemo(
    () => normalizeLegacySettings(settings, LEGACY_ENCRYPTION_SETTINGS_DEFAULTS),
    [settings],
  );
  // Seed the store with the normalized defaults when the drawer opens, mirroring
  // the legacy field's effect. Guarded so equal content stops the update loop.
  useEffect(() => {
    if (JSON.stringify(form.values.encryption_settings) !== JSON.stringify(value)) {
      form.setFields({ encryption_settings: value });
    }
  }, [value, form]);
  const change = (key: string, next: unknown) => {
    form.setFields({ encryption_settings: { ...value, [key]: next } });
  };

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label>Mode</Label>
        <NodeSelect
          value={legacyText(value.mode) || 'native'}
          options={LEGACY_ENCRYPTION_MODE_OPTIONS}
          onChange={(next) => change('mode', next)}
        />
      </div>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className="space-y-2">
          <Label>RTT</Label>
          <NodeSelect
            value={legacyText(value.rtt) || '0rtt'}
            options={LEGACY_ENCRYPTION_RTT_OPTIONS}
            onChange={(next) => change('rtt', next)}
          />
        </div>
        {value.rtt === '0rtt' ? (
          <div className="space-y-2">
            <Label>Ticket time</Label>
            <Input
              value={legacyText(value.ticket)}
              onChange={(event) => change('ticket', event.target.value)}
              placeholder="最长允许时间"
            />
          </div>
        ) : null}
      </div>
      <div className="space-y-2">
        <Label>Server Padding</Label>
        <Input
          value={legacyText(value.server_padding)}
          onChange={(event) => change('server_padding', event.target.value)}
          placeholder="留空使用默认值100-111-1111.75-0-111.50-0-3333"
        />
      </div>
      <div className="space-y-2">
        <Label>Private Key</Label>
        <Input
          value={legacyText(value.private_key)}
          onChange={(event) => change('private_key', event.target.value)}
          placeholder="留空自动生成，需抗量子加密请自行替换"
        />
      </div>
      <div className="space-y-2">
        <Label>Client Padding</Label>
        <Input
          value={legacyText(value.client_padding)}
          onChange={(event) => change('client_padding', event.target.value)}
          placeholder="留空使用默认值100-111-1111.75-0-111.50-0-3333"
        />
      </div>
      <div className="space-y-2">
        <Label>Password</Label>
        <Input
          value={legacyText(value.password)}
          onChange={(event) => change('password', event.target.value)}
          placeholder="留空自动生成，需抗量子加密请自行替换"
        />
      </div>
    </div>
  );
}
