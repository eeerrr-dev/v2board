import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

const arguments_ = process.argv.slice(2);
const check = arguments_.includes('--check');
const rootArgument = arguments_.find((value) => value.startsWith('--root='));
const root = path.resolve(rootArgument?.slice('--root='.length) ?? process.cwd());
const specPath = path.join(root, 'packages/api-client/openapi/internal-api.openapi.json');
const typesPath = path.join(root, 'packages/types/src/generated/internal-api.ts');
const runtimePath = path.join(root, 'packages/api-client/src/generated/internal-api.ts');

const spec = JSON.parse(await readFile(specPath, 'utf8'));
assert.equal(spec.openapi, '3.1.0', 'the internal contract must use OpenAPI 3.1');
assert.ok(spec.components?.schemas, 'the internal contract has no component schemas');

const schemas = spec.components.schemas;

function refName(reference) {
  const prefix = '#/components/schemas/';
  assert.ok(reference.startsWith(prefix), `unsupported reference: ${reference}`);
  return reference.slice(prefix.length);
}

function generatedTypeName(name) {
  return `InternalApi${name}`;
}

function generatedSchemaName(name) {
  return `internalApi${name[0].toUpperCase()}${name.slice(1)}Schema`;
}

function withoutNull(schema) {
  if (!Array.isArray(schema.type) || !schema.type.includes('null')) return null;
  const types = schema.type.filter((type) => type !== 'null');
  assert.equal(types.length, 1, `unsupported nullable schema: ${JSON.stringify(schema)}`);
  return { ...schema, type: types[0] };
}

function typeExpression(schema) {
  if (schema.$ref) return generatedTypeName(refName(schema.$ref));
  const nonNull = withoutNull(schema);
  if (nonNull) return `${typeExpression(nonNull)} | null`;
  if (schema.enum) return schema.enum.map((value) => JSON.stringify(value)).join(' | ');
  switch (schema.type) {
    case 'boolean':
      return 'boolean';
    case 'integer':
    case 'number':
      return 'number';
    case 'string':
      return 'string';
    case 'array': {
      const item = typeExpression(schema.items);
      return item.includes(' | ') ? `Array<${item}>` : `${item}[]`;
    }
    case 'object': {
      const required = new Set(schema.required ?? []);
      const properties = Object.entries(schema.properties ?? {})
        .sort(([left], [right]) => left.localeCompare(right))
        .map(
          ([name, property]) =>
            `${JSON.stringify(name)}${required.has(name) ? '' : '?'}: ${typeExpression(property)}`,
        );
      if (schema.additionalProperties && schema.additionalProperties !== false) {
        properties.push(
          `[key: string]: ${schema.additionalProperties === true ? 'unknown' : typeExpression(schema.additionalProperties)}`,
        );
      }
      return `{ ${properties.join('; ')} }`;
    }
    default:
      throw new Error(`unsupported OpenAPI type schema: ${JSON.stringify(schema)}`);
  }
}

function zodExpression(schema) {
  if (schema.$ref) return generatedSchemaName(refName(schema.$ref));
  const nonNull = withoutNull(schema);
  if (nonNull) return `${zodExpression(nonNull)}.nullable()`;
  if (schema.enum) {
    if (schema.enum.every((value) => typeof value === 'string')) {
      return `z.enum([${schema.enum.map((value) => JSON.stringify(value)).join(', ')}])`;
    }
    return `z.union([${schema.enum.map((value) => `z.literal(${JSON.stringify(value)})`).join(', ')}])`;
  }
  let expression;
  switch (schema.type) {
    case 'boolean':
      expression = 'z.boolean()';
      break;
    case 'integer':
      expression = 'z.number().int()';
      if (schema.format === 'int32') {
        expression += '.min(-2147483648).max(2147483647)';
      }
      break;
    case 'number':
      expression = 'z.number()';
      break;
    case 'string':
      expression =
        schema.format === 'date-time' ? 'z.iso.datetime({ offset: true })' : 'z.string()';
      break;
    case 'array':
      expression = `z.array(${zodExpression(schema.items)})`;
      break;
    case 'object': {
      const required = new Set(schema.required ?? []);
      const properties = Object.entries(schema.properties ?? {})
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([name, property]) => {
          const validator = zodExpression(property);
          return `  ${JSON.stringify(name)}: ${validator}${required.has(name) ? '' : '.optional()'},`;
        });
      expression = `z.strictObject({\n${properties.join('\n')}\n})`;
      break;
    }
    default:
      throw new Error(`unsupported OpenAPI zod schema: ${JSON.stringify(schema)}`);
  }
  if (schema.minimum !== undefined) expression += `.min(${schema.minimum})`;
  if (schema.maximum !== undefined) expression += `.max(${schema.maximum})`;
  return expression;
}

function schemaDependencies(schema, output = new Set()) {
  if (schema.$ref) output.add(refName(schema.$ref));
  if (schema.items) schemaDependencies(schema.items, output);
  for (const property of Object.values(schema.properties ?? {})) {
    schemaDependencies(property, output);
  }
  return output;
}

function topologicalSchemaNames() {
  const ordered = [];
  const visiting = new Set();
  const visited = new Set();
  function visit(name) {
    if (visited.has(name)) return;
    assert.ok(
      !visiting.has(name),
      `recursive schema requires an explicit generator policy: ${name}`,
    );
    visiting.add(name);
    for (const dependency of [...schemaDependencies(schemas[name])].sort()) visit(dependency);
    visiting.delete(name);
    visited.add(name);
    ordered.push(name);
  }
  for (const name of Object.keys(schemas).sort()) visit(name);
  return ordered;
}

function operations() {
  const found = [];
  for (const [operationPath, pathItem] of Object.entries(spec.paths ?? {})) {
    for (const method of ['get', 'post', 'put', 'patch', 'delete']) {
      const operation = pathItem[method];
      if (!operation) continue;
      assert.ok(
        operation.operationId,
        `${method.toUpperCase()} ${operationPath} has no operationId`,
      );
      const request = operation.requestBody?.content?.['application/json']?.schema;
      const responseEntries = Object.entries(operation.responses ?? {});
      const ambiguousSuccessStatuses = responseEntries
        .map(([status]) => status)
        .filter((status) => /^2xx$/i.test(status));
      assert.deepEqual(
        ambiguousSuccessStatuses,
        [],
        `${method.toUpperCase()} ${operationPath} must pin an exact success status; ` +
          `ambiguous OpenAPI response keys are unsupported: ${ambiguousSuccessStatuses.join(', ')}`,
      );
      const successResponses = responseEntries.filter(([status]) => /^2\d\d$/.test(status));
      assert.equal(
        successResponses.length,
        1,
        `${method.toUpperCase()} ${operationPath} must declare exactly one supported 2xx response; ` +
          `found ${successResponses.map(([status]) => status).join(', ') || 'none'}`,
      );
      const [[successStatusText, success]] = successResponses;
      const successStatus = Number(successStatusText);
      const response = success?.content?.['application/json']?.schema;
      const adminPrefix = '/api/v1/{secure_path}';
      found.push({
        id: operation.operationId,
        method: method.toUpperCase(),
        path: operationPath,
        adminPath: operationPath.startsWith(adminPrefix)
          ? operationPath.slice(adminPrefix.length)
          : null,
        successStatus,
        request,
        response,
      });
    }
  }
  return found.sort((left, right) => left.id.localeCompare(right.id));
}

function renderTypes() {
  const blocks = [
    '// @generated by scripts/generate-internal-api-contract.mjs; do not edit.',
    '// Source: backend/rust/crates/api-contract.',
    '',
  ];
  for (const name of Object.keys(schemas).sort()) {
    const schema = schemas[name];
    if (schema.type === 'object') {
      const required = new Set(schema.required ?? []);
      blocks.push(`export interface ${generatedTypeName(name)} {`);
      for (const [propertyName, property] of Object.entries(schema.properties ?? {}).sort(
        ([left], [right]) => left.localeCompare(right),
      )) {
        blocks.push(
          `  ${JSON.stringify(propertyName)}${required.has(propertyName) ? '' : '?'}: ${typeExpression(property)};`,
        );
      }
      blocks.push('}', '');
    } else {
      blocks.push(`export type ${generatedTypeName(name)} = ${typeExpression(schema)};`, '');
    }
  }
  blocks.push('export interface InternalApiSchemaMap {');
  for (const name of Object.keys(schemas).sort()) {
    blocks.push(`  ${JSON.stringify(name)}: ${generatedTypeName(name)};`);
  }
  blocks.push('}', '', 'export interface InternalApiOperationMap {');
  for (const operation of operations()) {
    blocks.push(`  ${JSON.stringify(operation.id)}: {`);
    blocks.push(`    method: ${JSON.stringify(operation.method)};`);
    blocks.push(`    path: ${JSON.stringify(operation.path)};`);
    blocks.push(`    successStatus: ${operation.successStatus};`);
    blocks.push(
      `    request: ${operation.request ? typeExpression(operation.request) : 'undefined'};`,
    );
    blocks.push(
      `    response: ${operation.response ? typeExpression(operation.response) : 'undefined'};`,
    );
    blocks.push('  };');
  }
  blocks.push('}');
  return `${blocks.join('\n')}\n`;
}

function renderRuntime() {
  const blocks = [
    '// @generated by scripts/generate-internal-api-contract.mjs; do not edit.',
    '// Source: backend/rust/crates/api-contract.',
    "import { z } from 'zod';",
    '',
  ];
  for (const name of topologicalSchemaNames()) {
    blocks.push(`export const ${generatedSchemaName(name)} = ${zodExpression(schemas[name])};`, '');
  }
  blocks.push('export const internalApiOperations = {');
  for (const operation of operations()) {
    blocks.push(`  ${JSON.stringify(operation.id)}: {`);
    blocks.push(`    method: ${JSON.stringify(operation.method)},`);
    blocks.push(`    path: ${JSON.stringify(operation.path)},`);
    blocks.push(`    successStatus: ${operation.successStatus},`);
    if (operation.adminPath !== null) {
      blocks.push(`    adminPath: ${JSON.stringify(operation.adminPath)},`);
    }
    blocks.push(
      `    requestSchema: ${operation.request ? zodExpression(operation.request) : 'z.undefined()'},`,
    );
    blocks.push(
      `    responseSchema: ${operation.response ? zodExpression(operation.response) : 'z.undefined()'},`,
    );
    blocks.push('  },');
  }
  blocks.push(
    '} as const;',
    '',
    '/** Expand a generated OpenAPI path without accepting missing parameters. */',
    'export function internalApiPath(',
    '  template: string,',
    '  parameters: Readonly<Record<string, string | number>> = {},',
    '): string {',
    '  return template.replace(/\\{([^}]+)\\}/g, (_match, name: string) => {',
    '    const value = parameters[name];',
    '    if (value === undefined) throw new TypeError(`Missing internal API path parameter: ${name}`);',
    '    return encodeURIComponent(String(value));',
    '  });',
    '}',
  );
  return `${blocks.join('\n')}\n`;
}

async function emit(file, content) {
  if (check) {
    const existing = await readFile(file, 'utf8');
    assert.equal(existing, content, `${path.relative(root, file)} is stale; regenerate it`);
    return;
  }
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, content);
}

await emit(typesPath, renderTypes());
await emit(runtimePath, renderRuntime());
