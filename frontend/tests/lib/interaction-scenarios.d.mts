export interface InteractionScenario {
  label: string;
  scenarioLabel: string;
  sourceOnly?: boolean;
  run: (...args: unknown[]) => unknown;
}

export const interactions: InteractionScenario[];
