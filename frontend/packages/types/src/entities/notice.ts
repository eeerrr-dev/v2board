/**
 * GET /user/notices item (docs/api-dialect.md §5.8, W3) and, since W10, the
 * admin `GET /{secure_path}/notices` row (§6.3 — same field set, same modern
 * value types): boolean `show`, RFC 3339 timestamps. `tags` keeps carrying
 * the backend `弹窗` auto-popup marker (Tier-1).
 */
export interface Notice {
  id: number;
  title: string;
  content: string;
  show: boolean;
  img_url: string | null;
  tags: string[] | null;
  created_at: string;
  updated_at: string;
}
