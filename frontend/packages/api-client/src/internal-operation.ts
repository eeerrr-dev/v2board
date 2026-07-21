import type { InternalApiOperationMap } from '@v2board/types';
import type { input, ZodType } from 'zod';
import type { ApiClient, ApiRequestConfig, BinaryApiResponse } from './client';
import { internalApiOperations, internalApiPath } from './generated/internal-api';

export type InternalOperationName = keyof InternalApiOperationMap &
  keyof typeof internalApiOperations;

type Operation<Name extends InternalOperationName> = InternalApiOperationMap[Name];
type OperationParameters<Name extends InternalOperationName> = Operation<Name>['parameters'];
type RequiredKeys<Value> = {
  [Key in keyof Value]-?: Record<string, never> extends Pick<Value, Key> ? never : Key;
}[keyof Value];

type PublicPathParameters<Value> = Value extends object ? Omit<Value, 'secure_path'> : Value;

type PathOptions<Name extends InternalOperationName> =
  OperationParameters<Name> extends { path: infer Path }
    ? keyof PublicPathParameters<Path> extends never
      ? { path?: never }
      : { path: PublicPathParameters<Path> }
    : { path?: never };

type QueryOptions<Name extends InternalOperationName> =
  OperationParameters<Name> extends { query: infer Query }
    ? [RequiredKeys<Query>] extends [never]
      ? { query?: Query }
      : { query: Query }
    : { query?: never };

type DataOptions<Name extends InternalOperationName> =
  Operation<Name>['requestRequired'] extends true
    ? { data: Operation<Name>['request'] }
    : Operation<Name>['request'] extends undefined
      ? { data?: never }
      : { data?: Operation<Name>['request'] };

type TransportOptions = Omit<
  ApiRequestConfig,
  | 'data'
  | 'dialect'
  | 'expectedStatus'
  | 'method'
  | 'params'
  | 'responseSchema'
  | 'responseType'
  | 'url'
> & {
  /**
   * Exceptional route override for activation probing through a just-written
   * dynamic admin prefix. The method, body, query and response contracts still
   * come from the named generated operation.
   */
  contractUrlOverride?: string;
};

export type InternalOperationRequestOptions<Name extends InternalOperationName> = TransportOptions &
  PathOptions<Name> &
  QueryOptions<Name> &
  DataOptions<Name>;

type RuntimeOperation<Name extends InternalOperationName> = (typeof internalApiOperations)[Name];
type JsonResponseSchema<Name extends InternalOperationName> =
  RuntimeOperation<Name>['jsonResponseSchema'] extends ZodType
    ? RuntimeOperation<Name>['jsonResponseSchema']
    : never;

interface RuntimeOperationShape {
  method: string;
  path: string;
  adminPath?: string;
  parameters: Partial<Record<'path' | 'query', ZodType>>;
  requestRequired: boolean;
  requestSchema: ZodType;
  responseSchema: ZodType;
  jsonResponseSchema: ZodType;
  successResponses: Record<number, unknown>;
}

interface PreparedOperation {
  config: Omit<ApiRequestConfig, 'responseType'> & { expectedStatus: readonly number[] };
  operation: RuntimeOperationShape;
}

/**
 * Execute one modern internal operation from its generated OpenAPI contract.
 * Request bodies, path/query parameters, exact success statuses and response
 * bodies are all validated by that operation; endpoint wrappers provide only
 * product-level mapping and ergonomics.
 */
export function requestInternal<Name extends InternalOperationName>(
  client: ApiClient,
  name: Name,
  options: InternalOperationRequestOptions<Name>,
): Promise<Operation<Name>['response']> {
  const { config, operation } = prepareOperation(client, name, options);
  return client.request({
    ...config,
    responseSchema: operation.responseSchema,
  }) as Promise<Operation<Name>['response']>;
}

/** Execute a generated CSV-or-JSON operation without hand-authoring its JSON arm. */
export function requestInternalBinary<Name extends InternalOperationName>(
  client: ApiClient,
  name: Name,
  options: InternalOperationRequestOptions<Name>,
): Promise<BinaryApiResponse<JsonResponseSchema<Name>>> {
  const { config, operation } = prepareOperation(client, name, options);
  return client.requestBinary({
    ...config,
    jsonResponseSchema: operation.jsonResponseSchema,
  }) as Promise<BinaryApiResponse<JsonResponseSchema<Name>>>;
}

function prepareOperation<Name extends InternalOperationName>(
  client: ApiClient,
  name: Name,
  rawOptions: InternalOperationRequestOptions<Name>,
): PreparedOperation {
  const operation = internalApiOperations[name] as RuntimeOperationShape;
  const options = rawOptions as TransportOptions & {
    data?: unknown;
    path?: unknown;
    query?: unknown;
  };
  const { contractUrlOverride, data, path, query, ...transport } = options;

  const pathParameters = parsePathParameters(operation, path);
  const queryParameters = parseParameterGroup(operation, 'query', query);
  const parsedData =
    operation.requestRequired || data !== undefined
      ? operation.requestSchema.parse(data)
      : undefined;

  const template = operation.adminPath ?? internalClientPath(operation.path);
  const expandedPath = internalApiPath(
    template,
    (pathParameters ?? {}) as Readonly<Record<string, string | number>>,
  );
  const url =
    contractUrlOverride ??
    (operation.adminPath === undefined ? expandedPath : client.resolveAdminPath(expandedPath));
  const expectedStatus = Object.keys(operation.successResponses).map(Number);

  return {
    operation,
    config: {
      ...transport,
      url,
      method: operation.method,
      dialect: 'v2',
      expectedStatus,
      ...(queryParameters === undefined ? {} : { params: queryParameters }),
      ...(parsedData === undefined ? {} : { data: parsedData }),
    },
  };
}

function parsePathParameters(operation: RuntimeOperationShape, value: unknown): unknown {
  if (operation.adminPath === undefined) {
    return parseParameterGroup(operation, 'path', value);
  }
  const publicValue = value === undefined ? {} : value;
  const parsed = parseParameterGroup(operation, 'path', {
    secure_path: '__runtime_admin_prefix__',
    ...(publicValue as object),
  });
  if (parsed === undefined) return parsed;
  const { secure_path: _securePath, ...publicParameters } = parsed as Record<string, unknown>;
  return publicParameters;
}

function parseParameterGroup(
  operation: RuntimeOperationShape,
  location: 'path' | 'query',
  value: unknown,
): unknown {
  const schema = operation.parameters[location];
  if (schema) return schema.parse(value ?? {});
  if (value !== undefined) {
    throw new TypeError(
      `Operation ${operation.method} ${operation.path} has no ${location} parameters`,
    );
  }
  return undefined;
}

function internalClientPath(path: string): string {
  const prefix = '/api/v1';
  if (!path.startsWith(`${prefix}/`)) {
    throw new TypeError(`Internal operation path is outside ${prefix}: ${path}`);
  }
  return path.slice(prefix.length);
}

// Compile-time assertion: the helper consumes Zod inputs while its public
// request DTOs remain the generated OpenAPI operation types.
type _GeneratedRequestInputsStayCompatible = {
  [Name in InternalOperationName]: input<
    RuntimeOperation<Name>['requestSchema']
  > extends Operation<Name>['request']
    ? true
    : never;
};
