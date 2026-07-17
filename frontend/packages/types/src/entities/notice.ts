/**
 * GET /user/notices item (docs/api-dialect.md §5.8, W3): boolean `show`,
 * RFC 3339 timestamps. `tags` keeps carrying the backend `弹窗` auto-popup
 * marker (Tier-1).
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

/** Legacy admin notice row (`/admin/notice/fetch`, W10 still legacy). */
export interface AdminNotice {
  id: number;
  title: string;
  content: string;
  img_url: string | null;
  tags: string[] | null;
  show: 0 | 1;
  created_at: number;
  updated_at: number;
}
