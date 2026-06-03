export type ServerType =
  | 'shadowsocks'
  | 'vmess'
  | 'trojan'
  | 'tuic'
  | 'vless'
  | 'hysteria'
  | 'anytls'
  | 'v2node';

export interface AvailableServer {
  id: number;
  parent_id: number | null;
  group_id: number[];
  route_id: number[] | null;
  name: string;
  rate: string;
  type: ServerType;
  host: string;
  port: string | number;
  cache_key: string;
  last_check_at: number | null;
  is_online: 0 | 1;
  tags?: string[] | null;
}
