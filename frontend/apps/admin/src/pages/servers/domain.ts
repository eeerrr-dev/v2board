import type { TFunction } from 'i18next';
import type { admin } from '@v2board/api-client';

export const SERVER_ROUTE_ACTIONS = [
  'block',
  'block_ip',
  'block_port',
  'protocol',
  'dns',
  'route',
  'route_ip',
  'default_out',
] as const;

export const V2NODE_PROTOCOLS = [
  'anytls',
  'hysteria2',
  'shadowsocks',
  'trojan',
  'tuic',
  'vless',
  'vmess',
] as const;

export const SHADOWSOCKS_CIPHERS = [
  'aes-128-gcm',
  'aes-192-gcm',
  'aes-256-gcm',
  'chacha20-ietf-poly1305',
  '2022-blake3-aes-128-gcm',
  '2022-blake3-aes-256-gcm',
] as const;

export const STREAM_NETWORKS = ['tcp', 'ws', 'grpc', 'kcp', 'httpupgrade', 'xhttp'] as const;

export const VMESS_NETWORKS = ['', ...STREAM_NETWORKS, 'http', 'domainsocket', 'quic'] as const;

export const TROJAN_TRANSPORTS = ['tcp', 'ws', 'grpc'] as const;
export const TROJAN_NETWORKS = ['', ...TROJAN_TRANSPORTS] as const;
export const V2NODE_STANDARD_TRANSPORTS = ['tcp', 'ws', 'grpc', 'httpupgrade', 'xhttp'] as const;
export const V2NODE_SHADOWSOCKS_ONLY_TRANSPORTS = ['http'] as const;
export const V2NODE_SHADOWSOCKS_NETWORKS = ['tcp', ...V2NODE_SHADOWSOCKS_ONLY_TRANSPORTS] as const;
export const V2NODE_TRANSPORTS = [
  ...V2NODE_STANDARD_TRANSPORTS,
  ...V2NODE_SHADOWSOCKS_ONLY_TRANSPORTS,
] as const;

export type SelectValueType = string | number | null | undefined;
export type SelectOption<T extends string | number | null = string | number | null> = {
  value: T;
  label: string;
};

function optionsFromLabels<const T extends readonly (string | number)[]>(
  values: T,
  labels: Record<T[number], string>,
): SelectOption<T[number]>[] {
  return values.map((value) => ({ value, label: labels[value as T[number]] }));
}

export const SERVER_TYPES = [
  'v2node',
  'shadowsocks',
  'vmess',
  'trojan',
  'hysteria',
  'tuic',
  'vless',
  'anytls',
] as const satisfies readonly admin.ServerTypeName[];
export type ServerType = (typeof SERVER_TYPES)[number];

export const SERVER_TYPE_LABELS = {
  v2node: 'V2node',
  shadowsocks: 'Shadowsocks',
  vmess: 'VMess',
  trojan: 'Trojan',
  hysteria: 'Hysteria',
  tuic: 'Tuic',
  vless: 'VLess',
  anytls: 'AnyTLS',
} satisfies Record<(typeof SERVER_TYPES)[number], string>;

export const SERVER_TYPE_BADGE_CLASSES = {
  shadowsocks: 'bg-success text-white',
  vmess: 'bg-primary text-primary-foreground',
  trojan: 'bg-warning text-white',
  hysteria: 'bg-foreground text-background',
  tuic: 'bg-secondary text-secondary-foreground',
  vless: 'bg-info text-white',
  anytls: 'bg-warning text-white',
  v2node: 'bg-destructive text-white',
} satisfies Record<(typeof SERVER_TYPES)[number], string>;

export const AVAILABLE_STATUS: Record<number, 'error' | 'warning' | 'processing'> = {
  0: 'error',
  1: 'warning',
  2: 'processing',
};

export const AVAILABLE_STATUS_DOT: Record<'error' | 'warning' | 'processing', string> = {
  error: 'bg-destructive',
  warning: 'bg-warning',
  processing: 'bg-info',
};

// Route action values are wire identifiers; only the labels are copy, so the
// option/label constants resolve their copy through t() at render time.
export function getRouteActionText(
  t: TFunction,
): Record<(typeof SERVER_ROUTE_ACTIONS)[number], string> {
  return {
    block: t(($) => $.admin.servers.route_action_block),
    block_ip: t(($) => $.admin.servers.route_action_block_ip),
    block_port: t(($) => $.admin.servers.route_action_block_port),
    protocol: t(($) => $.admin.servers.route_action_protocol),
    dns: t(($) => $.admin.servers.route_action_dns),
    route: t(($) => $.admin.servers.route_action_route),
    route_ip: t(($) => $.admin.servers.route_action_route_ip),
    default_out: t(($) => $.admin.servers.route_action_default_out),
  };
}

export function getRouteActionOptions(t: TFunction) {
  return optionsFromLabels(SERVER_ROUTE_ACTIONS, getRouteActionText(t));
}

export function getBinarySelectOptions(t: TFunction): SelectOption<0 | 1>[] {
  return [
    { value: 0, label: t(($) => $.common.no) },
    { value: 1, label: t(($) => $.common.yes) },
  ];
}

export function getTlsSupportOptions(t: TFunction): SelectOption<0 | 1>[] {
  return [
    { value: 0, label: t(($) => $.admin.servers.not_supported) },
    { value: 1, label: t(($) => $.admin.servers.supported) },
  ];
}

export function getSecurityNoneOption(t: TFunction): SelectOption<0> {
  return { value: 0, label: t(($) => $.admin.servers.none) };
}
export const SECURITY_TLS_OPTION = { value: 1, label: 'TLS' } satisfies SelectOption<1>;
export const SECURITY_REALITY_OPTION = {
  value: 2,
  label: 'Reality',
} satisfies SelectOption<2>;
export const STREAM_NETWORK_LABELS = {
  tcp: 'TCP',
  ws: 'WebSocket',
  grpc: 'gRPC',
  kcp: 'mKCP',
  httpupgrade: 'HTTPUpgrade',
  xhttp: 'XHTTP',
} satisfies Record<(typeof STREAM_NETWORKS)[number], string>;
export const STREAM_NETWORK_OPTIONS = optionsFromLabels(STREAM_NETWORKS, STREAM_NETWORK_LABELS);

export const TROJAN_NETWORK_LABELS = {
  tcp: 'TCP',
  ws: 'WebSocket',
  grpc: 'gRPC',
} satisfies Record<(typeof TROJAN_TRANSPORTS)[number], string>;
export const TROJAN_NETWORK_OPTIONS = optionsFromLabels(TROJAN_TRANSPORTS, TROJAN_NETWORK_LABELS);

export const V2NODE_PROTOCOL_LABELS = {
  anytls: 'AnyTLS',
  hysteria2: 'Hysteria2',
  shadowsocks: 'Shadowsocks',
  trojan: 'Trojan',
  tuic: 'Tuic',
  vless: 'VLess',
  vmess: 'VMess',
} satisfies Record<(typeof V2NODE_PROTOCOLS)[number], string>;
export const V2NODE_PROTOCOL_OPTIONS = optionsFromLabels(V2NODE_PROTOCOLS, V2NODE_PROTOCOL_LABELS);

function getV2nodeTransportLabels(t: TFunction) {
  return {
    tcp: 'TCP',
    ws: 'WebSocket',
    grpc: 'gRPC',
    http: t(($) => $.admin.servers.http_disguise),
    httpupgrade: 'HTTPUpgrade',
    xhttp: 'XHTTP',
  } satisfies Record<(typeof V2NODE_TRANSPORTS)[number], string>;
}
export function getV2nodeShadowsocksNetworkOptions(t: TFunction) {
  return optionsFromLabels(V2NODE_SHADOWSOCKS_NETWORKS, getV2nodeTransportLabels(t));
}

export function getHysteria2ObfsOptions(t: TFunction): SelectOption[] {
  return [
    { value: null, label: t(($) => $.admin.servers.none) },
    { value: 'salamander', label: 'salamander' },
  ];
}
export const TUIC_RELAY_MODE_OPTIONS: SelectOption[] = [
  { value: 'native', label: 'native' },
  { value: 'quic', label: 'quic' },
];
export const TUIC_CONGESTION_CONTROL_OPTIONS: SelectOption[] = [
  { value: 'cubic', label: 'cubic' },
  { value: 'new_reno', label: 'new_reno' },
  { value: 'bbr', label: 'bbr' },
];
export const SHADOWSOCKS_CIPHER_LABELS = {
  'aes-128-gcm': 'aes-128-gcm',
  'aes-192-gcm': 'aes-192-gcm',
  'aes-256-gcm': 'aes-256-gcm',
  'chacha20-ietf-poly1305': 'chacha20-ietf-poly1305',
  '2022-blake3-aes-128-gcm': '2022-blake3-aes-128-gcm',
  '2022-blake3-aes-256-gcm': '2022-blake3-aes-256-gcm',
} satisfies Record<(typeof SHADOWSOCKS_CIPHERS)[number], string>;
export const SHADOWSOCKS_CIPHER_OPTIONS = optionsFromLabels(
  SHADOWSOCKS_CIPHERS,
  SHADOWSOCKS_CIPHER_LABELS,
);
export function getShadowsocksObfsOptions(t: TFunction): SelectOption[] {
  return [
    { value: '', label: t(($) => $.admin.servers.none) },
    { value: 'http', label: 'HTTP' },
  ];
}
export function getVlessEncryptionOptions(t: TFunction): SelectOption[] {
  return [
    { value: null, label: t(($) => $.admin.servers.none) },
    { value: 'mlkem768x25519plus', label: 'MLKEM768X25519PLUS' },
  ];
}
function getVlessFlowNoneOptions(t: TFunction): SelectOption[] {
  return [{ value: null, label: t(($) => $.admin.servers.none) }];
}
export function getVlessFlowOptions(t: TFunction): SelectOption[] {
  return [...getVlessFlowNoneOptions(t), { value: 'xtls-rprx-vision', label: 'xtls-rprx-vision' }];
}
export const HYSTERIA_VERSION_OPTIONS: SelectOption[] = [
  { value: 1, label: 'v1' },
  { value: 2, label: 'v2' },
];
export function getHysteriaV1ObfsOptions(t: TFunction): SelectOption[] {
  return [
    { value: null, label: t(($) => $.admin.servers.none) },
    { value: 'xplus', label: 'xplus' },
  ];
}
export function getTlsCertModeOptions(t: TFunction): SelectOption[] {
  return [
    { value: 'self', label: t(($) => $.admin.servers.cert_mode_self) },
    { value: 'http', label: t(($) => $.admin.servers.cert_mode_http) },
    { value: 'dns', label: t(($) => $.admin.servers.cert_mode_dns) },
    { value: 'none', label: t(($) => $.admin.servers.cert_mode_none) },
  ];
}
export const PROXY_PROTOCOL_OPTIONS: SelectOption[] = [
  { value: 0, label: '0' },
  { value: 1, label: '1' },
  { value: 2, label: '2' },
];
export const TLS_FINGERPRINT_OPTIONS: SelectOption[] = [
  { value: 'chrome', label: 'Chrome' },
  { value: 'firefox', label: 'Firefox' },
  { value: 'safari', label: 'Safari' },
  { value: 'ios', label: 'IOS' },
  { value: 'android', label: 'Android' },
  { value: 'edge', label: 'Edge' },
  { value: '360', label: '360' },
  { value: 'qq', label: 'QQ' },
];
export function getEchModeOptions(t: TFunction): SelectOption[] {
  return [
    { value: '', label: t(($) => $.admin.servers.none) },
    { value: 'cloudflare', label: 'Cloudflare' },
    { value: 'custom', label: t(($) => $.admin.servers.ech_custom_sni) },
  ];
}
export const ENCRYPTION_MODE_OPTIONS: SelectOption[] = [
  { value: 'native', label: 'native' },
  { value: 'xorpub', label: 'xorpub' },
  { value: 'random', label: 'random' },
];
export const ENCRYPTION_RTT_OPTIONS: SelectOption[] = [
  { value: '0rtt', label: '0rtt' },
  { value: '1rtt', label: '1rtt' },
];

export const ANYTLS_PADDING_SCHEME_PLACEHOLDER = JSON.stringify(
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
export const V2NODE_SECURITY_DEFAULT_TLS_PROTOCOLS = ['hysteria2', 'trojan', 'tuic'];
export const VMESS_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
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
export const VLESS_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: VMESS_NETWORK_SETTINGS_PLACEHOLDERS.tcp!,
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
  grpc: VMESS_NETWORK_SETTINGS_PLACEHOLDERS.grpc!,
  kcp: VMESS_NETWORK_SETTINGS_PLACEHOLDERS.kcp!,
  httpupgrade: VMESS_NETWORK_SETTINGS_PLACEHOLDERS.httpupgrade!,
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
export const TROJAN_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: '',
  ws: VMESS_NETWORK_SETTINGS_PLACEHOLDERS.ws!,
  grpc: VMESS_NETWORK_SETTINGS_PLACEHOLDERS.grpc!,
};
export const V2NODE_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
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
  grpc: VMESS_NETWORK_SETTINGS_PLACEHOLDERS.grpc!,
  httpupgrade: JSON.stringify(
    {
      acceptProxyProtocol: false,
      path: '/',
      host: 'xtls.github.io',
    },
    null,
    4,
  ),
  xhttp: VLESS_NETWORK_SETTINGS_PLACEHOLDERS.xhttp!,
};
export const NETWORK_SETTINGS_PLACEHOLDERS: Partial<
  Record<admin.ServerTypeName, Record<string, string>>
> = {
  vmess: VMESS_NETWORK_SETTINGS_PLACEHOLDERS,
  vless: VLESS_NETWORK_SETTINGS_PLACEHOLDERS,
  trojan: TROJAN_NETWORK_SETTINGS_PLACEHOLDERS,
  v2node: V2NODE_NETWORK_SETTINGS_PLACEHOLDERS,
};
export const TLS_SETTINGS_DEFAULTS = {
  server_name: '',
  cert_mode: 'self',
  provider: '',
  dns_env: '',
  reject_unknown_sni: false,
  allow_insecure: false,
};
export const ENCRYPTION_SETTINGS_DEFAULTS = {
  mode: 'native',
  rtt: '0rtt',
  ticket: '600s',
  server_padding: null,
  client_padding: null,
  private_key: null,
  password: null,
};

// Node ID filter values preserve the externally tested `node.type === value.toLowerCase()` rule.
export const NODE_TYPE_FILTERS = [
  'V2node',
  'Shadowsocks',
  'Vmess',
  'Trojan',
  'Hysteria',
  'Tuic',
  'Vless',
  'AnyTLS',
].map((value) => ({ text: value, value }));

export interface NodeFilterItem {
  text: string;
  value: string;
}

// ---------------------------------------------------------------------------
// Pure contract helpers (Tier-1). Signatures and request-shaping behavior stay
// stable because the Rust API and deployed proxy nodes consume the result.
// ---------------------------------------------------------------------------

// §6.7 (W13): route `match` is always a real JSON array on the modern wire.
export function getRouteMatchLabel(t: TFunction, value: admin.ServerRoute['match'] | undefined) {
  if (!value || value.length === 0) return t(($) => $.admin.servers.route_match_default);
  return t(($) => $.admin.servers.route_match_count, { n: value.length });
}

export function getRouteMatchTextareaValue(value: admin.ServerRoute['match'] | undefined) {
  return value?.join('\n');
}

export function getRouteMatchPlaceholder(t: TFunction, action: string | undefined) {
  if (action === 'protocol') return 'http\ntls\nquic\nbittorrent';
  if (action === 'block_port') return '53\n443\n1000-2000';
  if (action && ['route_ip', 'block_ip'].includes(action)) {
    return t(($) => $.admin.servers.route_match_ip_placeholder);
  }
  return t(($) => $.admin.servers.route_match_domain_placeholder);
}

export function getAvailableStatus(status?: number | null) {
  return status == null ? undefined : AVAILABLE_STATUS[status];
}

// Apply node type and permission-group filters before the online-count sort.
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

export function moveServerNodeByDragIndexes(
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

export function normalizeNullableArray(value: unknown) {
  return Array.isArray(value) && value.length === 0 ? null : value;
}

export function getNetworkSettingsPlaceholder(type: admin.ServerTypeName, network: unknown) {
  return NETWORK_SETTINGS_PLACEHOLDERS[type]?.[String(network)] || '';
}

export function getV2nodeSecurityValue(protocol: unknown, tls: unknown) {
  const parsedTls = parseInt(String(tls ?? 0), 10);
  if (parsedTls) return parsedTls;
  const protocolValue = protocol == null ? null : String(protocol);
  return protocolValue && V2NODE_SECURITY_DEFAULT_TLS_PROTOCOLS.includes(protocolValue) ? 1 : 0;
}

export function getV2nodeSecurityOptions(t: TFunction, protocol: unknown): SelectOption[] {
  const protocolValue = protocol == null ? null : String(protocol);
  return [
    ...(protocolValue === 'vless' || protocolValue === 'vmess' ? [getSecurityNoneOption(t)] : []),
    SECURITY_TLS_OPTION,
    ...(protocolValue === 'vless' || protocolValue === 'anytls' ? [SECURITY_REALITY_OPTION] : []),
  ];
}

export function getV2nodeTransportOptions(t: TFunction, protocol: unknown): SelectOption[] {
  return protocol === 'trojan'
    ? TROJAN_NETWORK_OPTIONS
    : optionsFromLabels(V2NODE_STANDARD_TRANSPORTS, getV2nodeTransportLabels(t));
}

export function getVlessFlowOptionsForNetwork(t: TFunction, network: unknown): SelectOption[] {
  return String(network) === 'tcp' ? getVlessFlowOptions(t) : getVlessFlowNoneOptions(t);
}

export function getNumericSelectValue(value: unknown, fallback = 0) {
  return parseInt(String(value ?? fallback), 10) || fallback;
}

export function getBinarySelectValue(value: unknown) {
  return getNumericSelectValue(value) ? 1 : 0;
}

export function settingsObject(value: unknown): object | undefined {
  if (value && typeof value === 'object' && !Array.isArray(value)) return value;
  if (typeof value !== 'string' || !value.trim()) return undefined;
  try {
    const parsed: unknown = JSON.parse(value);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : undefined;
  } catch {
    return undefined;
  }
}

export function normalizeSettings(value: unknown, defaults: object): object {
  return Object.assign({}, defaults, settingsObject(value));
}

export function settingValue(settings: object, key: string): unknown {
  const value: unknown = Reflect.get(settings, key);
  return value;
}

export function withSetting(settings: object, key: string, value: unknown): object {
  return Object.assign({}, settings, { [key]: value });
}
