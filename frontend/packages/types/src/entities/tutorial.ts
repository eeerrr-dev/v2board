export interface TutorialSummary {
  id: number;
  title: string;
  category?: number | string;
  updated_at?: number;
  [key: string]: unknown;
}

export interface TutorialStep {
  title?: string;
  body?: string;
  [key: string]: unknown;
}

export interface Tutorial {
  id: number;
  title: string;
  steps?: string | TutorialStep[] | null;
  [key: string]: unknown;
}

export interface TutorialFetchResult {
  tutorials: TutorialSummary[] | Record<string, TutorialSummary[]>;
  safe_area_var?: Record<string, unknown>;
}
