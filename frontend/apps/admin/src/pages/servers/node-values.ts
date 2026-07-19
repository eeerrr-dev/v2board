import type { admin } from '@v2board/api-client';
import { V2NODE_PROTOCOLS, type SelectValueType } from './domain';
import type { ServerNodeEditorValues, V2nodeEditorValues } from './form-schema';

export function nodeRecordValue(record: admin.ServerNode | undefined, key: string): unknown {
  if (!record) return undefined;
  const value: unknown = Reflect.get(record, key);
  return value;
}

export function nullableScalar(value: unknown): string | number | null | undefined {
  return value === null || typeof value === 'string' || typeof value === 'number'
    ? value
    : undefined;
}

export function nullableText(value: unknown): string | null | undefined {
  return value === null || typeof value === 'string' ? value : undefined;
}

export function binaryValue(value: unknown, fallback: 0 | 1): 0 | 1 | '0' | '1' {
  // §6.7 (W13): the dialect-v2 node rows carry real booleans for the legacy
  // 0/1 flags; the form keeps its binary select vocabulary.
  if (typeof value === 'boolean') return value ? 1 : 0;
  return value === 0 || value === 1 || value === '0' || value === '1' ? value : fallback;
}

export function securityValue(value: unknown, fallback: 0 | 1 | 2): 0 | 1 | 2 | '0' | '1' | '2' {
  return value === 0 ||
    value === 1 ||
    value === 2 ||
    value === '0' ||
    value === '1' ||
    value === '2'
    ? value
    : fallback;
}

export function stringArray(value: unknown): string[] | undefined {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
    ? value
    : undefined;
}

export function jsonEditorValue(value: unknown): unknown {
  return value && typeof value === 'object' ? JSON.stringify(value, null, 2) : value;
}

/** §6.7 (W13): `padding_scheme` rides the dialect-v2 wire as its decoded JSON
 * container; the editor textarea keeps working on its string spelling. */
export function paddingSchemeText(value: unknown): string | null | undefined {
  if (value === null || typeof value === 'string') return value;
  if (value === undefined) return undefined;
  return JSON.stringify(value, null, 4);
}

export type ShadowsocksEditorValues = Extract<ServerNodeEditorValues, { type: 'shadowsocks' }>;
export type VmessEditorValues = Extract<ServerNodeEditorValues, { type: 'vmess' }>;
export type TrojanEditorValues = Extract<ServerNodeEditorValues, { type: 'trojan' }>;
export type VlessEditorValues = Extract<ServerNodeEditorValues, { type: 'vless' }>;

export function shadowsocksCipher(value: unknown): ShadowsocksEditorValues['cipher'] {
  if (
    value === 'aes-128-gcm' ||
    value === 'aes-192-gcm' ||
    value === 'aes-256-gcm' ||
    value === 'chacha20-ietf-poly1305' ||
    value === '2022-blake3-aes-128-gcm' ||
    value === '2022-blake3-aes-256-gcm'
  ) {
    return value;
  }
  return 'chacha20-ietf-poly1305';
}

export function shadowsocksObfs(value: unknown): ShadowsocksEditorValues['obfs'] {
  return value === 'http' || value === null ? value : '';
}

export function vmessNetwork(value: unknown): VmessEditorValues['network'] {
  if (
    value === 'tcp' ||
    value === 'kcp' ||
    value === 'ws' ||
    value === 'http' ||
    value === 'domainsocket' ||
    value === 'quic' ||
    value === 'grpc' ||
    value === 'httpupgrade' ||
    value === 'xhttp'
  ) {
    return value;
  }
  return '';
}

export function trojanNetwork(value: unknown): TrojanEditorValues['network'] {
  return value === 'tcp' || value === 'ws' || value === 'grpc' ? value : '';
}

export function v2nodeNetwork(value: unknown): V2nodeEditorValues['config']['network'] {
  if (
    value === 'tcp' ||
    value === 'ws' ||
    value === 'grpc' ||
    value === 'http' ||
    value === 'httpupgrade' ||
    value === 'xhttp'
  ) {
    return value;
  }
  return 'tcp';
}

export function v2nodeProtocol(value: unknown): V2nodeEditorValues['config']['protocol'] {
  return V2NODE_PROTOCOLS.find((protocol) => protocol === value) ?? '';
}

export function vlessFlow(value: unknown): VlessEditorValues['flow'] {
  return value === 'xtls-rprx-vision' || value === null ? value : undefined;
}

export function hysteriaVersion(value: unknown): 1 | 2 | '1' | '2' {
  return value === 2 || value === '2' ? value : 1;
}

export function commonNodeInitialValues(record?: admin.ServerNode) {
  const tags = stringArray(nodeRecordValue(record, 'tags'));
  return {
    ...(record ? { id: record.id } : {}),
    name: record?.name ?? '',
    group_id: record?.group_id ?? [],
    ...(record ? { route_id: record.route_id } : {}),
    ...(record ? { parent_id: record.parent_id } : {}),
    host: record?.host ?? '',
    port: record?.port ?? '',
    server_port: record?.server_port ?? '',
    ...(tags === undefined ? {} : { tags }),
    rate: record?.rate ?? 1,
    ...(record ? { show: binaryValue(record.show, 0) } : {}),
  };
}

export function v2nodeInitialConfig(
  record: admin.ServerNode | undefined,
): V2nodeEditorValues['config'] {
  const protocol = v2nodeProtocol(nodeRecordValue(record, 'protocol'));
  const common = {
    tls: securityValue(nodeRecordValue(record, 'tls'), 0),
    network: v2nodeNetwork(nodeRecordValue(record, 'network')),
    network_settings: jsonEditorValue(nodeRecordValue(record, 'network_settings')),
    disable_sni: binaryValue(nodeRecordValue(record, 'disable_sni'), 0),
    zero_rtt_handshake: binaryValue(nodeRecordValue(record, 'zero_rtt_handshake'), 0),
  };

  if (protocol === '') return { ...common, protocol };
  if (protocol === 'shadowsocks') {
    return {
      ...common,
      protocol,
      cipher: nullableText(nodeRecordValue(record, 'cipher')) ?? 'aes-128-gcm',
    };
  }
  if (protocol === 'vless') {
    return {
      ...common,
      protocol,
      tls_settings: nodeRecordValue(record, 'tls_settings'),
      flow: vlessFlow(nodeRecordValue(record, 'flow')),
      encryption: nullableText(nodeRecordValue(record, 'encryption')),
      encryption_settings: nodeRecordValue(record, 'encryption_settings'),
    };
  }
  if (protocol === 'tuic') {
    return {
      ...common,
      protocol,
      tls_settings: nodeRecordValue(record, 'tls_settings'),
      udp_relay_mode: nullableText(nodeRecordValue(record, 'udp_relay_mode')) ?? 'native',
      congestion_control: nullableText(nodeRecordValue(record, 'congestion_control')) ?? 'cubic',
    };
  }
  if (protocol === 'hysteria2') {
    return {
      ...common,
      protocol,
      tls_settings: nodeRecordValue(record, 'tls_settings'),
      up_mbps: nullableScalar(nodeRecordValue(record, 'up_mbps')),
      down_mbps: nullableScalar(nodeRecordValue(record, 'down_mbps')),
      obfs: nullableText(nodeRecordValue(record, 'obfs')),
      obfs_password: nullableText(nodeRecordValue(record, 'obfs_password')),
    };
  }
  if (protocol === 'anytls') {
    return {
      ...common,
      protocol,
      tls_settings: nodeRecordValue(record, 'tls_settings'),
      padding_scheme: paddingSchemeText(nodeRecordValue(record, 'padding_scheme')),
    };
  }
  if (protocol === 'vmess') {
    return {
      ...common,
      protocol,
      tls_settings: nodeRecordValue(record, 'tls_settings'),
    };
  }
  return {
    ...common,
    protocol,
    tls_settings: nodeRecordValue(record, 'tls_settings'),
  };
}

// Build the exact typed RHF input for each endpoint. Response-only fields are
// read explicitly, and V2node protocol data is nested before it reaches the form.
export function getNodeInitialValues(
  type: admin.ServerTypeName,
  record?: admin.ServerNode,
): ServerNodeEditorValues {
  const common = commonNodeInitialValues(record);
  if (type === 'shadowsocks') {
    return {
      ...common,
      type,
      cipher: shadowsocksCipher(nodeRecordValue(record, 'cipher')),
      obfs: shadowsocksObfs(nodeRecordValue(record, 'obfs')),
      obfs_settings: nodeRecordValue(record, 'obfs_settings'),
    };
  }
  if (type === 'vmess') {
    return {
      ...common,
      type,
      tls: binaryValue(nodeRecordValue(record, 'tls'), 0),
      network: vmessNetwork(nodeRecordValue(record, 'network')),
      networkSettings: jsonEditorValue(nodeRecordValue(record, 'networkSettings')),
      tlsSettings: nodeRecordValue(record, 'tlsSettings'),
      ruleSettings: nodeRecordValue(record, 'ruleSettings'),
      dnsSettings: nodeRecordValue(record, 'dnsSettings'),
    };
  }
  if (type === 'trojan') {
    return {
      ...common,
      type,
      network: trojanNetwork(nodeRecordValue(record, 'network')),
      network_settings: jsonEditorValue(nodeRecordValue(record, 'network_settings')),
      allow_insecure: binaryValue(nodeRecordValue(record, 'allow_insecure'), 0),
      server_name: nullableText(nodeRecordValue(record, 'server_name')),
    };
  }
  if (type === 'hysteria') {
    return {
      ...common,
      type,
      version: hysteriaVersion(nodeRecordValue(record, 'version')),
      up_mbps: nullableScalar(nodeRecordValue(record, 'up_mbps')),
      down_mbps: nullableScalar(nodeRecordValue(record, 'down_mbps')),
      obfs: nullableText(nodeRecordValue(record, 'obfs')),
      obfs_password: nullableText(nodeRecordValue(record, 'obfs_password')),
      server_name: nullableText(nodeRecordValue(record, 'server_name')),
      insecure: binaryValue(nodeRecordValue(record, 'insecure'), 0),
    };
  }
  if (type === 'tuic') {
    return {
      ...common,
      type,
      server_name: nullableText(nodeRecordValue(record, 'server_name')),
      insecure: binaryValue(nodeRecordValue(record, 'insecure'), 0),
      disable_sni: binaryValue(nodeRecordValue(record, 'disable_sni'), 0),
      udp_relay_mode: nullableText(nodeRecordValue(record, 'udp_relay_mode')) ?? 'native',
      zero_rtt_handshake: binaryValue(nodeRecordValue(record, 'zero_rtt_handshake'), 0),
      congestion_control: nullableText(nodeRecordValue(record, 'congestion_control')) ?? 'cubic',
    };
  }
  if (type === 'vless') {
    return {
      ...common,
      type,
      sort: nullableScalar(nodeRecordValue(record, 'sort')),
      tls: securityValue(nodeRecordValue(record, 'tls'), 0),
      tls_settings: nodeRecordValue(record, 'tls_settings'),
      flow: record ? vlessFlow(nodeRecordValue(record, 'flow')) : null,
      network:
        typeof nodeRecordValue(record, 'network') === 'string'
          ? String(nodeRecordValue(record, 'network'))
          : '',
      network_settings: jsonEditorValue(nodeRecordValue(record, 'network_settings')),
      encryption: nullableText(nodeRecordValue(record, 'encryption')),
      encryption_settings: nodeRecordValue(record, 'encryption_settings'),
    };
  }
  if (type === 'anytls') {
    return {
      ...common,
      type,
      server_name: nullableText(nodeRecordValue(record, 'server_name')),
      insecure: binaryValue(nodeRecordValue(record, 'insecure'), 0),
      padding_scheme: paddingSchemeText(nodeRecordValue(record, 'padding_scheme')),
    };
  }
  return {
    ...common,
    type,
    sort: nullableScalar(nodeRecordValue(record, 'sort')),
    listen_ip: nullableText(nodeRecordValue(record, 'listen_ip')),
    install_command: nullableText(nodeRecordValue(record, 'install_command')),
    config: v2nodeInitialConfig(record),
  };
}

export function displayText(value: unknown) {
  return value == null ? '' : String(value);
}

export function inputValue(value: unknown) {
  return typeof value === 'string' || typeof value === 'number' ? value : '';
}

export function selectValue(value: unknown): SelectValueType {
  return value === null || typeof value === 'string' || typeof value === 'number'
    ? value
    : undefined;
}

export function toBoolean(value: unknown) {
  return parseInt(String(value ?? 0), 10) !== 0;
}
