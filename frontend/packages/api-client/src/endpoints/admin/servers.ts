import type { InternalApiOperationMap, InternalApiServerRouteAction } from '@v2board/types';
import type { ApiClient } from '../../client';
import {
  internalApiServerEncryptionSettingsSchema,
  internalApiServerNetworkSettingsSchema,
  internalApiServerTlsSettingsSchema,
  internalApiServerWriteRequestSchema,
  internalApiVmessDnsSettingsSchema,
  internalApiVmessRuleSettingsSchema,
} from '../../generated/internal-api';
import { requestInternal } from '../../internal-operation';
import type { QueryRequestConfig } from './shared';

type ArrayItem<Value> = Value extends Array<infer Item> ? Item : never;

type GeneratedServerNode = ArrayItem<InternalApiOperationMap['adminNodesList']['response']>;
export type ServerNode = Omit<GeneratedServerNode, 'online'> & { online: number };

/** GET /{secure_path}/nodes — dialect v2 bare array (§6.7, W13). The rows
 * carry live node credentials, so the read is step-up gated in the backend
 * (the client attaches `x-v2board-step-up` globally when a grant is held). */
export const fetchServerNodes = async (
  client: ApiClient,
  config?: QueryRequestConfig,
): Promise<ServerNode[]> =>
  (
    await requestInternal(client, 'adminNodesList', {
      ...config,
    })
  ).map((node) => ({ ...node, online: node.online ?? 0 }));

/** POST /{secure_path}/nodes/sort `{<type>: {<id>: sort}}` — 204 (§6.7); the
 * legacy JSON body shape is kept as-is. */
export const sortServerNodes = (
  client: ApiClient,
  payload: Record<string, Record<string | number, number>>,
) =>
  requestInternal(client, 'adminNodesSort', {
    data: payload,
  });

export type ServerGroup = ArrayItem<InternalApiOperationMap['adminServerGroupsList']['response']>;

export interface SaveServerGroupPayload {
  id?: number;
  name: string;
}

/** GET /{secure_path}/server-groups — dialect v2 bare array (§6.7, W13). */
export const fetchServerGroups = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminServerGroupsList', {
    ...config,
  });

/**
 * POST /{secure_path}/server-groups (201 `{id}`) / PATCH `server-groups/{id}`
 * (204) — the dialect-v2 upsert split (§6.7, W13); the one-field `{name}` body
 * is required in both verbs.
 */
export const saveServerGroup = (client: ApiClient, { id, name }: SaveServerGroupPayload) =>
  id === undefined
    ? requestInternal(client, 'adminServerGroupsCreate', {
        data: { name },
      })
    : requestInternal(client, 'adminServerGroupsUpdate', {
        path: { id },
        data: { name },
      });

/** DELETE /{secure_path}/server-groups/{id} — 204; a still-referenced group is
 * the 400 `server_group_in_use` problem (§6.7). */
export const dropServerGroup = (client: ApiClient, id: number) =>
  requestInternal(client, 'adminServerGroupsDelete', { path: { id } });

export type ServerRouteAction = InternalApiServerRouteAction;
export type ServerRoute = ArrayItem<InternalApiOperationMap['adminServerRoutesList']['response']>;

export interface SaveServerRoutePayload {
  id?: number;
  remarks: string;
  match: string[];
  action: ServerRouteAction;
  action_value: string | null;
}

/** GET /{secure_path}/server-routes — dialect v2 bare array; `match` is
 * always an array (§6.7, W13). */
export const fetchServerRoutes = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminServerRoutesList', {
    ...config,
  });

/**
 * POST /{secure_path}/server-routes (201 `{id}`) / PATCH `server-routes/{id}`
 * (204) — the dialect-v2 upsert split (§6.7, W13). The `ROUTE_ACTIONS`
 * vocabulary is unchanged; `action_value` is the one §4.4 nullable field.
 */
export const saveServerRoute = (client: ApiClient, { id, ...data }: SaveServerRoutePayload) =>
  id === undefined
    ? requestInternal(client, 'adminServerRoutesCreate', {
        data,
      })
    : requestInternal(client, 'adminServerRoutesUpdate', {
        path: { id },
        data,
      });

/** DELETE /{secure_path}/server-routes/{id} — 204 (§6.7). */
export const dropServerRoute = (client: ApiClient, id: number) =>
  requestInternal(client, 'adminServerRoutesDelete', { path: { id } });

export type ServerTypeName = ServerNode['type'];

type ServerPayloadScalar = string | number;
type ServerPayloadBinary = 0 | 1 | '0' | '1';
type ServerPayloadSecurity = ServerPayloadBinary | 2 | '2';
type GeneratedServerWrite = InternalApiOperationMap['adminServersCreate']['request'];
type PresentServerField<Key extends keyof GeneratedServerWrite> = Exclude<
  GeneratedServerWrite[Key],
  null | undefined
>;

export type ServerNetworkSettings = PresentServerField<'network_settings'>;
export type ServerTlsSettings = PresentServerField<'tls_settings'>;
export type ServerEncryptionSettings = PresentServerField<'encryption_settings'>;
export type VmessRuleSettings = PresentServerField<'ruleSettings'>;
export type VmessDnsSettings = PresentServerField<'dnsSettings'>;

export const serverNetworkSettingsSchema = internalApiServerNetworkSettingsSchema;
export const serverTlsSettingsSchema = internalApiServerTlsSettingsSchema;
export const serverEncryptionSettingsSchema = internalApiServerEncryptionSettingsSchema;
export const vmessRuleSettingsSchema = internalApiVmessRuleSettingsSchema;
export const vmessDnsSettingsSchema = internalApiVmessDnsSettingsSchema;

/**
 * Exact public request surface shared by the eight server save endpoints.
 * Protocol-specific keys are optional because the endpoint path supplies the
 * discriminator, but arbitrary keys and response-only fields are rejected.
 */
export interface SaveServerPayload {
  id?: number;
  name: string;
  group_id: ServerPayloadScalar[];
  route_id?: ServerPayloadScalar[] | null;
  parent_id?: ServerPayloadScalar | null;
  host: string;
  port: ServerPayloadScalar;
  server_port: ServerPayloadScalar;
  tags?: string[] | null;
  rate: ServerPayloadScalar;
  show?: ServerPayloadBinary;
  sort?: ServerPayloadScalar | null;
  listen_ip?: string | null;
  protocol?: 'shadowsocks' | 'vmess' | 'vless' | 'trojan' | 'tuic' | 'hysteria2' | 'anytls';
  tls?: ServerPayloadSecurity;
  tls_settings?: ServerTlsSettings | null;
  tlsSettings?: ServerTlsSettings | null;
  network?: string;
  network_settings?: ServerNetworkSettings | null;
  networkSettings?: ServerNetworkSettings | null;
  ruleSettings?: VmessRuleSettings | null;
  dnsSettings?: VmessDnsSettings | null;
  flow?: 'xtls-rprx-vision' | null;
  encryption?: string | null;
  encryption_settings?: ServerEncryptionSettings | null;
  cipher?: string | null;
  obfs?: string | null;
  obfs_settings?: {
    path?: ServerPayloadScalar | null;
    host?: ServerPayloadScalar | null;
  } | null;
  obfs_password?: string | null;
  server_name?: string | null;
  allow_insecure?: ServerPayloadBinary;
  insecure?: ServerPayloadBinary;
  version?: 1 | 2 | '1' | '2';
  up_mbps?: ServerPayloadScalar | null;
  down_mbps?: ServerPayloadScalar | null;
  disable_sni?: ServerPayloadBinary;
  udp_relay_mode?: string | null;
  zero_rtt_handshake?: ServerPayloadBinary;
  congestion_control?: string | null;
  padding_scheme?: string[] | null;
}

export interface SaveServerRequest {
  type: ServerTypeName;
  data: SaveServerPayload;
}

/**
 * Serialize a tolerant form payload to the §6.7 dialect-v2 protocol body:
 * ports/rate/tls/version as JSON numbers, 0/1 flags as booleans, id arrays as
 * integer arrays, `padding_scheme` as its decoded JSON container, and the R22
 * camelCase vmess settings keys passed through exactly as spelled. Field
 * presence is preserved so the legacy `param_present` gates map 1:1 onto the
 * §4.4 tri-state (absent retains, null clears, value sets).
 */
function optionalNumber(value: ServerPayloadScalar | null | undefined): number | null | undefined {
  return value === undefined ? undefined : value === null || value === '' ? null : Number(value);
}

function optionalFlag(value: ServerPayloadBinary | undefined): boolean | undefined {
  return value === undefined ? undefined : value === 1 || value === '1';
}

function serializeServerBody(data: SaveServerPayload): GeneratedServerWrite {
  const { id: _id, ...fields } = data;
  return internalApiServerWriteRequestSchema.parse({
    ...fields,
    group_id: data.group_id.map(Number),
    route_id: data.route_id?.map(Number) ?? data.route_id,
    parent_id: optionalNumber(data.parent_id),
    port: Number(data.port),
    server_port: Number(data.server_port),
    rate: Number(data.rate),
    sort: optionalNumber(data.sort),
    tls: optionalNumber(data.tls),
    version: optionalNumber(data.version),
    up_mbps: optionalNumber(data.up_mbps),
    down_mbps: optionalNumber(data.down_mbps),
    show: optionalFlag(data.show),
    allow_insecure: optionalFlag(data.allow_insecure),
    insecure: optionalFlag(data.insecure),
    disable_sni: optionalFlag(data.disable_sni),
    zero_rtt_handshake: optionalFlag(data.zero_rtt_handshake),
    obfs_settings:
      data.obfs_settings == null
        ? data.obfs_settings
        : {
            ...(data.obfs_settings.path === undefined
              ? {}
              : {
                  path: data.obfs_settings.path === null ? null : String(data.obfs_settings.path),
                }),
            ...(data.obfs_settings.host === undefined
              ? {}
              : {
                  host: data.obfs_settings.host === null ? null : String(data.obfs_settings.host),
                }),
          },
  });
}

/**
 * POST /{secure_path}/servers/{type} (201 `{id}`) / PATCH `servers/{type}/{id}`
 * (204) — the dialect-v2 upsert split for the eight protocol matrices
 * (§6.7, W13).
 */
export const saveServer = (client: ApiClient, type: ServerTypeName, data: SaveServerPayload) =>
  data.id === undefined
    ? requestInternal(client, 'adminServersCreate', {
        path: { type },
        data: serializeServerBody(data),
      })
    : requestInternal(client, 'adminServersUpdate', {
        path: { type, id: data.id },
        data: serializeServerBody(data),
      });

/** DELETE /{secure_path}/servers/{type}/{id} — 204 (§6.7). */
export const dropServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  requestInternal(client, 'adminServersDelete', { path: { type, id } });

/** PATCH /{secure_path}/servers/{type}/{id} `{show}` — the merged legacy
 * show toggle (§6.7). */
export const showServer = (client: ApiClient, type: ServerTypeName, id: number, show: boolean) =>
  requestInternal(client, 'adminServersUpdate', {
    path: { type, id },
    data: { show },
  });

/** POST /{secure_path}/servers/{type}/{id}/copy — 201 bare `{id}` of the new
 * copy (§6.7). */
export const copyServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  requestInternal(client, 'adminServersCopy', { path: { type, id } });
