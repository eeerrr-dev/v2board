export interface KnowledgeSummary {
  id: number;
  category: string;
  title: string;
  /** Detail responses include sort; list projections intentionally omit it. */
  sort?: number | null;
  /** Admin lists/details include visibility; user list projections omit it. */
  show?: 0 | 1;
  updated_at: number;
}

export interface Knowledge extends KnowledgeSummary {
  body: string;
  language: string;
  sort: number | null;
  show: 0 | 1;
  created_at: number;
}

export type KnowledgeCategory = Record<string, KnowledgeSummary[]>;
