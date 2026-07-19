import type { output } from 'zod';
import type { ApiClient } from '../../client';
import type { serverRouteActionSchema, serverTypeNameSchema } from '../../contracts';
import {
  arraySchema,
  createdIdSchema,
  noContentSchema,
  serverGroupSchema,
  serverNodeSchema,
  serverRouteSchema,
} from '../../contracts';
import type { QueryRequestConfig } from './shared';

export type ServerNode = output<typeof serverNodeSchema>;

/** GET /{secure_path}/nodes — dialect v2 bare array (§6.7, W13). The rows
 * carry live node credentials, so the read is step-up gated in the backend
 * (the client attaches `x-v2board-step-up` globally when a grant is held). */
export const fetchServerNodes = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/nodes'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(serverNodeSchema),
    ...config,
  });

/** POST /{secure_path}/nodes/sort `{<type>: {<id>: sort}}` — 204 (§6.7); the
 * legacy JSON body shape is kept as-is. */
export const sortServerNodes = (
  client: ApiClient,
  payload: Record<string, Record<string | number, number>>,
) =>
  client.request({
    url: client.resolveAdminPath('/nodes/sort'),
    method: 'POST',
    dialect: 'v2',
    data: payload,
    responseSchema: noContentSchema,
  });

export type ServerGroup = output<typeof serverGroupSchema>;

export interface SaveServerGroupPayload {
  id?: number;
  name: string;
}

/** GET /{secure_path}/server-groups — dialect v2 bare array (§6.7, W13). */
export const fetchServerGroups = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/server-groups'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(serverGroupSchema),
    ...config,
  });

/**
 * POST /{secure_path}/server-groups (201 `{id}`) / PATCH `server-groups/{id}`
 * (204) — the dialect-v2 upsert split (§6.7, W13); the one-field `{name}` body
 * is required in both verbs.
 */
export const saveServerGroup = (client: ApiClient, { id, name }: SaveServerGroupPayload) =>
  id === undefined
    ? client.request({
        url: client.resolveAdminPath('/server-groups'),
        method: 'POST',
        dialect: 'v2',
        data: { name },
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/server-groups/${id}`),
        method: 'PATCH',
        dialect: 'v2',
        data: { name },
        responseSchema: noContentSchema,
      });

/** DELETE /{secure_path}/server-groups/{id} — 204; a still-referenced group is
 * the 400 `server_group_in_use` problem (§6.7). */
export const dropServerGroup = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/server-groups/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

export type ServerRouteAction = output<typeof serverRouteActionSchema>;
export type ServerRoute = output<typeof serverRouteSchema>;

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
  client.request({
    url: client.resolveAdminPath('/server-routes'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(serverRouteSchema),
    ...config,
  });

/**
 * POST /{secure_path}/server-routes (201 `{id}`) / PATCH `server-routes/{id}`
 * (204) — the dialect-v2 upsert split (§6.7, W13). The `ROUTE_ACTIONS`
 * vocabulary is unchanged; `action_value` is the one §4.4 nullable field.
 */
export const saveServerRoute = (client: ApiClient, { id, ...data }: SaveServerRoutePayload) =>
  id === undefined
    ? client.request({
        url: client.resolveAdminPath('/server-routes'),
        method: 'POST',
        dialect: 'v2',
        data,
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/server-routes/${id}`),
        method: 'PATCH',
        dialect: 'v2',
        data,
        responseSchema: noContentSchema,
      });

/** DELETE /{secure_path}/server-routes/{id} — 204 (§6.7). */
export const dropServerRoute = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/server-routes/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

export type ServerTypeName = output<typeof serverTypeNameSchema>;

type ServerPayloadScalar = string | number;
type ServerPayloadBinary = 0 | 1 | '0' | '1';
type ServerPayloadSecurity = ServerPayloadBinary | 2 | '2';
type ServerJsonContainer = Record<string, unknown> | unknown[] | null;

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
  tls_settings?: ServerJsonContainer;
  tlsSettings?: ServerJsonContainer;
  network?: string;
  network_settings?: ServerJsonContainer;
  networkSettings?: ServerJsonContainer;
  ruleSettings?: ServerJsonContainer;
  dnsSettings?: ServerJsonContainer;
  flow?: 'xtls-rprx-vision' | null;
  encryption?: string | null;
  encryption_settings?: ServerJsonContainer;
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
  padding_scheme?: string | null;
}

export interface SaveServerRequest {
  type: ServerTypeName;
  data: SaveServerPayload;
}

/** §6.7 plain integer/number wire fields (JSON numbers on the modern wire). */
const SERVER_NUMBER_FIELDS = new Set(['port', 'server_port', 'tls', 'version', 'rate']);
/** §6.7 nullable integers under §4.4 (empty input is an explicit clear). */
const SERVER_NULLABLE_NUMBER_FIELDS = new Set(['parent_id', 'sort', 'up_mbps', 'down_mbps']);
/** §4.1 boolean flags the legacy form spelled 0/1. */
const SERVER_FLAG_FIELDS = new Set([
  'show',
  'allow_insecure',
  'insecure',
  'disable_sni',
  'zero_rtt_handshake',
]);

/**
 * Serialize a tolerant form payload to the §6.7 dialect-v2 protocol body:
 * ports/rate/tls/version as JSON numbers, 0/1 flags as booleans, id arrays as
 * integer arrays, `padding_scheme` as its decoded JSON container, and the R22
 * camelCase vmess settings keys passed through exactly as spelled. Field
 * presence is preserved so the legacy `param_present` gates map 1:1 onto the
 * §4.4 tri-state (absent retains, null clears, value sets).
 */
function serializeServerBody(data: SaveServerPayload): Record<string, unknown> {
  const body: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(data)) {
    if (key === 'id' || value === undefined) continue;
    if (key === 'group_id') {
      body[key] = (value as ServerPayloadScalar[]).map(Number);
    } else if (key === 'route_id') {
      body[key] = value === null ? null : (value as ServerPayloadScalar[]).map(Number);
    } else if (SERVER_NULLABLE_NUMBER_FIELDS.has(key)) {
      body[key] = value === null || value === '' ? null : Number(value);
    } else if (SERVER_NUMBER_FIELDS.has(key)) {
      body[key] = Number(value);
    } else if (SERVER_FLAG_FIELDS.has(key)) {
      body[key] = value === 1 || value === '1';
    } else if (key === 'padding_scheme') {
      body[key] = parsePaddingScheme(value as string | null);
    } else {
      body[key] = value;
    }
  }
  return body;
}

function parsePaddingScheme(value: string | null): unknown {
  if (value === null || value.trim() === '') return null;
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

/**
 * POST /{secure_path}/servers/{type} (201 `{id}`) / PATCH `servers/{type}/{id}`
 * (204) — the dialect-v2 upsert split for the eight protocol matrices
 * (§6.7, W13).
 */
export const saveServer = (client: ApiClient, type: ServerTypeName, data: SaveServerPayload) =>
  data.id === undefined
    ? client.request({
        url: client.resolveAdminPath(`/servers/${type}`),
        method: 'POST',
        dialect: 'v2',
        data: serializeServerBody(data),
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/servers/${type}/${data.id}`),
        method: 'PATCH',
        dialect: 'v2',
        data: serializeServerBody(data),
        responseSchema: noContentSchema,
      });

/** DELETE /{secure_path}/servers/{type}/{id} — 204 (§6.7). */
export const dropServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/servers/${type}/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** PATCH /{secure_path}/servers/{type}/{id} `{show}` — the merged legacy
 * show toggle (§6.7). */
export const showServer = (client: ApiClient, type: ServerTypeName, id: number, show: boolean) =>
  client.request({
    url: client.resolveAdminPath(`/servers/${type}/${id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: { show },
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/servers/{type}/{id}/copy — 201 bare `{id}` of the new
 * copy (§6.7). */
export const copyServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/servers/${type}/${id}/copy`),
    method: 'POST',
    dialect: 'v2',
    responseSchema: createdIdSchema,
  });
