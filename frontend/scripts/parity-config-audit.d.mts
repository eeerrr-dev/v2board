export interface ScenarioRoute {
  label: string;
  route: string;
  visualRetired?: boolean;
}

export interface ParityInteraction {
  label: string;
}

export interface ParityConfigAuditResult {
  adminRouteCount: number;
  failures: string[];
  interactionScenarioCount: number;
  scenarioCount: number;
  specGroupCount: number;
  userRouteCount: number;
  viewportCount: number;
}

export function auditParityConfig(projectRoot?: string): Promise<ParityConfigAuditResult>;
export function formatAuditSuccess(result: ParityConfigAuditResult): string;
export function readMakeList(source: string, name: string): string[];
export function extractBlock(source: string, startMarker: string, endMarker: string): string;
export function extractRouteArray(source: string, name: string): string[];
export function extractAssignedRouteArray(source: string, startMarker: string): string[];
export function extractObjectArray(
  source: string,
  objectName: string,
  propertyName: string,
): string[];
export function extractQuotedValues(block: string): string[];
export function assertUnique(name: string, values: string[]): string[];
export function assertSameOrderedList(name: string, actual: string[], expected: string[]): string[];
export function assertSameOrderedValues(name: string, actual: string[], expected: string[]): string[];
export function assertSubset(name: string, actual: string[], expected: string[]): string[];
export function assertInteractionTargetsExist(
  scenarioLabels: string[],
  interactionTargets: string[],
): string[];
export function assertSpecGroupCoverage(
  interactionList: readonly ParityInteraction[],
  groupNames: string[],
): string[];
export function assertRouteCoverage(
  name: string,
  routes: string[],
  scenarios: ScenarioRoute[],
  behaviorCoveredLabels?: Set<string>,
): string[];
export function normalizeScenarioRoute(path: string): string;
export function routePatternMatches(pattern: string, route: string): boolean;
