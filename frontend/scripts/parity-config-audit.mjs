#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { WORLDS, routeMap } from '../tests/lib/dialect/route-map.mjs';
import { viewports } from '../tests/lib/env.mjs';
import { SOURCE_ONLY_INTERACTION_ALLOWLIST } from '../tests/lib/interaction-contract.mjs';
import { interactions } from '../tests/lib/interaction-scenarios.mjs';
import { scenariosByLabel } from '../tests/lib/scenario-meta.mjs';
import { GROUP_NAMES, groupOf } from '../tests/lib/spec-groups.mjs';
import { auditUiSync } from './ui-sync-audit.mjs';

if (isMainModule()) {
  const result = await auditParityConfig();

  if (result.failures.length > 0) {
    console.error(`Parity config audit failed:\n${result.failures.join('\n\n')}`);
    process.exit(1);
  }

  console.log(formatAuditSuccess(result));
}

export async function auditParityConfig(projectRoot = getDefaultProjectRoot()) {
  const makefilePath = resolve(projectRoot, 'Makefile');
  const userAppPath = resolve(projectRoot, 'frontend/apps/user/src/App.tsx');
  const adminAppPath = resolve(projectRoot, 'frontend/apps/admin/src/App.tsx');
  const assertionPath = resolve(projectRoot, 'frontend/tests/lib/assert-useful.mjs');

  const [makefile, userApp, adminApp, assertionSource] = await Promise.all([
    readFile(makefilePath, 'utf8'),
    readFile(userAppPath, 'utf8'),
    readFile(adminAppPath, 'utf8'),
    readFile(assertionPath, 'utf8'),
  ]);
  const uiSync = await auditUiSync(projectRoot);

  // The parity modules are the single source of truth for the interaction lane.
  // The old text-parse of visual-parity.mjs retired with the driver; the
  // Playwright specs now import these same modules, so auditing them keeps the
  // Makefile scope list and route coverage honest against what actually runs.
  const scenarios = [...scenariosByLabel.values()];
  const scenarioLabels = scenarios.map((scenario) => scenario.label);
  const scenarioPaths = scenarios.map((scenario) => ({
    label: scenario.label,
    route: normalizeScenarioRoute(scenario.path),
    ...(scenario.visualRetired === true ? { visualRetired: true } : {}),
  }));
  const interactionLabels = interactions.map((interaction) => interaction.label);
  const interactionTargets = interactions.map((interaction) => interaction.scenarioLabel);
  const viewportLabels = viewports.map((viewport) => viewport.label);

  const makeInteractionScenarios = readMakeList(makefile, 'INTERACTION_PARITY_SCENARIOS');
  const makeViewports = readMakeList(makefile, 'VISUAL_PARITY_VIEWPORTS');

  const userRoutes = extractRouteArray(userApp, 'USER_ROUTE_PATHS');
  const adminRoutes = extractRouteArray(adminApp, 'ADMIN_ROUTE_PATHS');
  const userAppPublicRoutes = extractObjectArray(
    userApp,
    'USER_ROUTE_GUARD_OPTIONS',
    'publicRoutes',
  );
  const adminAppPublicRoutes = extractObjectArray(
    adminApp,
    'ADMIN_ROUTE_GUARD_OPTIONS',
    'publicRoutes',
  );
  const failures = [
    ...uiSync.failures,
    ...assertUnique('parity scenarios', scenarioLabels),
    ...assertUnique('parity interactions', interactionLabels),
    ...assertUnique('Makefile INTERACTION_PARITY_SCENARIOS', makeInteractionScenarios),
    ...assertSameOrderedList(
      'INTERACTION_PARITY_SCENARIOS',
      makeInteractionScenarios,
      interactionLabels,
    ),
    ...assertSubset('VISUAL_PARITY_VIEWPORTS', makeViewports, viewportLabels),
    ...assertInteractionTargetsExist(scenarioLabels, interactionTargets),
    ...assertSpecGroupCoverage(interactions, GROUP_NAMES),
    ...assertSourceOnlyAllowlist(interactions, SOURCE_ONLY_INTERACTION_ALLOWLIST),
    ...assertUsefulInteractionCoverage(interactions, assertionSource),
    ...assertRouteCoverage(
      'user parity route coverage',
      userRoutes,
      scenarioPaths.filter((scenario) => scenario.label.startsWith('user-')),
      new Set(interactionTargets),
    ),
    ...assertRouteCoverage(
      'admin parity route coverage',
      adminRoutes,
      scenarioPaths.filter((scenario) => scenario.label.startsWith('admin-')),
      new Set(interactionTargets),
    ),
    ...assertSubset('user public routes', userAppPublicRoutes, userRoutes),
    ...assertSubset('admin public routes', adminAppPublicRoutes, adminRoutes),
    ...assertDialectRouteMap(routeMap),
  ];

  return {
    adminRouteCount: adminRoutes.length,
    dialectRouteCount: routeMap.length,
    failures,
    interactionScenarioCount: interactionLabels.length,
    scenarioCount: scenarioLabels.length,
    specGroupCount: GROUP_NAMES.length,
    uiAppSpecificCount: uiSync.appSpecificCount,
    uiSharedPrimitiveCount: uiSync.sharedPrimitiveCount,
    uiSharedStylesheetCount: uiSync.sharedStylesheetCount,
    userRouteCount: userRoutes.length,
    viewportCount: viewportLabels.length,
  };
}

export function formatAuditSuccess(result) {
  const appOnlyNoun = result.uiAppSpecificCount === 1 ? 'primitive' : 'primitives';
  return (
    `Parity config audit OK: ${result.scenarioCount} parity scenarios and ` +
    `${result.interactionScenarioCount} interactions across ${result.specGroupCount} spec groups ` +
    `and ${result.viewportCount} viewports, Makefile INTERACTION_PARITY_SCENARIOS mirrors ` +
    `the interaction modules, and App.tsx route definitions cover ${result.userRouteCount} ` +
    `user routes plus ${result.adminRouteCount} admin routes. The dialect route map carries ` +
    `${result.dialectRouteCount} well-formed two-world rows (incl. the §6.5 admin ticket rows). ` +
    `Source-only exceptions and useful-result assertions are fail-closed. UI sync covers ` +
    `${result.uiSharedPrimitiveCount} shared primitives, ${result.uiSharedStylesheetCount} ` +
    `shared stylesheets, and ${result.uiAppSpecificCount} explicit app-only ${appOnlyNoun}.`
  );
}

export function assertSourceOnlyAllowlist(interactionList, allowlist) {
  return assertSameOrderedList(
    'sourceOnly interaction allowlist',
    interactionList.filter((interaction) => interaction.sourceOnly).map(({ label }) => label),
    [...allowlist],
  );
}

export function assertUsefulInteractionCoverage(interactionList, assertionSource) {
  // Assertions are intentionally composed as direct comparisons, shared
  // `.includes(label)` arrays, and expectation maps. Treat a full interaction
  // label literal in this module as an explicit registration while still
  // supporting deliberate prefix/suffix families.
  const exactLabels = new Set(
    [...assertionSource.matchAll(/(['"])([^'"\r\n]+)\1/g)].map((match) => match[2]),
  );
  const prefixes = [
    ...assertionSource.matchAll(/\blabel\.startsWith\('([^']+)'\)/g),
  ].map((match) => match[1]);
  const suffixes = [
    ...assertionSource.matchAll(/\blabel\.endsWith\('([^']+)'\)/g),
  ].map((match) => match[1]);
  const interactionLabels = interactionList.map(({ label }) => label);
  const covered = (label) =>
    exactLabels.has(label) ||
    prefixes.some((prefix) => label.startsWith(prefix)) ||
    suffixes.some((suffix) => label.endsWith(suffix));
  const missing = interactionLabels.filter((label) => !covered(label));
  const stalePrefixes = prefixes.filter(
    (prefix) => !interactionLabels.some((label) => label.startsWith(prefix)),
  );
  const staleSuffixes = suffixes.filter(
    (suffix) => !interactionLabels.some((label) => label.endsWith(suffix)),
  );
  const failures = [];
  if (missing.length > 0) {
    failures.push(
      `Interactions missing assertUsefulInteraction checks: ${missing.join(', ')}`,
    );
  }
  if (stalePrefixes.length > 0 || staleSuffixes.length > 0) {
    failures.push(
      `assertUsefulInteraction has stale matchers: ${[
        ...stalePrefixes.map((prefix) => `${prefix}*`),
        ...staleSuffixes.map((suffix) => `*${suffix}`),
      ].join(', ')}`,
    );
  }
  return failures;
}

/**
 * Structural audit of the internal-dialect route map (docs/api-dialect.md
 * §13.1). W14 closed the wave series, so every row must be a complete
 * two-world entry, and the §6.5 admin ticket rows (plus their §6.9 staff
 * mirrors) must be present — the interaction lane resolves its intercepted
 * URLs through these rows.
 */
export function assertDialectRouteMap(map) {
  const failures = assertUnique(
    'dialect route map ids',
    map.map((entry) => entry.id),
  );
  for (const entry of map) {
    for (const world of ['legacy', 'modern']) {
      const shape = entry[world];
      if (!shape || typeof shape.method !== 'string' || typeof shape.path !== 'string') {
        failures.push(`dialect route map: ${entry.id} is missing its ${world} shape`);
        continue;
      }
      if (!/^(GET|POST|PATCH|PUT|DELETE)$/.test(shape.method)) {
        failures.push(`dialect route map: ${entry.id} ${world} method ${shape.method} is invalid`);
      }
      if (!shape.path.startsWith('/')) {
        failures.push(`dialect route map: ${entry.id} ${world} path must start with /`);
      }
    }
  }
  if (WORLDS.length !== 2) {
    failures.push(`dialect route map: expected the two-world seam, got ${WORLDS.join(', ')}`);
  }
  const ids = new Set(map.map((entry) => entry.id));
  for (const required of [
    'admin.tickets.list',
    'admin.tickets.get',
    'admin.tickets.replies.create',
    'admin.tickets.close',
    'staff.tickets.list',
    'staff.tickets.get',
    'staff.tickets.replies.create',
    'staff.tickets.close',
  ]) {
    if (!ids.has(required)) {
      failures.push(
        `dialect route map: required §6.5/§6.9 ticket row ${required} is missing`,
      );
    }
  }
  return failures;
}

export function readMakeList(source, name) {
  const lines = source.split(/\r?\n/);
  const prefix = `${name} ?=`;
  const start = lines.findIndex((line) => line.startsWith(prefix));

  if (start === -1) {
    throw new Error(`Missing Makefile variable ${name}`);
  }

  const chunks = [];
  for (let index = start; index < lines.length; index += 1) {
    const rawLine = index === start ? lines[index].slice(prefix.length) : lines[index];
    const hasContinuation = /\\\s*$/.test(rawLine);
    chunks.push(rawLine.replace(/\\\s*$/, '').trim());

    if (!hasContinuation) {
      break;
    }
  }

  return chunks.join(' ').trim().split(/\s+/).filter(Boolean);
}

export function extractBlock(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  if (start === -1) {
    throw new Error(`Missing marker ${startMarker}`);
  }

  const contentStart = start + startMarker.length;
  const end = source.indexOf(endMarker, contentStart);
  if (end === -1) {
    throw new Error(`Missing marker ${endMarker}`);
  }

  return source.slice(contentStart, end);
}

export function extractRouteArray(source, name) {
  const block = extractBlock(source, `export const ${name} = [`, '] as const;');
  return extractQuotedValues(block);
}

export function extractObjectArray(source, objectName, propertyName) {
  const block = extractBlock(source, `const ${objectName} = {`, '} as const;');
  const match = new RegExp(`\\b${propertyName}:\\s*\\[([\\s\\S]*?)\\]`).exec(block);
  if (!match) {
    throw new Error(`Missing ${objectName}.${propertyName} array`);
  }

  return extractQuotedValues(match[1]);
}

export function extractQuotedValues(block) {
  return [...block.matchAll(/'([^']+)'/g)].map((match) => match[1]);
}

export function assertUnique(name, values) {
  const seen = new Set();
  const duplicates = [];

  for (const value of values) {
    if (seen.has(value)) {
      duplicates.push(value);
    }
    seen.add(value);
  }

  return duplicates.length === 0
    ? []
    : [`${name} contains duplicate labels: ${duplicates.join(', ')}`];
}

export function assertSameOrderedList(name, actual, expected) {
  const failures = [];
  const missing = expected.filter((value) => !actual.includes(value));
  const extra = actual.filter((value) => !expected.includes(value));

  if (missing.length > 0) {
    failures.push(`${name} is missing labels from the interaction modules: ${missing.join(', ')}`);
  }

  if (extra.length > 0) {
    failures.push(`${name} has labels not defined in the interaction modules: ${extra.join(', ')}`);
  }

  if (missing.length === 0 && extra.length === 0 && actual.join('\n') !== expected.join('\n')) {
    failures.push(
      `${name} order differs from the interaction modules.\n` +
        `Makefile: ${actual.join(' ')}\n` +
        `Modules:  ${expected.join(' ')}`,
    );
  }

  return failures;
}

export function assertSubset(name, actual, expected) {
  const expectedSet = new Set(expected);
  const missing = actual.filter((value) => !expectedSet.has(value));

  return missing.length === 0
    ? []
    : [`${name} has values outside the allowed set: ${missing.join(', ')}`];
}

export function assertInteractionTargetsExist(scenarioLabels, interactionTargets) {
  const scenarioLabelSet = new Set(scenarioLabels);
  const missingTargets = interactionTargets.filter((label) => !scenarioLabelSet.has(label));

  return missingTargets.length === 0
    ? []
    : [`Interactions reference missing parity scenarios: ${missingTargets.join(', ')}`];
}

export function assertSpecGroupCoverage(interactionList, groupNames) {
  const failures = [];
  const groupNameSet = new Set(groupNames);
  const usedGroups = new Set();
  const unmapped = [];

  for (const interaction of interactionList) {
    const group = groupOf(interaction);
    if (!group) {
      unmapped.push(interaction.label);
      continue;
    }
    if (!groupNameSet.has(group)) {
      failures.push(`Interaction ${interaction.label} maps to unknown spec group ${group}`);
      continue;
    }
    usedGroups.add(group);
  }

  if (unmapped.length > 0) {
    failures.push(`Interactions not assigned to any spec group: ${unmapped.join(', ')}`);
  }

  const emptyGroups = groupNames.filter((group) => !usedGroups.has(group));
  if (emptyGroups.length > 0) {
    failures.push(`Spec groups with no interactions: ${emptyGroups.join(', ')}`);
  }

  return failures;
}

export function assertRouteCoverage(name, routes, scenarios, behaviorCoveredLabels = new Set()) {
  const failures = [];
  const uncoveredRoutes = [];
  const retiredWithoutBehavior = [];

  for (const route of routes) {
    const matching = scenarios.filter((scenario) => routePatternMatches(route, scenario.route));

    if (matching.length === 0) {
      uncoveredRoutes.push(route);
      continue;
    }

    // A route still on the replica needs at least one active (non-retired) pixel
    // scenario. A redesigned route may retire all of its pixel scenarios, but only
    // if an interaction/behavior scenario keeps gating it — visual coverage may be
    // retired, behavior coverage may not be silently dropped.
    if (matching.some((scenario) => !scenario.visualRetired)) {
      continue;
    }

    if (!matching.some((scenario) => behaviorCoveredLabels.has(scenario.label))) {
      retiredWithoutBehavior.push({ route, labels: matching.map((scenario) => scenario.label) });
    }
  }

  const unknownScenarioRoutes = scenarios.filter(
    (scenario) => !routes.some((route) => routePatternMatches(route, scenario.route)),
  );

  if (uncoveredRoutes.length > 0) {
    failures.push(`${name} is missing parity scenarios for routes: ${uncoveredRoutes.join(', ')}`);
  }

  if (retiredWithoutBehavior.length > 0) {
    failures.push(
      `${name} retired pixel parity without interaction/behavior coverage for routes: ${retiredWithoutBehavior
        .map((entry) => `${entry.route} (${entry.labels.join(', ')})`)
        .join('; ')}`,
    );
  }

  if (unknownScenarioRoutes.length > 0) {
    failures.push(
      `${name} has parity scenarios for routes not declared by App.tsx: ${unknownScenarioRoutes
        .map((scenario) => `${scenario.label} -> ${scenario.route}`)
        .join(', ')}`,
    );
  }

  return failures;
}

export function normalizeScenarioRoute(path) {
  // Scenario paths are canonical route paths since W1 (docs/api-dialect.md
  // §13.4): the harness derives each world's entry URL (path-style source,
  // legacy /#/… oracle) from them, so a hash form here is a regression.
  if (path.includes('#')) {
    throw new Error(`Parity scenario path must be a canonical route path, not a hash URL: ${path}`);
  }

  const route = path.split('?', 1)[0] || '/';

  return route.startsWith('/') ? route : `/${route}`;
}

export function routePatternMatches(pattern, route) {
  const source = pattern
    .split('/')
    .map((segment) => {
      if (segment === '') return '';
      if (segment.startsWith(':')) return '[^/]+';
      return escapeRegExp(segment);
    })
    .join('/');

  return new RegExp(`^${source}$`).test(route);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function isMainModule() {
  const modulePath = getModulePath();

  return Boolean(modulePath && process.argv[1] && modulePath === resolve(process.argv[1]));
}

function getDefaultProjectRoot() {
  const modulePath = getModulePath();
  if (!modulePath) {
    throw new Error('Cannot infer project root from a non-file module URL');
  }

  return resolve(dirname(modulePath), '../..');
}

function getModulePath() {
  return import.meta.url.startsWith('file:') ? fileURLToPath(import.meta.url) : null;
}
