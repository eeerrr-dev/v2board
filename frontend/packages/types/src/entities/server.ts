export type ServerType =
  'shadowsocks' | 'vmess' | 'trojan' | 'tuic' | 'vless' | 'hysteria' | 'anytls' | 'v2node';

/**
 * GET /user/servers row (docs/api-dialect.md §5.4, W6): boolean `is_online`,
 * numeric `rate`/`port`, RFC 3339 `last_check_at`.
 */
export interface AvailableServer {
  id: number;
  parent_id: number | null;
  group_id: number[];
  route_id: number[] | null;
  name: string;
  rate: number;
  type: ServerType;
  host: string;
  port: number;
  cache_key: string;
  last_check_at: string | null;
  is_online: boolean;
  tags?: string[] | null;
}
