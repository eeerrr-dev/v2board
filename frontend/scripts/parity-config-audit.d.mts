export interface VisualScenarioPath {
  label: string;
  route: string;
}

export interface ParityConfigAuditResult {
  adminRouteCount: number;
  browserScenarioCount: number;
  browserViewportCount: number;
  failures: string[];
  interactionScenarioCount: number;
  userRouteCount: number;
  visualScenarioCount: number;
}

export function auditParityConfig(projectRoot?: string): Promise<ParityConfigAuditResult>;
export function formatAuditSuccess(result: ParityConfigAuditResult): string;
export function readMakeList(source: string, name: string): string[];
export function resolveMakeListReferences(
  values: string[],
  references: Record<string, string[]>,
): string[];
export function extractBlock(source: string, startMarker: string, endMarker: string): string;
export function extractLabelsFromBlock(block: string): string[];
export function extractVisualScenarioPaths(block: string): VisualScenarioPath[];
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
  visualLabels: string[],
  interactionTargets: string[],
): string[];
export function assertRouteCoverage(
  name: string,
  routes: string[],
  scenarios: VisualScenarioPath[],
  behaviorCoveredLabels?: Set<string>,
): string[];
export function normalizeScenarioRoute(path: string): string;
export function routePatternMatches(pattern: string, route: string): boolean;
