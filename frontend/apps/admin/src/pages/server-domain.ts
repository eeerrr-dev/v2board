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

export const ROUTE_ACTION_TEXT = {
  block: '禁止访问(域名目标)',
  block_ip: '禁止访问(IP目标)',
  block_port: '禁止访问(端口目标)',
  protocol: '禁止访问(协议)',
  dns: '指定DNS服务器进行解析',
  route: '指定出站服务器(域名目标)',
  route_ip: '指定出站服务器(IP目标)',
  default_out: '自定义默认出站',
} satisfies Record<(typeof SERVER_ROUTE_ACTIONS)[number], string>;

export const ROUTE_ACTION_OPTIONS = optionsFromLabels(SERVER_ROUTE_ACTIONS, ROUTE_ACTION_TEXT);

export const BINARY_SELECT_OPTIONS = [
  { value: 0, label: '否' },
  { value: 1, label: '是' },
] satisfies SelectOption<0 | 1>[];

export const TLS_SUPPORT_OPTIONS = [
  { value: 0, label: '不支持' },
  { value: 1, label: '支持' },
] satisfies SelectOption<0 | 1>[];

export const SECURITY_NONE_OPTION = { value: 0, label: '无' } satisfies SelectOption<0>;
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

export const V2NODE_TRANSPORT_LABELS = {
  tcp: 'TCP',
  ws: 'WebSocket',
  grpc: 'gRPC',
  http: 'HTTP伪装',
  httpupgrade: 'HTTPUpgrade',
  xhttp: 'XHTTP',
} satisfies Record<(typeof V2NODE_TRANSPORTS)[number], string>;
export const V2NODE_SHADOWSOCKS_NETWORK_OPTIONS = optionsFromLabels(
  V2NODE_SHADOWSOCKS_NETWORKS,
  V2NODE_TRANSPORT_LABELS,
);

export const V2NODE_TRANSPORT_OPTIONS = optionsFromLabels(
  V2NODE_STANDARD_TRANSPORTS,
  V2NODE_TRANSPORT_LABELS,
);
export const HYSTERIA2_OBFS_OPTIONS: SelectOption[] = [
  { value: null, label: '无' },
  { value: 'salamander', label: 'salamander' },
];
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
export const SHADOWSOCKS_OBFS_OPTIONS: SelectOption[] = [
  { value: '', label: '无' },
  { value: 'http', label: 'HTTP' },
];
export const VLESS_ENCRYPTION_OPTIONS: SelectOption[] = [
  { value: null, label: '无' },
  { value: 'mlkem768x25519plus', label: 'MLKEM768X25519PLUS' },
];
export const VLESS_FLOW_NONE_OPTIONS: SelectOption[] = [{ value: null, label: '无' }];
export const VLESS_FLOW_OPTIONS: SelectOption[] = [
  ...VLESS_FLOW_NONE_OPTIONS,
  { value: 'xtls-rprx-vision', label: 'xtls-rprx-vision' },
];
export const HYSTERIA_VERSION_OPTIONS: SelectOption[] = [
  { value: 1, label: 'v1' },
  { value: 2, label: 'v2' },
];
export const HYSTERIA_V1_OBFS_OPTIONS: SelectOption[] = [
  { value: null, label: '无' },
  { value: 'xplus', label: 'xplus' },
];
export const TLS_CERT_MODE_OPTIONS: SelectOption[] = [
  { value: 'self', label: '自签名' },
  { value: 'http', label: 'HTTP申请' },
  { value: 'dns', label: 'DNS申请' },
  { value: 'none', label: '无证书(关闭TLS)' },
];
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
export const ECH_MODE_OPTIONS: SelectOption[] = [
  { value: '', label: '无' },
  { value: 'cloudflare', label: 'Cloudflare' },
  { value: 'custom', label: '自定义 SNI' },
];
export const ENCRYPTION_MODE_OPTIONS: SelectOption[] = [
  { value: 'native', label: 'native' },
  { value: 'xorpub', label: 'xorpub' },
  { value: 'random', label: 'random' },
];
export const ENCRYPTION_RTT_OPTIONS: SelectOption[] = [
  { value: '0rtt', label: '0rtt' },
  { value: '1rtt', label: '1rtt' },
];

export const SERVER_SORT_LEAVE_PROMPT = '节点排序还没有保存，是否离开';
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
  reject_unknown_sni: '0',
  allow_insecure: '0',
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

export function getRouteMatchLabel(value: admin.ServerRoute['match'] | undefined) {
  if (!value || value.length === 0) return '无规则时默认';
  const rules = typeof value === 'string' ? value.split(',').filter(Boolean) : value;
  return `匹配 ${rules.length} 条规则`;
}

export function getRouteMatchTextareaValue(value: admin.ServerRoute['match'] | undefined) {
  if (Array.isArray(value)) return value.join('\n');
  return value?.split(',').join('\n');
}

export function getRouteMatchPlaceholder(action: string | undefined) {
  if (action === 'protocol') return 'http\ntls\nquic\nbittorrent';
  if (action === 'block_port') return '53\n443\n1000-2000';
  if (action && ['route_ip', 'block_ip'].includes(action)) {
    return '127.0.0.1(单一匹配)\n10.0.0.0/8(范围匹配)\ngeoip:cn(预定义列表匹配)';
  }
  return 'example.com(关键字匹配)\ndomain:example.com(子域名匹配)\ngeosite:netflix(预定义域名列表)';
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

export function getV2nodeSecurityOptions(protocol: unknown): SelectOption[] {
  const protocolValue = protocol == null ? null : String(protocol);
  return [
    ...(protocolValue === 'vless' || protocolValue === 'vmess' ? [SECURITY_NONE_OPTION] : []),
    SECURITY_TLS_OPTION,
    ...(protocolValue === 'vless' || protocolValue === 'anytls' ? [SECURITY_REALITY_OPTION] : []),
  ];
}

export function getV2nodeTransportOptions(protocol: unknown): SelectOption[] {
  return protocol === 'trojan' ? TROJAN_NETWORK_OPTIONS : V2NODE_TRANSPORT_OPTIONS;
}

export function getVlessFlowOptions(network: unknown): SelectOption[] {
  return String(network) === 'tcp' ? VLESS_FLOW_OPTIONS : VLESS_FLOW_NONE_OPTIONS;
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
