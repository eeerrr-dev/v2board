#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

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
  const parityScriptPath = resolve(projectRoot, 'frontend/scripts/visual-parity.mjs');
  const userAppPath = resolve(projectRoot, 'frontend/apps/user/src/App.tsx');
  const adminAppPath = resolve(projectRoot, 'frontend/apps/admin/src/App.tsx');
  const userMainPath = resolve(projectRoot, 'frontend/apps/user/src/main.tsx');
  const adminMainPath = resolve(projectRoot, 'frontend/apps/admin/src/main.tsx');
  const userDevEntryPath = resolve(projectRoot, 'frontend/apps/user/index.html');
  const adminDevEntryPath = resolve(projectRoot, 'frontend/apps/admin/index.html');

  const [
    makefile,
    parityScript,
    userApp,
    adminApp,
    userMain,
    adminMain,
    userDevEntry,
    adminDevEntry,
  ] = await Promise.all([
    readFile(makefilePath, 'utf8'),
    readFile(parityScriptPath, 'utf8'),
    readFile(userAppPath, 'utf8'),
    readFile(adminAppPath, 'utf8'),
    readFile(userMainPath, 'utf8'),
    readFile(adminMainPath, 'utf8'),
    readFile(userDevEntryPath, 'utf8'),
    readFile(adminDevEntryPath, 'utf8'),
  ]);

  const makeVisualScenarios = readMakeList(makefile, 'VISUAL_PARITY_SCENARIOS');
  const makeInteractionScenarios = readMakeList(makefile, 'INTERACTION_PARITY_SCENARIOS');
  const makeBrowserScenarios = resolveMakeListReferences(
    readMakeList(makefile, 'BROWSER_PARITY_SCENARIOS'),
    { VISUAL_PARITY_SCENARIOS: makeVisualScenarios },
  );
  const makeBrowserViewports = readMakeList(makefile, 'BROWSER_PARITY_VIEWPORTS');
  const visualScenarioBlock = extractBlock(
    parityScript,
    'const scenarios = [',
    '];\nconst interactionScenarios = [',
  );
  const visualScenarios = extractLabelsFromBlock(visualScenarioBlock);
  const visualScenarioPaths = extractVisualScenarioPaths(visualScenarioBlock);
  const viewportBlock = extractBlock(
    parityScript,
    'const viewports = [',
    '];\nconst darkModeStyleTargets =',
  );
  const visualViewports = extractLabelsFromBlock(viewportBlock);
  const interactionBlock = extractBlock(
    parityScript,
    'const interactionScenarios = [',
    '];\nconst guestConfigFixture =',
  );
  const interactionScenarios = extractLabelsFromBlock(interactionBlock);
  const interactionTargets = [...interactionBlock.matchAll(/\bscenarioLabel:\s*'([^']+)'/g)].map(
    (match) => match[1],
  );
  const userRoutes = extractRouteArray(userApp, 'USER_LEGACY_ROUTE_PATHS');
  const adminRoutes = extractRouteArray(adminApp, 'ADMIN_LEGACY_ROUTE_PATHS');
  const userAppPublicRoutes = extractObjectArray(
    userApp,
    'USER_LEGACY_ROUTE_OPTIONS',
    'publicRoutes',
  );
  const adminAppPublicRoutes = extractObjectArray(
    adminApp,
    'ADMIN_LEGACY_ROUTE_OPTIONS',
    'publicRoutes',
  );
  const userMainPublicRoutes = extractObjectArray(
    userMain,
    'legacyHashRouteOptions',
    'publicRoutes',
  );
  const adminMainPublicRoutes = extractObjectArray(
    adminMain,
    'legacyHashRouteOptions',
    'publicRoutes',
  );
  const userDevRoutes = extractAssignedRouteArray(userDevEntry, 'var legacyRoutes = [');
  const adminDevRoutes = extractAssignedRouteArray(adminDevEntry, 'var legacyRoutes = [');
  const userDevPublicRoutes = extractAssignedRouteArray(
    userDevEntry,
    'var legacyPublicRoutes = [',
  );
  const adminDevPublicRoutes = extractAssignedRouteArray(
    adminDevEntry,
    'var legacyPublicRoutes = [',
  );

  const failures = [
    ...assertUnique('visual parity script scenarios', visualScenarios),
    ...assertUnique('interaction parity script scenarios', interactionScenarios),
    ...assertUnique('Makefile VISUAL_PARITY_SCENARIOS', makeVisualScenarios),
    ...assertUnique('Makefile INTERACTION_PARITY_SCENARIOS', makeInteractionScenarios),
    ...assertUnique('Makefile BROWSER_PARITY_SCENARIOS', makeBrowserScenarios),
    ...assertUnique('Makefile BROWSER_PARITY_VIEWPORTS', makeBrowserViewports),
    ...assertSameOrderedList('VISUAL_PARITY_SCENARIOS', makeVisualScenarios, visualScenarios),
    ...assertSameOrderedList(
      'INTERACTION_PARITY_SCENARIOS',
      makeInteractionScenarios,
      interactionScenarios,
    ),
    ...assertSameOrderedList('BROWSER_PARITY_SCENARIOS', makeBrowserScenarios, visualScenarios),
    ...assertSubset('BROWSER_PARITY_VIEWPORTS', makeBrowserViewports, visualViewports),
    ...assertInteractionTargetsExist(visualScenarios, interactionTargets),
    ...assertRouteCoverage(
      'user visual parity route coverage',
      userRoutes,
      visualScenarioPaths.filter((scenario) => scenario.label.startsWith('user-')),
    ),
    ...assertRouteCoverage(
      'admin visual parity route coverage',
      adminRoutes,
      visualScenarioPaths.filter((scenario) => scenario.label.startsWith('admin-')),
    ),
    ...assertSameOrderedValues('user dev entry legacyRoutes', userDevRoutes, userRoutes),
    ...assertSameOrderedValues('admin dev entry legacyRoutes', adminDevRoutes, adminRoutes),
    ...assertSameOrderedValues(
      'user main legacy publicRoutes',
      userMainPublicRoutes,
      userAppPublicRoutes,
    ),
    ...assertSameOrderedValues(
      'admin main legacy publicRoutes',
      adminMainPublicRoutes,
      adminAppPublicRoutes,
    ),
    ...assertSameOrderedValues(
      'user dev entry legacyPublicRoutes',
      userDevPublicRoutes,
      userAppPublicRoutes,
    ),
    ...assertSameOrderedValues(
      'admin dev entry legacyPublicRoutes',
      adminDevPublicRoutes,
      adminAppPublicRoutes,
    ),
  ];

  return {
    adminRouteCount: adminRoutes.length,
    browserScenarioCount: makeBrowserScenarios.length,
    browserViewportCount: makeBrowserViewports.length,
    failures,
    interactionScenarioCount: interactionScenarios.length,
    userRouteCount: userRoutes.length,
    visualScenarioCount: visualScenarios.length,
  };
}

export function formatAuditSuccess(result) {
  return `Parity config audit OK: Makefile tracks ${result.visualScenarioCount} visual scenarios, ${result.interactionScenarioCount} interaction scenarios, ${result.browserScenarioCount} browser scenarios across ${result.browserViewportCount} viewports, parity covers ${result.userRouteCount} user routes plus ${result.adminRouteCount} admin routes, and dev entry route mirrors are aligned.`;
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

export function resolveMakeListReferences(values, references) {
  return values.flatMap((value) => {
    const match = /^\$\(([^)]+)\)$/.exec(value);
    if (!match) return [value];
    const resolved = references[match[1]];
    if (!resolved) {
      throw new Error(`Unsupported Makefile list reference ${value}`);
    }
    return resolved;
  });
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

export function extractLabelsFromBlock(block) {
  return [...block.matchAll(/\blabel:\s*'([^']+)'/g)].map((match) => match[1]);
}

export function extractVisualScenarioPaths(block) {
  return [
    ...block.matchAll(
      /\blabel:\s*'([^']+)'[\s\S]*?\bpath:\s*(?:'([^']+)'|`([^`]+)`)/g,
    ),
  ].map((match) => ({
    label: match[1],
    route: normalizeScenarioRoute(match[2] ?? match[3]),
  }));
}

export function extractRouteArray(source, name) {
  const block = extractBlock(source, `export const ${name} = [`, '] as const;');
  return extractQuotedValues(block);
}

export function extractAssignedRouteArray(source, startMarker) {
  const block = extractBlock(source, startMarker, '];');
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
    failures.push(`${name} is missing labels from visual-parity.mjs: ${missing.join(', ')}`);
  }

  if (extra.length > 0) {
    failures.push(`${name} has labels not defined in visual-parity.mjs: ${extra.join(', ')}`);
  }

  if (missing.length === 0 && extra.length === 0 && actual.join('\n') !== expected.join('\n')) {
    failures.push(
      `${name} order differs from visual-parity.mjs.\n` +
        `Makefile: ${actual.join(' ')}\n` +
        `Script:   ${expected.join(' ')}`,
    );
  }

  return failures;
}

export function assertSameOrderedValues(name, actual, expected) {
  const failures = [];
  const missing = expected.filter((value) => !actual.includes(value));
  const extra = actual.filter((value) => !expected.includes(value));

  if (missing.length > 0) {
    failures.push(`${name} is missing values: ${missing.join(', ')}`);
  }

  if (extra.length > 0) {
    failures.push(`${name} has unexpected values: ${extra.join(', ')}`);
  }

  if (missing.length === 0 && extra.length === 0 && actual.join('\n') !== expected.join('\n')) {
    failures.push(
      `${name} order differs.\nActual:   ${actual.join(' ')}\nExpected: ${expected.join(' ')}`,
    );
  }

  return failures;
}

export function assertSubset(name, actual, expected) {
  const expectedSet = new Set(expected);
  const missing = actual.filter((value) => !expectedSet.has(value));

  return missing.length === 0
    ? []
    : [`${name} has values not defined by visual-parity.mjs: ${missing.join(', ')}`];
}

export function assertInteractionTargetsExist(visualLabels, interactionTargets) {
  const visualLabelSet = new Set(visualLabels);
  const missingTargets = interactionTargets.filter((label) => !visualLabelSet.has(label));

  return missingTargets.length === 0
    ? []
    : [`Interaction scenarios reference missing visual scenarios: ${missingTargets.join(', ')}`];
}

export function assertRouteCoverage(name, routes, scenarios) {
  const failures = [];
  const uncoveredRoutes = routes.filter(
    (route) => !scenarios.some((scenario) => routePatternMatches(route, scenario.route)),
  );
  const unknownScenarioRoutes = scenarios.filter(
    (scenario) => !routes.some((route) => routePatternMatches(route, scenario.route)),
  );

  if (uncoveredRoutes.length > 0) {
    failures.push(`${name} is missing screenshot scenarios for routes: ${uncoveredRoutes.join(', ')}`);
  }

  if (unknownScenarioRoutes.length > 0) {
    failures.push(
      `${name} has screenshot scenarios for routes not declared by App.tsx: ${unknownScenarioRoutes
        .map((scenario) => `${scenario.label} -> ${scenario.route}`)
        .join(', ')}`,
    );
  }

  return failures;
}

export function normalizeScenarioRoute(path) {
  const hashIndex = path.indexOf('#');
  if (hashIndex === -1) {
    throw new Error(`Visual parity scenario path does not include a hash route: ${path}`);
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
