import { z } from 'zod';
import { isEmptyInput, isIntegerInput, isNumericInput } from '@/lib/form-input-validation';
import {
  SERVER_ROUTE_ACTIONS,
  SHADOWSOCKS_CIPHERS,
  TROJAN_NETWORKS,
  V2NODE_TRANSPORTS,
  VMESS_NETWORKS,
} from './server-domain';

export const serverGroupFormSchema = z.object({
  id: z.number().int().positive().optional(),
  name: z.string().refine((value) => value.trim().length > 0, '组名不能为空'),
});

export type ServerGroupFormValues = z.infer<typeof serverGroupFormSchema>;

export const serverRouteFormSchema = z
  .object({
    id: z.number().int().positive().optional(),
    remarks: z.string().refine((value) => value.trim().length > 0, '备注不能为空'),
    match: z.string(),
    action: z.enum(SERVER_ROUTE_ACTIONS, { error: '动作类型不能为空' }),
    action_value: z.string().nullable(),
  })
  .superRefine((values, context) => {
    if (values.action !== 'default_out' && splitServerRouteMatches(values.match).length === 0) {
      context.addIssue({
        code: 'custom',
        path: ['match'],
        message: '匹配值不能为空',
      });
    }
  });

export type ServerRouteFormValues = z.infer<typeof serverRouteFormSchema>;
export type ServerRouteAction = ServerRouteFormValues['action'];

export function splitServerRouteMatches(value: string): string[] {
  return value.split('\n').filter(Boolean);
}

const scalar = z.union([z.string(), z.number()]);
const optionalScalar = scalar.nullable().optional();
const requiredScalar = (message: string) =>
  scalar.refine((value) => String(value).trim().length > 0, message);
const numericScalar = (requiredMessage: string, invalidMessage: string) =>
  requiredScalar(requiredMessage).refine(
    (value) => String(value).trim().length === 0 || isNumericInput(value),
    invalidMessage,
  );
const optionalIntegerScalar = optionalScalar.refine(
  (value) => isEmptyInput(value) || isIntegerInput(value),
  '父节点格式不正确',
);
const optionalNumericScalar = optionalScalar.refine(
  (value) => isEmptyInput(value) || isNumericInput(value),
  '带宽格式不正确',
);
const binaryScalar = z.union([z.literal(0), z.literal(1), z.literal('0'), z.literal('1')]);
const securityScalar = z.union([
  z.literal(0),
  z.literal(1),
  z.literal(2),
  z.literal('0'),
  z.literal('1'),
  z.literal('2'),
]);
const nullableString = z.string().nullable().optional();
const settingsContainer = z.union([z.looseObject({}), z.array(z.unknown())], {
  error: '配置格式有误',
});
const settingsInput = z.preprocess(parseJsonContainer, settingsContainer.nullable().optional());
const obfsSettingsInput = z.preprocess(
  parseJsonContainer,
  z.object({ path: optionalScalar, host: optionalScalar }).nullable().optional(),
);

function parseJsonContainer(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  if (!value.trim()) return null;
  try {
    const parsed: unknown = JSON.parse(value);
    return parsed;
  } catch {
    return value;
  }
}

const jsonContainerInput = z
  .preprocess(
    parseJsonContainer,
    z.union([settingsContainer, z.null()], { error: '传输协议配置格式有误' }).optional(),
  )
  .transform((value) => value ?? null);

const dnsSettingsInput = settingsInput.transform((value) => {
  if (!value || Array.isArray(value)) return null;
  return Array.isArray(value.servers) && value.servers.length > 0 ? value : null;
});

function isJsonContainer(value: string): boolean {
  if (!value.trim()) return true;
  const parsed = parseJsonContainer(value);
  return parsed === null || typeof parsed === 'object';
}

const paddingSchemeInput = z
  .string()
  .nullable()
  .optional()
  .refine((value) => value == null || !value.trim() || isJsonContainer(value), '填充方案格式有误');

const commonNodeFields = {
  id: z.number().int().positive().optional(),
  name: z.string().refine((value) => value.trim().length > 0, '节点名称不能为空'),
  group_id: z.array(scalar).min(1, '权限组不能为空'),
  route_id: z.array(scalar).nullable().optional(),
  parent_id: optionalIntegerScalar,
  host: z.string().refine((value) => value.trim().length > 0, '节点地址不能为空'),
  port: requiredScalar('连接端口不能为空'),
  server_port: requiredScalar('后端服务端口不能为空'),
  tags: z.array(z.string()).nullable().optional(),
  rate: numericScalar('倍率不能为空', '倍率格式不正确'),
  show: binaryScalar.optional(),
};

const commonNodeSchema = z.object(commonNodeFields);

const shadowsocksNodeSchema = commonNodeSchema.extend({
  type: z.literal('shadowsocks'),
  cipher: z.enum(SHADOWSOCKS_CIPHERS, { error: '加密方式不能为空' }),
  obfs: z.union([z.literal(''), z.literal('http'), z.null()]).optional(),
  obfs_settings: obfsSettingsInput,
});

const vmessNodeSchema = commonNodeSchema.extend({
  type: z.literal('vmess'),
  tls: binaryScalar,
  network: z.enum(VMESS_NETWORKS, { error: '传输协议格式不正确' }),
  networkSettings: jsonContainerInput,
  tlsSettings: settingsInput,
  ruleSettings: settingsInput,
  dnsSettings: dnsSettingsInput,
});

const trojanNodeSchema = commonNodeSchema.extend({
  type: z.literal('trojan'),
  network: z.enum(TROJAN_NETWORKS, { error: '传输协议格式不正确' }),
  network_settings: jsonContainerInput,
  allow_insecure: binaryScalar.optional(),
  server_name: nullableString,
});

const hysteriaNodeSchema = commonNodeSchema.extend({
  type: z.literal('hysteria'),
  version: z.union([z.literal(1), z.literal(2), z.literal('1'), z.literal('2')]),
  up_mbps: optionalNumericScalar,
  down_mbps: optionalNumericScalar,
  obfs: nullableString,
  obfs_password: nullableString,
  server_name: nullableString,
  insecure: binaryScalar,
});

const tuicNodeSchema = commonNodeSchema.extend({
  type: z.literal('tuic'),
  server_name: nullableString,
  insecure: binaryScalar,
  disable_sni: binaryScalar,
  udp_relay_mode: nullableString,
  zero_rtt_handshake: binaryScalar,
  congestion_control: nullableString,
});

const vlessNodeSchema = commonNodeSchema.extend({
  type: z.literal('vless'),
  sort: optionalScalar,
  tls: securityScalar,
  tls_settings: settingsInput,
  flow: z.union([z.literal('xtls-rprx-vision'), z.null()]).optional(),
  network: z.string().refine((value) => value.length > 0, '传输协议不能为空'),
  network_settings: jsonContainerInput,
  encryption: nullableString,
  encryption_settings: settingsInput,
});

const anytlsNodeSchema = commonNodeSchema.extend({
  type: z.literal('anytls'),
  server_name: nullableString,
  insecure: binaryScalar,
  padding_scheme: paddingSchemeInput,
});

const v2nodeProtocolCommonFields = {
  tls: securityScalar,
  network: z.enum(V2NODE_TRANSPORTS, {
    error: '传输协议格式不正确',
  }),
  network_settings: jsonContainerInput,
  disable_sni: binaryScalar,
  zero_rtt_handshake: binaryScalar,
};

const v2nodeProtocolInputSchema = z.discriminatedUnion('protocol', [
  z.object({
    ...v2nodeProtocolCommonFields,
    protocol: z.literal(''),
  }),
  z.object({
    ...v2nodeProtocolCommonFields,
    protocol: z.literal('shadowsocks'),
    cipher: nullableString,
  }),
  z.object({
    ...v2nodeProtocolCommonFields,
    protocol: z.literal('vmess'),
    tls_settings: settingsInput,
  }),
  z.object({
    ...v2nodeProtocolCommonFields,
    protocol: z.literal('vless'),
    tls_settings: settingsInput,
    flow: z.union([z.literal('xtls-rprx-vision'), z.null()]).optional(),
    encryption: nullableString,
    encryption_settings: settingsInput,
  }),
  z.object({
    ...v2nodeProtocolCommonFields,
    protocol: z.literal('trojan'),
    tls_settings: settingsInput,
  }),
  z.object({
    ...v2nodeProtocolCommonFields,
    protocol: z.literal('tuic'),
    tls_settings: settingsInput,
    udp_relay_mode: nullableString,
    congestion_control: nullableString,
  }),
  z.object({
    ...v2nodeProtocolCommonFields,
    protocol: z.literal('hysteria2'),
    tls_settings: settingsInput,
    up_mbps: optionalNumericScalar,
    down_mbps: optionalNumericScalar,
    obfs: nullableString,
    obfs_password: nullableString,
  }),
  z.object({
    ...v2nodeProtocolCommonFields,
    protocol: z.literal('anytls'),
    tls_settings: settingsInput,
    padding_scheme: paddingSchemeInput,
  }),
]);

const v2nodeNodeSchema = commonNodeSchema.extend({
  type: z.literal('v2node'),
  sort: optionalScalar,
  listen_ip: nullableString,
  install_command: nullableString,
  config: v2nodeProtocolInputSchema,
});

const serverNodeInputSchema = z.discriminatedUnion('type', [
  shadowsocksNodeSchema,
  vmessNodeSchema,
  trojanNodeSchema,
  hysteriaNodeSchema,
  tuicNodeSchema,
  vlessNodeSchema,
  anytlsNodeSchema,
  v2nodeNodeSchema,
]);

type ParsedCommonNode = z.output<typeof commonNodeSchema>;

function commonNodePayload(values: ParsedCommonNode) {
  return {
    ...(values.id === undefined ? {} : { id: values.id }),
    name: values.name,
    group_id: values.group_id,
    ...(values.route_id === undefined ? {} : { route_id: values.route_id }),
    ...(values.parent_id === undefined ? {} : { parent_id: values.parent_id }),
    host: values.host,
    port: values.port,
    server_port: values.server_port,
    ...(values.tags === undefined ? {} : { tags: values.tags }),
    rate: values.rate,
    ...(values.show === undefined ? {} : { show: values.show }),
  };
}

export const serverNodeFormSchema = serverNodeInputSchema
  .superRefine((values, context) => {
    if ((values.type === 'vmess' || values.type === 'trojan') && values.network === '') {
      context.addIssue({
        code: 'custom',
        path: ['network'],
        message: '传输协议不能为空',
      });
    }
    if (values.type === 'v2node' && values.config.protocol === '') {
      context.addIssue({
        code: 'custom',
        path: ['config', 'protocol'],
        message: '节点协议不能为空',
      });
    }
  })
  .transform((values) => {
    const common = commonNodePayload(values);
    if (values.type === 'shadowsocks') {
      return {
        type: values.type,
        data: {
          ...common,
          cipher: values.cipher,
          ...(values.obfs === undefined ? {} : { obfs: values.obfs }),
          ...(values.obfs_settings === undefined ? {} : { obfs_settings: values.obfs_settings }),
        },
      };
    }
    if (values.type === 'vmess') {
      return {
        type: values.type,
        data: {
          ...common,
          tls: values.tls,
          network: values.network,
          networkSettings: values.networkSettings,
          ...(values.tlsSettings === undefined ? {} : { tlsSettings: values.tlsSettings }),
          ...(values.ruleSettings === undefined ? {} : { ruleSettings: values.ruleSettings }),
          dnsSettings: values.dnsSettings,
        },
      };
    }
    if (values.type === 'trojan') {
      return {
        type: values.type,
        data: {
          ...common,
          network: values.network,
          network_settings: values.network_settings,
          ...(values.allow_insecure === undefined ? {} : { allow_insecure: values.allow_insecure }),
          ...(values.server_name === undefined ? {} : { server_name: values.server_name }),
        },
      };
    }
    if (values.type === 'hysteria') {
      return {
        type: values.type,
        data: {
          ...common,
          version: values.version,
          ...(values.up_mbps === undefined ? {} : { up_mbps: values.up_mbps }),
          ...(values.down_mbps === undefined ? {} : { down_mbps: values.down_mbps }),
          ...(values.obfs === undefined ? {} : { obfs: values.obfs }),
          ...(values.obfs_password === undefined ? {} : { obfs_password: values.obfs_password }),
          ...(values.server_name === undefined ? {} : { server_name: values.server_name }),
          insecure: values.insecure,
        },
      };
    }
    if (values.type === 'tuic') {
      return {
        type: values.type,
        data: {
          ...common,
          ...(values.server_name === undefined ? {} : { server_name: values.server_name }),
          insecure: values.insecure,
          disable_sni: values.disable_sni,
          ...(values.udp_relay_mode === undefined ? {} : { udp_relay_mode: values.udp_relay_mode }),
          zero_rtt_handshake: values.zero_rtt_handshake,
          ...(values.congestion_control === undefined
            ? {}
            : { congestion_control: values.congestion_control }),
        },
      };
    }
    if (values.type === 'vless') {
      return {
        type: values.type,
        data: {
          ...common,
          ...(values.sort === undefined ? {} : { sort: values.sort }),
          tls: values.tls,
          ...(values.tls_settings === undefined ? {} : { tls_settings: values.tls_settings }),
          ...(values.flow === undefined ? {} : { flow: values.flow }),
          network: values.network,
          network_settings: values.network_settings,
          ...(values.encryption === undefined ? {} : { encryption: values.encryption }),
          ...(values.encryption_settings === undefined
            ? {}
            : { encryption_settings: values.encryption_settings }),
        },
      };
    }
    if (values.type === 'anytls') {
      return {
        type: values.type,
        data: {
          ...common,
          ...(values.server_name === undefined ? {} : { server_name: values.server_name }),
          insecure: values.insecure,
          ...(values.padding_scheme === undefined ? {} : { padding_scheme: values.padding_scheme }),
        },
      };
    }

    if (values.config.protocol === '') return z.NEVER;
    const config = values.config;
    const configCommon = {
      protocol: config.protocol,
      tls: config.tls,
      network: config.network,
      network_settings: config.network_settings,
      disable_sni: config.disable_sni,
      zero_rtt_handshake: config.zero_rtt_handshake,
    };
    const protocolData = (() => {
      if (config.protocol === 'shadowsocks') {
        return {
          ...configCommon,
          protocol: config.protocol,
          ...(config.cipher === undefined ? {} : { cipher: config.cipher }),
        };
      }
      if (config.protocol === 'vless') {
        return {
          ...configCommon,
          protocol: config.protocol,
          ...(config.tls_settings === undefined ? {} : { tls_settings: config.tls_settings }),
          ...(config.flow === undefined ? {} : { flow: config.flow }),
          ...(config.encryption === undefined ? {} : { encryption: config.encryption }),
          ...(config.encryption_settings === undefined
            ? {}
            : { encryption_settings: config.encryption_settings }),
        };
      }
      if (config.protocol === 'tuic') {
        return {
          ...configCommon,
          protocol: config.protocol,
          ...(config.tls_settings === undefined ? {} : { tls_settings: config.tls_settings }),
          ...(config.udp_relay_mode === undefined ? {} : { udp_relay_mode: config.udp_relay_mode }),
          ...(config.congestion_control === undefined
            ? {}
            : { congestion_control: config.congestion_control }),
        };
      }
      if (config.protocol === 'hysteria2') {
        return {
          ...configCommon,
          protocol: config.protocol,
          ...(config.tls_settings === undefined ? {} : { tls_settings: config.tls_settings }),
          ...(config.up_mbps === undefined ? {} : { up_mbps: config.up_mbps }),
          ...(config.down_mbps === undefined ? {} : { down_mbps: config.down_mbps }),
          ...(config.obfs === undefined ? {} : { obfs: config.obfs }),
          ...(config.obfs_password === undefined ? {} : { obfs_password: config.obfs_password }),
        };
      }
      if (config.protocol === 'anytls') {
        return {
          ...configCommon,
          protocol: config.protocol,
          ...(config.tls_settings === undefined ? {} : { tls_settings: config.tls_settings }),
          ...(config.padding_scheme === undefined ? {} : { padding_scheme: config.padding_scheme }),
        };
      }
      return {
        ...configCommon,
        protocol: config.protocol,
        ...(config.tls_settings === undefined ? {} : { tls_settings: config.tls_settings }),
      };
    })();

    return {
      type: values.type,
      data: {
        ...common,
        ...(values.sort === undefined ? {} : { sort: values.sort }),
        ...(values.listen_ip === undefined ? {} : { listen_ip: values.listen_ip }),
        ...protocolData,
      },
    };
  });

export type ServerNodeEditorValues = z.input<typeof serverNodeFormSchema>;
export type ServerNodeSaveRequest = z.output<typeof serverNodeFormSchema>;
export type V2nodeEditorValues = Extract<ServerNodeEditorValues, { type: 'v2node' }>;
export type V2nodeProtocol = Exclude<V2nodeEditorValues['config']['protocol'], ''>;

function v2nodeProtocolDefaults(protocol: V2nodeProtocol): V2nodeEditorValues['config'] {
  const common: {
    tls: 0 | 1;
    network: 'tcp';
    network_settings: null;
    disable_sni: 0;
    zero_rtt_handshake: 0;
  } = {
    tls: ['anytls', 'hysteria2', 'trojan', 'tuic'].includes(protocol) ? 1 : 0,
    network: 'tcp',
    network_settings: null,
    disable_sni: 0,
    zero_rtt_handshake: 0,
  };

  if (protocol === 'shadowsocks') {
    return { ...common, protocol, cipher: 'aes-128-gcm' };
  }
  if (protocol === 'tuic') {
    return {
      ...common,
      protocol,
      udp_relay_mode: 'native',
      congestion_control: 'cubic',
    };
  }
  if (protocol === 'vmess') return { ...common, protocol };
  if (protocol === 'vless') return { ...common, protocol };
  if (protocol === 'trojan') return { ...common, protocol };
  if (protocol === 'hysteria2') return { ...common, protocol };
  return { ...common, protocol };
}

export function switchV2nodeProtocol(
  values: V2nodeEditorValues,
  protocol: V2nodeProtocol,
): V2nodeEditorValues {
  return { ...values, config: v2nodeProtocolDefaults(protocol) };
}
