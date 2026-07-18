/**
 * User knowledge rows (docs/api-dialect.md §5.8, W3): boolean `show` and RFC
 * 3339 timestamps. The detail `body` stays non-idempotent — re-substituted
 * per request (Tier-1 refetch behavior). Since W10 the admin
 * `GET /{secure_path}/knowledge` rows (§6.3) reuse the same shapes — the
 * admin detail differs only in serving the raw stored body.
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
