#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { viewports } from '../tests/lib/env.mjs';
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

  const [makefile, userApp, adminApp] = await Promise.all([
    readFile(makefilePath, 'utf8'),
    readFile(userAppPath, 'utf8'),
    readFile(adminAppPath, 'utf8'),
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
  const userAppPublicRoutes = extractObjectArray(userApp, 'USER_HASH_ROUTE_OPTIONS', 'publicRoutes');
  const adminAppPublicRoutes = extractObjectArray(
    adminApp,
    'ADMIN_HASH_ROUTE_OPTIONS',
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
  ];

  return {
    adminRouteCount: adminRoutes.length,
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
    `user routes plus ${result.adminRouteCount} admin routes. UI sync covers ` +
    `${result.uiSharedPrimitiveCount} shared primitives, ${result.uiSharedStylesheetCount} ` +
    `shared stylesheets, and ${result.uiAppSpecificCount} explicit app-only ${appOnlyNoun}.`
  );
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
  const hashIndex = path.indexOf('#');
  if (hashIndex === -1) {
    throw new Error(`Parity scenario path does not include a hash route: ${path}`);
  }

  const routeWithQuery = path.slice(hashIndex + 1) || '/';
  const route = routeWithQuery.split(/[?#]/, 1)[0] || '/';

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
