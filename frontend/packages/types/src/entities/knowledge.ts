/**
 * User knowledge rows (docs/api-dialect.md §5.8, W3): boolean `show` and RFC
 * 3339 timestamps. The detail `body` stays non-idempotent — re-substituted
 * per request (Tier-1 refetch behavior).
 */
export interface KnowledgeSummary {
  id: number;
  category: string;
  title: string;
  sort: number | null;
  show: boolean;
  updated_at: string;
}

export interface Knowledge extends KnowledgeSummary {
  body: string;
  language: string;
  created_at: string;
}

export type KnowledgeCategory = Record<string, KnowledgeSummary[]>;

/** Legacy admin knowledge rows (`/admin/knowledge/*`, W10 still legacy). */
export interface AdminKnowledgeSummary {
  id: number;
  category: string;
  title: string;
  show: 0 | 1;
  updated_at: number;
}

export interface AdminKnowledge extends AdminKnowledgeSummary {
  sort: number | null;
  body: string;
  language: string;
  created_at: number;
}
