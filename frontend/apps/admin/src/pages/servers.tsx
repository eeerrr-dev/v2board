import { cloneElement, useEffect, useMemo, useRef, useState } from 'react';
import type {
  CSSProperties,
  HTMLAttributes,
  MouseEvent as ReactMouseEvent,
  ReactElement,
  ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { App, Form, Input } from 'antd';
import type { FormInstance } from 'antd';
import { useLocation } from 'react-router-dom';
import {
  useCopyServerMutation,
  useDropServerGroupMutation,
  useDropServerMutation,
  useDropServerRouteMutation,
  useSaveServerGroupMutation,
  useSaveServerRouteMutation,
  useServerGroups,
  useServerNodes,
  useServerRoutes,
  useSortServerNodesMutation,
  useUpdateServerMutation,
} from '@/lib/queries';
import { admin } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { i18nGet } from '@/lib/errors';
import { legacyCopyText } from '@/lib/legacy-copy';
import { formatDateTime } from '@v2board/config/format';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';
import { LegacyDragSort, LegacyMenuIcon } from '@/components/legacy-drag-sort';
import { LegacyButton } from '@/components/legacy-button';
import { LegacyDrawer } from '@/components/legacy-drawer';
import {
  LegacyCaretDownIcon,
  LegacyCaretUpIcon,
  LegacyCopyIcon,
  LegacyDatabaseIcon,
  LegacyDeleteIcon,
  LegacyEditIcon,
  LegacyFilterIcon,
  LegacyFormIcon,
  LegacyLinkIcon,
  LegacyLoadingIcon,
  LegacyPlusIcon,
  LegacyQuestionCircleIcon,
  LegacyReadIcon,
  LegacyUserIcon,
} from '@/components/legacy-ant-icon';
import { LegacyCheckboxInput, LegacyInput, LegacyTextArea } from '@/components/legacy-input';
import { LegacyEmpty } from '@/components/legacy-empty';
import { LegacySwitch } from '@/components/legacy-switch';
import { LegacyModal } from '@/components/legacy-modal';
import { LegacyTooltip } from '@/components/legacy-tooltip';
import {
  LegacySelect,
  type LegacySelectOption,
  type LegacySelectValue,
} from '@/components/legacy-select';
import {
  LegacyStandaloneTable,
  legacyTableRowKey as legacyRowKey,
  type LegacyStandaloneTableHeader,
} from '@/components/legacy-standalone-table';

const SERVER_TYPES: admin.ServerTypeName[] = [
  'v2node',
  'shadowsocks',
  'vmess',
  'trojan',
  'hysteria',
  'tuic',
  'vless',
  'anytls',
];

const SERVER_TYPE_LABELS: Record<string, string> = {
  v2node: 'V2node',
  shadowsocks: 'Shadowsocks',
  vmess: 'VMess',
  trojan: 'Trojan',
  hysteria: 'Hysteria',
  tuic: 'Tuic',
  vless: 'VLess',
  anytls: 'AnyTLS',
};

const SERVER_TYPE_COLORS: Record<string, string> = {
  shadowsocks: '#489851',
  vmess: '#CB3180',
  trojan: '#EAB854',
  hysteria: '#1A1A1A',
  tuic: '#9400D3',
  vless: '#4080FF',
  anytls: '#FF8C00',
  v2node: '#FF0000',
};

const AVAILABLE_STATUS: Record<number, 'error' | 'warning' | 'processing'> = {
  0: 'error',
  1: 'warning',
  2: 'processing',
};

type RouteActionTextMap = Record<string, string> &
  Record<
    'block' | 'block_ip' | 'block_port' | 'protocol' | 'dns' | 'route' | 'route_ip' | 'default_out',
    string
  >;

const ROUTE_ACTION_TEXT: RouteActionTextMap = {
  block: '禁止访问(域名目标)',
  block_ip: '禁止访问(IP目标)',
  block_port: '禁止访问(端口目标)',
  protocol: '禁止访问(协议)',
  dns: '指定DNS服务器进行解析',
  route: '指定出站服务器(域名目标)',
  route_ip: '指定出站服务器(IP目标)',
  default_out: '自定义默认出站',
};

const LEGACY_BINARY_SELECT_OPTIONS: LegacySelectOption[] = [
  { value: 0, label: '否' },
  { value: 1, label: '是' },
];

const LEGACY_TLS_SUPPORT_OPTIONS: LegacySelectOption[] = [
  { value: 0, label: '不支持' },
  { value: 1, label: '支持' },
];

const LEGACY_SECURITY_NONE_OPTION: LegacySelectOption = { value: 0, label: '无' };
const LEGACY_SECURITY_TLS_OPTION: LegacySelectOption = { value: 1, label: 'TLS' };
const LEGACY_SECURITY_REALITY_OPTION: LegacySelectOption = { value: 2, label: 'Reality' };
const LEGACY_STREAM_NETWORK_OPTIONS: LegacySelectOption[] = [
  { value: 'tcp', label: 'TCP' },
  { value: 'ws', label: 'WebSocket' },
  { value: 'grpc', label: 'gRPC' },
  { value: 'kcp', label: 'mKCP' },
  { value: 'httpupgrade', label: 'HTTPUpgrade' },
  { value: 'xhttp', label: 'XHTTP' },
];
const LEGACY_TROJAN_NETWORK_OPTIONS: LegacySelectOption[] = [
  { value: 'tcp', label: 'TCP' },
  { value: 'ws', label: 'WebSocket' },
  { value: 'grpc', label: 'gRPC' },
];
const LEGACY_V2NODE_PROTOCOL_OPTIONS: LegacySelectOption[] = [
  { value: 'anytls', label: 'AnyTLS' },
  { value: 'hysteria2', label: 'Hysteria2' },
  { value: 'shadowsocks', label: 'Shadowsocks' },
  { value: 'trojan', label: 'Trojan' },
  { value: 'tuic', label: 'Tuic' },
  { value: 'vless', label: 'VLess' },
  { value: 'vmess', label: 'VMess' },
];
const LEGACY_V2NODE_SHADOWSOCKS_NETWORK_OPTIONS: LegacySelectOption[] = [
  { value: 'tcp', label: 'TCP' },
  { value: 'http', label: 'HTTP伪装' },
];
const LEGACY_V2NODE_TRANSPORT_OPTIONS: LegacySelectOption[] = [
  { value: 'tcp', label: 'TCP' },
  { value: 'ws', label: 'WebSocket' },
  { value: 'grpc', label: 'gRPC' },
  { value: 'httpupgrade', label: 'HTTPUpgrade' },
  { value: 'xhttp', label: 'XHTTP' },
];
const LEGACY_HYSTERIA2_OBFS_OPTIONS: LegacySelectOption[] = [
  { value: null, label: '无' },
  { value: 'salamander', label: 'salamander' },
];
const LEGACY_TUIC_RELAY_MODE_OPTIONS: LegacySelectOption[] = [
  { value: 'native', label: 'native' },
  { value: 'quic', label: 'quic' },
];
const LEGACY_TUIC_CONGESTION_CONTROL_OPTIONS: LegacySelectOption[] = [
  { value: 'cubic', label: 'cubic' },
  { value: 'new_reno', label: 'new_reno' },
  { value: 'bbr', label: 'bbr' },
];
const LEGACY_SHADOWSOCKS_CIPHER_OPTIONS: LegacySelectOption[] = [
  { value: 'aes-128-gcm', label: 'aes-128-gcm' },
  { value: 'aes-192-gcm', label: 'aes-192-gcm' },
  { value: 'aes-256-gcm', label: 'aes-256-gcm' },
  { value: 'chacha20-ietf-poly1305', label: 'chacha20-ietf-poly1305' },
  { value: '2022-blake3-aes-128-gcm', label: '2022-blake3-aes-128-gcm' },
  { value: '2022-blake3-aes-256-gcm', label: '2022-blake3-aes-256-gcm' },
];
const LEGACY_SHADOWSOCKS_OBFS_OPTIONS: LegacySelectOption[] = [
  { value: '', label: '无' },
  { value: 'http', label: 'HTTP' },
];
const LEGACY_VLESS_ENCRYPTION_OPTIONS: LegacySelectOption[] = [
  { value: null, label: '无' },
  { value: 'mlkem768x25519plus', label: 'MLKEM768X25519PLUS' },
];
const LEGACY_VLESS_FLOW_NONE_OPTIONS: LegacySelectOption[] = [{ value: null, label: '无' }];
const LEGACY_VLESS_FLOW_OPTIONS: LegacySelectOption[] = [
  ...LEGACY_VLESS_FLOW_NONE_OPTIONS,
  { value: 'xtls-rprx-vision', label: 'xtls-rprx-vision' },
];
const LEGACY_HYSTERIA_VERSION_OPTIONS: LegacySelectOption[] = [
  { value: 1, label: 'v1' },
  { value: 2, label: 'v2' },
];
const LEGACY_HYSTERIA_V1_OBFS_OPTIONS: LegacySelectOption[] = [
  { value: null, label: '无' },
  { value: 'xplus', label: 'xplus' },
];
const LEGACY_TLS_CERT_MODE_OPTIONS: LegacySelectOption[] = [
  { value: 'self', label: '自签名' },
  { value: 'http', label: 'HTTP申请' },
  { value: 'dns', label: 'DNS申请' },
  { value: 'none', label: '无证书(关闭TLS)' },
];
const LEGACY_PROXY_PROTOCOL_OPTIONS: LegacySelectOption[] = [
  { value: 0, label: '0' },
  { value: 1, label: '1' },
  { value: 2, label: '2' },
];
const LEGACY_TLS_FINGERPRINT_OPTIONS: LegacySelectOption[] = [
  { value: 'chrome', label: 'Chrome' },
  { value: 'firefox', label: 'Firefox' },
  { value: 'safari', label: 'Safari' },
  { value: 'ios', label: 'IOS' },
  { value: 'android', label: 'Android' },
  { value: 'edge', label: 'Edge' },
  { value: '360', label: '360' },
  { value: 'qq', label: 'QQ' },
];
const LEGACY_ECH_MODE_OPTIONS: LegacySelectOption[] = [
  { value: '', label: '无' },
  { value: 'cloudflare', label: 'Cloudflare' },
  { value: 'custom', label: '自定义 SNI' },
];
const LEGACY_ENCRYPTION_MODE_OPTIONS: LegacySelectOption[] = [
  { value: 'native', label: 'native' },
  { value: 'xorpub', label: 'xorpub' },
  { value: 'random', label: 'random' },
];
const LEGACY_ENCRYPTION_RTT_OPTIONS: LegacySelectOption[] = [
  { value: '0rtt', label: '0rtt' },
  { value: '1rtt', label: '1rtt' },
];

const LEGACY_HABIT_KEY = 'habit';
const LEGACY_SERVER_PAGE_SIZE_KEY = 'server_manage_page_size';
const LEGACY_SERVER_SORT_PROMPT = '节点排序还没有保存，是否离开';
const ANYTLS_PADDING_SCHEME_PLACEHOLDER = JSON.stringify(
  [
    'stop=8',
    '0=30-30',
    '1=100-400',
    '2=400-500,c,500-1000,c,500-1000,c,500-1000,c,500-1000',
    '3=9-9,500-1000',
    '4=500-1000',
    '5=500-1000',
    '6=500-1000',
    '7=500-1000',
  ],
  null,
  4,
);
const LEGACY_TLS_FORCED_PROTOCOLS = ['anytls', 'hysteria2', 'trojan', 'tuic'];
const LEGACY_V2NODE_SECURITY_FALLBACK_PROTOCOLS = ['hysteria2', 'trojan', 'tuic'];
const LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: JSON.stringify(
    {
      header: {
        type: 'http',
        request: {
          path: ['/'],
          headers: {
            Host: ['www.baidu.com', 'www.bing.com'],
          },
        },
        response: {},
      },
    },
    null,
    4,
  ),
  ws: JSON.stringify(
    {
      path: '/',
      headers: {
        Host: 'v2ray.com',
      },
    },
    null,
    4,
  ),
  grpc: JSON.stringify(
    {
      serviceName: 'GunService',
    },
    null,
    4,
  ),
  kcp: JSON.stringify(
    {
      header: {
        type: 'none',
      },
      seed: '',
    },
    null,
    4,
  ),
  httpupgrade: JSON.stringify(
    {
      path: '/',
      host: 'xtls.github.io',
    },
    null,
    4,
  ),
  xhttp: JSON.stringify(
    {
      path: '/',
      host: 'xtls.github.io',
    },
    null,
    4,
  ),
};
const LEGACY_VLESS_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.tcp!,
  ws: JSON.stringify(
    {
      security: 'auto',
      path: '/',
      headers: {
        Host: 'xtls.github.io',
      },
    },
    null,
    4,
  ),
  grpc: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.grpc!,
  kcp: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.kcp!,
  httpupgrade: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.httpupgrade!,
  xhttp: JSON.stringify(
    {
      path: '/',
      host: 'xtls.github.io',
      mode: 'auto',
      extra: {},
    },
    null,
    4,
  ),
};
const LEGACY_TROJAN_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: '',
  ws: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.ws!,
  grpc: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.grpc!,
};
const LEGACY_V2NODE_NETWORK_SETTINGS_PLACEHOLDERS: Record<string, string> = {
  tcp: JSON.stringify(
    {
      acceptProxyProtocol: false,
      header: {
        type: 'http',
        request: {
          path: ['/'],
          headers: {
            Host: ['www.baidu.com', 'www.bing.com'],
          },
        },
        response: {},
      },
    },
    null,
    4,
  ),
  http: JSON.stringify(
    {
      acceptProxyProtocol: false,
      path: '/',
      Host: 'xtls.github.io',
    },
    null,
    4,
  ),
  ws: JSON.stringify(
    {
      acceptProxyProtocol: false,
      path: '/',
      headers: {
        Host: 'xtls.github.io',
      },
    },
    null,
    4,
  ),
  grpc: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.grpc!,
  httpupgrade: JSON.stringify(
    {
      acceptProxyProtocol: false,
      path: '/',
      host: 'xtls.github.io',
    },
    null,
    4,
  ),
  xhttp: LEGACY_VLESS_NETWORK_SETTINGS_PLACEHOLDERS.xhttp!,
};
const LEGACY_NETWORK_SETTINGS_PLACEHOLDERS: Partial<
  Record<admin.ServerTypeName, Record<string, string>>
> = {
  vmess: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS,
  vless: LEGACY_VLESS_NETWORK_SETTINGS_PLACEHOLDERS,
  trojan: LEGACY_TROJAN_NETWORK_SETTINGS_PLACEHOLDERS,
  v2node: LEGACY_V2NODE_NETWORK_SETTINGS_PLACEHOLDERS,
};
const LEGACY_TLS_SETTINGS_DEFAULTS: Record<string, unknown> = {
  server_name: '',
  cert_mode: 'self',
  provider: '',
  dns_env: '',
  reject_unknown_sni: '0',
  allow_insecure: '0',
};
const LEGACY_ENCRYPTION_SETTINGS_DEFAULTS: Record<string, unknown> = {
  mode: 'native',
  rtt: '0rtt',
  ticket: '600s',
  server_padding: null,
  client_padding: null,
  private_key: null,
  password: null,
};

function readLegacyHabit(key: string): unknown {
  if (typeof window === 'undefined') return undefined;
  try {
    const stored = window.localStorage.getItem(LEGACY_HABIT_KEY);
    if (!stored) return undefined;
    const parsed = JSON.parse(stored) as Record<string, unknown>;
    return parsed?.[key];
  } catch {
    return undefined;
  }
}

function writeLegacyHabit(key: string, value: unknown) {
  if (typeof window === 'undefined') return;
  try {
    const stored = window.localStorage.getItem(LEGACY_HABIT_KEY);
    if (stored) {
      const legacyHabit = stored as unknown as Record<string, unknown>;
      legacyHabit[key] = value;
      window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify(legacyHabit));
    } else {
      window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify({ [key]: value }));
    }
  } catch {
    window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify({ [key]: value }));
  }
}

interface LegacyDropdownCoords {
  left: number;
  top: number;
  minWidth: number;
}

type LegacyDropdownTrigger = 'click' | 'hover';

type LegacyDropdownChildProps = {
  className?: string;
  onClick?: (event: ReactMouseEvent<HTMLElement>) => void;
  onMouseEnter?: (event: ReactMouseEvent<HTMLElement>) => void;
  onMouseLeave?: (event: ReactMouseEvent<HTMLElement>) => void;
};

interface LegacyDropdownProps {
  children: ReactElement<LegacyDropdownChildProps>;
  overlay: ReactNode;
  trigger?: LegacyDropdownTrigger | LegacyDropdownTrigger[];
}

interface LegacyDropdownMenuItemProps {
  children?: ReactNode;
  onClick?: (event: ReactMouseEvent<HTMLLIElement>) => void;
  onContextMenu?: (event: ReactMouseEvent<HTMLLIElement>) => void;
  style?: CSSProperties;
}

const LEGACY_DROPDOWN_CLICK_TRIGGER = 'click' satisfies LegacyDropdownTrigger;
const LEGACY_DROPDOWN_HOVER_CLOSE_DELAY = 120;
const LEGACY_DROPDOWN_OFFSET = 4;

function dropdownTriggerModes(trigger: LegacyDropdownProps['trigger']) {
  return Array.isArray(trigger) ? trigger : trigger ? [trigger] : ['hover'];
}

function legacyDropdownClassName(open: boolean) {
  return [
    'ant-dropdown',
    'ant-dropdown-placement-bottomLeft',
    open ? undefined : 'ant-dropdown-hidden',
  ]
    .filter(Boolean)
    .join(' ');
}

function mergeClassName(...values: Array<string | undefined | false>) {
  return values.filter(Boolean).join(' ');
}

function LegacyDropdown({ children, overlay, trigger }: LegacyDropdownProps) {
  const [open, setOpen] = useState(false);
  const [hasOpened, setHasOpened] = useState(false);
  const [coords, setCoords] = useState<LegacyDropdownCoords>();
  const popupRef = useRef<HTMLDivElement | null>(null);
  const closeTimer = useRef<number | undefined>(undefined);
  const triggerModes = dropdownTriggerModes(trigger);
  const opensOnClick = triggerModes.includes('click');
  const opensOnHover = triggerModes.includes('hover');

  const clearCloseTimer = () => {
    if (closeTimer.current !== undefined) {
      window.clearTimeout(closeTimer.current);
      closeTimer.current = undefined;
    }
  };

  const openFromElement = (element: HTMLElement) => {
    const rect = element.getBoundingClientRect();
    clearCloseTimer();
    setCoords({
      left: rect.left,
      top: rect.bottom + LEGACY_DROPDOWN_OFFSET,
      minWidth: rect.width,
    });
    setHasOpened(true);
    setOpen(true);
  };

  const scheduleHoverClose = () => {
    if (!opensOnHover) return;
    clearCloseTimer();
    closeTimer.current = window.setTimeout(() => setOpen(false), LEGACY_DROPDOWN_HOVER_CLOSE_DELAY);
  };

  useEffect(() => {
    if (!open || !opensOnClick) return undefined;
    const closeOnOutsideClick = (event: MouseEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (!target) return;
      if (popupRef.current?.contains(target)) return;
      if (target.closest('.ant-dropdown-trigger')) return;
      setOpen(false);
    };

    document.addEventListener('click', closeOnOutsideClick);
    return () => document.removeEventListener('click', closeOnOutsideClick);
  }, [open, opensOnClick]);

  useEffect(() => {
    return () => clearCloseTimer();
  }, []);

  const triggerElement = cloneElement(children, {
    className: mergeClassName(
      children.props.className,
      'ant-dropdown-trigger',
      open && 'ant-dropdown-open',
    ),
    onClick: (event: ReactMouseEvent<HTMLElement>) => {
      children.props.onClick?.(event);
      if (opensOnClick) {
        if (open) {
          setOpen(false);
        } else {
          openFromElement(event.currentTarget);
        }
        return;
      }
      openFromElement(event.currentTarget);
    },
    onMouseEnter: (event: ReactMouseEvent<HTMLElement>) => {
      children.props.onMouseEnter?.(event);
      if (opensOnHover) openFromElement(event.currentTarget);
    },
    onMouseLeave: (event: ReactMouseEvent<HTMLElement>) => {
      children.props.onMouseLeave?.(event);
      scheduleHoverClose();
    },
  });

  return (
    <>
      {triggerElement}
      {hasOpened && coords && typeof document !== 'undefined'
        ? createPortal(
            <div
              ref={popupRef}
              className={legacyDropdownClassName(open)}
              style={{
                position: 'fixed',
                top: coords.top,
                left: coords.left,
                minWidth: coords.minWidth,
              }}
              onClick={() => setOpen(false)}
              onMouseEnter={clearCloseTimer}
              onMouseLeave={scheduleHoverClose}
            >
              {overlay}
            </div>,
            document.body,
          )
        : null}
    </>
  );
}

function LegacyDropdownMenu({ children }: { children: ReactNode }) {
  return (
    <ul className="ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical">
      {children}
    </ul>
  );
}

function LegacyDropdownMenuItem({
  children,
  onClick,
  onContextMenu,
  style,
}: LegacyDropdownMenuItemProps) {
  return (
    <li
      className="ant-dropdown-menu-item"
      role="menuitem"
      style={style}
      onClick={onClick}
      onContextMenu={onContextMenu}
    >
      {children}
    </li>
  );
}

function LegacyTag({
  children,
  color,
  style,
}: {
  children: ReactNode;
  color?: string;
  style?: CSSProperties;
}) {
  return (
    <span
      className={color ? 'ant-tag ant-tag-has-color' : 'ant-tag'}
      style={color ? { ...style, backgroundColor: color } : style}
    >
      {children}
    </span>
  );
}

function readLegacyServerPageSize() {
  const pageSize = Number(readLegacyHabit(LEGACY_SERVER_PAGE_SIZE_KEY));
  return Number.isFinite(pageSize) && pageSize > 0 ? pageSize : 10;
}

export default function ServersPage() {
  const location = useLocation();
  if (location.pathname === '/server/group') return <ServerGroupPage />;
  if (location.pathname === '/server/route') return <ServerRoutePage />;
  if (location.pathname === '/server/manage') return <ServerManagePage />;

  return null;
}

function ServerGroupPage() {
  const groups = useServerGroups();
  const drop = useDropServerGroupMutation();
  const groupItems = groups.data ?? [];
  const headers: LegacyStandaloneTableHeader[] = [
    { title: '组ID' },
    { title: '组名称' },
    { title: '用户数量' },
    { title: '节点数量' },
    { title: '操作', alignRight: true },
  ];

  return (
    <>
      <div className="d-flex justify-content-between align-items-center" />
      <LegacySpin loading={groups.isFetching}>
        <div className="block block-rounded">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <ServerGroupModal>
                <LegacyButton className="ant-btn">
                  <LegacyPlusIcon />
                  <span> 添加权限组</span>
                </LegacyButton>
              </ServerGroupModal>
            </div>
            <LegacyStandaloneTable headers={headers} isEmpty={groupItems.length === 0}>
              {groupItems.map((record, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  {...legacyRowKey(index)}
                >
                  <td className="">{record.id}</td>
                  <td className="">{record.name}</td>
                  <td className="">
                    <LegacyUserIcon style={{ cursor: 'move' }} /> {record.user_count}
                  </td>
                  <td className="">
                    <LegacyDatabaseIcon style={{ cursor: 'move' }} /> {record.server_count}
                  </td>
                  <td className="" style={{ textAlign: 'right' }}>
                    <div>
                      <ServerGroupModal key={record.id} record={record}>
                        <a ref={legacyHref()}>编辑</a>
                      </ServerGroupModal>
                      <div className="ant-divider ant-divider-vertical" role="separator" />
                      <a
                        ref={legacyHref()}
                        onClick={() =>
                          drop.mutate(record.id, {
                            onSuccess: () => {
                              void groups.refetch();
                            },
                          })
                        }
                      >
                        删除
                      </a>
                    </div>
                  </td>
                </tr>
              ))}
            </LegacyStandaloneTable>
          </div>
        </div>
      </LegacySpin>
    </>
  );
}

function ServerGroupModal({
  record,
  children,
}: {
  record?: admin.ServerGroup;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const groups = useServerGroups();
  const save = useSaveServerGroupMutation();
  const [visible, setVisible] = useState(false);
  const [submit, setSubmit] = useState<Partial<admin.ServerGroup>>(record ?? {});

  const open = () => {
    setVisible(true);
  };

  const saveGroup = async () => {
    await save.mutateAsync({ ...submit });
    void groups.refetch();
    setVisible(false);
  };

  return (
    <>
      {cloneElement(children, { onClick: open })}
      <LegacyModal
        title={`${submit.id ? '编辑组' : '创建组'}`}
        visible={visible}
        onCancel={() => setVisible(false)}
        onOk={() => {
          if (groups.isFetching) return;
          void saveGroup();
        }}
        okText={groups.isFetching ? <LegacyLoadingIcon /> : '提交'}
        cancelText="取消"
      >
        <div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">组名</label>
            <LegacyInput
              className="ant-input"
              placeholder="请输入组名"
              value={submit.name}
              onChange={(event) => setSubmit((value) => ({ ...value, name: event.target.value }))}
            />
          </div>
        </div>
      </LegacyModal>
    </>
  );
}

function getRouteMatchLabel(value: admin.ServerRoute['match'] | undefined) {
  if (!value || value.length === 0) return '无规则时默认';
  const rules = typeof value === 'string' ? value.split(',').filter(Boolean) : value;
  return `匹配 ${rules.length} 条规则`;
}

function getRouteMatchTextareaValue(value: admin.ServerRoute['match'] | undefined) {
  if (Array.isArray(value)) return value.join('\n');
  return value?.split(',').join('\n');
}

function getRouteMatchPlaceholder(action: string | undefined) {
  if (action === 'protocol') return 'http\ntls\nquic\nbittorrent';
  if (action === 'block_port') return '53\n443\n1000-2000';
  if (action && ['route_ip', 'block_ip'].includes(action)) {
    return '127.0.0.1(单一匹配)\n10.0.0.0/8(范围匹配)\ngeoip:cn(预定义列表匹配)';
  }
  return 'example.com(关键字匹配)\ndomain:example.com(子域名匹配)\ngeosite:netflix(预定义域名列表)';
}

function ServerRoutePage() {
  const routes = useServerRoutes();
  const drop = useDropServerRouteMutation();
  const routeItems = routes.data ?? [];
  const headers: LegacyStandaloneTableHeader[] = [
    { title: 'ID' },
    { title: '备注' },
    { title: '匹配数量' },
    { title: '动作' },
    { title: '操作', alignRight: true },
  ];

  return (
    <>
      <div className="d-flex justify-content-between align-items-center" />
      <LegacySpin loading={routes.isFetching}>
        <div className="block block-rounded">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <ServerRouteModal>
                <LegacyButton className="ant-btn">
                  <LegacyPlusIcon />
                  <span> 添加路由</span>
                </LegacyButton>
              </ServerRouteModal>
            </div>
            <LegacyStandaloneTable headers={headers} isEmpty={routeItems.length === 0}>
              {routeItems.map((record, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  {...legacyRowKey(index)}
                >
                  <td className="">{record.id}</td>
                  <td className="">{record.remarks}</td>
                  <td className="">{getRouteMatchLabel(record.match)}</td>
                  <td className="">{ROUTE_ACTION_TEXT[record.action]}</td>
                  <td className="" style={{ textAlign: 'right' }}>
                    <div>
                      <ServerRouteModal key={record.id} route={record}>
                        <a ref={legacyHref()}>编辑</a>
                      </ServerRouteModal>
                      <div className="ant-divider ant-divider-vertical" role="separator" />
                      <a
                        ref={legacyHref()}
                        onClick={() =>
                          drop.mutate(record.id, {
                            onSuccess: () => {
                              void routes.refetch();
                            },
                          })
                        }
                      >
                        删除
                      </a>
                    </div>
                  </td>
                </tr>
              ))}
            </LegacyStandaloneTable>
          </div>
        </div>
      </LegacySpin>
    </>
  );
}

function ServerRouteModal({
  route: initialRoute,
  children,
}: {
  route?: admin.ServerRoute;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const routes = useServerRoutes();
  const save = useSaveServerRouteMutation();
  const [visible, setVisible] = useState(false);
  const [route, setRoute] = useState<Partial<admin.ServerRoute>>(initialRoute ?? {});
  const routeActionOptions: LegacySelectOption[] = [
    { value: 'block', label: ROUTE_ACTION_TEXT.block },
    { value: 'block_ip', label: ROUTE_ACTION_TEXT.block_ip },
    { value: 'block_port', label: ROUTE_ACTION_TEXT.block_port },
    { value: 'protocol', label: ROUTE_ACTION_TEXT.protocol },
    { value: 'dns', label: ROUTE_ACTION_TEXT.dns },
    { value: 'route', label: ROUTE_ACTION_TEXT.route },
    { value: 'route_ip', label: ROUTE_ACTION_TEXT.route_ip },
    { value: 'default_out', label: ROUTE_ACTION_TEXT.default_out },
  ];

  const open = () => {
    setVisible(true);
  };

  const saveRoute = async () => {
    const payload = { ...route };
    if (Array.isArray(payload.match)) {
      payload.match = payload.match.filter(Boolean);
    } else if (payload.match && typeof payload.match === 'string') {
      payload.match = payload.match.split(',').filter(Boolean);
    } else {
      payload.match = [];
    }
    await save.mutateAsync(payload);
    void routes.refetch();
    setVisible(false);
  };

  return (
    <>
      {cloneElement(children, { onClick: open })}
      <LegacyModal
        title={`${route.id ? '编辑路由' : '创建路由'}`}
        visible={visible}
        onCancel={() => setVisible(false)}
        onOk={() => {
          if (routes.isFetching) return;
          void saveRoute();
        }}
        okText={routes.isFetching ? <LegacyLoadingIcon /> : '提交'}
        cancelText="取消"
      >
        <div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">备注</label>
            <LegacyInput
              className="ant-input"
              placeholder="请输入备注"
              value={route.remarks}
              onChange={(event) => setRoute((value) => ({ ...value, remarks: event.target.value }))}
            />
          </div>
          {route.action !== 'default_out' ? (
            <div className="form-group">
              <label htmlFor="example-text-input-alt">
                匹配值
                <a href="https://xtls.github.io/config/routing.html#ruleobject">
                  <LegacyLinkIcon />
                  填写参考
                </a>
              </label>
              <LegacyTextArea
                className="ant-input"
                rows={5}
                placeholder={getRouteMatchPlaceholder(route.action)}
                value={getRouteMatchTextareaValue(route.match)}
                onChange={(event) =>
                  setRoute((value) => ({
                    ...value,
                    match: event.target.value?.split('\n'),
                  }))
                }
              />
            </div>
          ) : null}
          <div className="form-group">
            <label htmlFor="example-text-input-alt">动作</label>
            <div>
              <LegacySelect
                value={route.action}
                placeholder="请选择动作"
                style={{ width: '100%' }}
                options={routeActionOptions}
                onChange={(value) => setRoute((route) => ({ ...route, action: value as string }))}
              />
            </div>
          </div>
          {route.action === 'dns' ? (
            <div className="form-group">
              <label htmlFor="example-text-input-alt">DNS服务器</label>
              <LegacyInput
                className="ant-input"
                placeholder="请输入用于解析的DNS服务器地址"
                value={legacyInputValue(route.action_value)}
                onChange={(event) =>
                  setRoute((value) => ({ ...value, action_value: event.target.value }))
                }
              />
            </div>
          ) : null}
          {route.action === 'route' ||
          route.action === 'route_ip' ||
          route.action === 'default_out' ? (
            <div className="form-group">
              <label htmlFor="example-text-input-alt">
                Xray出站配置
                <a href="https://xtls.github.io/config/outbound.html">
                  <LegacyLinkIcon />
                  填写参考
                </a>
              </label>
              <LegacyTextArea
                className="ant-input"
                rows={8}
                placeholder={JSON.stringify(
                  {
                    tag: 'ss_out',
                    sendThrough: '0.0.0.0',
                    protocol: 'shadowsocks',
                    settings: {
                      email: 'love@xray.com',
                      address: '8.8.8.8',
                      port: 5555,
                      method: 'chacha20-ietf-poly1305',
                      password: 'abcdefghijklmnopqrstuvwxyz',
                      level: 0,
                    },
                  },
                  null,
                  4,
                )}
                value={legacyInputValue(route.action_value)}
                onChange={(event) =>
                  setRoute((value) => ({ ...value, action_value: event.target.value }))
                }
              />
            </div>
          ) : null}
        </div>
      </LegacyModal>
    </>
  );
}

function getServerTypeTag(type: string, label: ReactNode) {
  return <LegacyTag color={SERVER_TYPE_COLORS[type]}>{label}</LegacyTag>;
}

function getLegacyAvailableStatus(status?: number | null) {
  return status == null ? undefined : AVAILABLE_STATUS[status];
}

function LegacyStatusBadge({
  status,
}: {
  status?: 'error' | 'warning' | 'processing' | 'success' | 'default';
}) {
  return (
    <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
      <span className={`ant-badge-status-dot${status ? ` ant-badge-status-${status}` : ''}`} />
      <span className="ant-badge-status-text" />
    </span>
  );
}

export function createServerSortPayload(nodes: admin.ServerNode[]) {
  return nodes.reduce<Record<string, Record<string | number, number>>>((payload, node, index) => {
    const typePayload = payload[node.type] ?? {};
    typePayload[node.id] = index;
    payload[node.type] = typePayload;
    return payload;
  }, {});
}

export function moveServerNodeByLegacyDragIndexes(
  nodes: admin.ServerNode[],
  fromIndex: number,
  toIndex: number,
) {
  const next = [...nodes];
  const moved = next[fromIndex];
  if (!moved || fromIndex === toIndex) return next;
  if (fromIndex < toIndex) {
    next.splice(toIndex + 1, 0, moved);
    next.splice(fromIndex, 1);
  } else {
    next.splice(toIndex, 0, moved);
    next.splice(fromIndex + 1, 1);
  }
  return next;
}

function isLegacyMobile() {
  if (typeof window === 'undefined') return false;
  return window.navigator.userAgent.toLowerCase().includes('mobile');
}

export function shouldPromptLegacyServerSortClick(target: EventTarget | null) {
  if (!(target instanceof Element)) return false;
  if (target.closest('.nav-main-link')) return true;
  if (target.closest('.dropdown-item')) return true;

  const anchor = target.closest('a');
  if (!anchor) return false;
  const href = anchor.getAttribute('href');
  return Boolean(href && !href.startsWith('javascript:'));
}

export function installLegacyServerSortPrompt(message = LEGACY_SERVER_SORT_PROMPT) {
  if (typeof window === 'undefined') return () => {};

  let lastHash = window.location.hash;
  let restoringHash = false;

  const confirmLeave = () => window.confirm(message);

  const warnBeforeUnload = (event: BeforeUnloadEvent) => {
    event.preventDefault();
    event.returnValue = message;
    return message;
  };

  const warnBeforeRouteClick = (event: MouseEvent) => {
    if (!shouldPromptLegacyServerSortClick(event.target)) return;
    if (confirmLeave()) return;
    event.preventDefault();
    event.stopImmediatePropagation();
  };

  const warnBeforeHashChange = () => {
    if (restoringHash) {
      restoringHash = false;
      return;
    }
    if (window.location.hash === lastHash) return;
    if (confirmLeave()) {
      lastHash = window.location.hash;
      return;
    }
    restoringHash = true;
    window.location.hash = lastHash || '#/server/manage';
  };

  window.addEventListener('beforeunload', warnBeforeUnload);
  document.addEventListener('click', warnBeforeRouteClick, true);
  window.addEventListener('hashchange', warnBeforeHashChange);

  return () => {
    window.removeEventListener('beforeunload', warnBeforeUnload);
    document.removeEventListener('click', warnBeforeRouteClick, true);
    window.removeEventListener('hashchange', warnBeforeHashChange);
  };
}

function LegacyServerSortPrompt({ when }: { when: boolean }) {
  useEffect(() => {
    if (!when) return undefined;
    return installLegacyServerSortPrompt();
  }, [when]);

  return null;
}

function LegacyNodeEditMenuTrigger({
  type,
  record,
  nodes,
  groups,
  routes,
  onSaved,
  children,
}: {
  type: admin.ServerTypeName;
  record?: Partial<admin.ServerNode>;
  nodes: admin.ServerNode[];
  groups: admin.ServerGroup[];
  routes: admin.ServerRoute[];
  onSaved: () => void | Promise<unknown>;
  children: ReactElement<{ onClick?: (event: ReactMouseEvent<HTMLElement>) => void }>;
}) {
  const [open, setOpen] = useState(false);

  return (
    <>
      {cloneElement(children, {
        onClick: (event: ReactMouseEvent<HTMLElement>) => {
          children.props.onClick?.(event);
          setOpen(true);
        },
      })}
      <NodeEditDrawer
        open={open}
        type={type}
        id={record?.id}
        record={record}
        nodes={nodes}
        groups={groups}
        routes={routes}
        onSaved={onSaved}
        onClose={() => setOpen(false)}
      />
    </>
  );
}

function LegacyServerMobileNodeList({
  nodes,
  actionMenu,
  onToggleNodeShow,
}: {
  nodes: admin.ServerNode[];
  actionMenu: (node: admin.ServerNode) => ReactNode;
  onToggleNodeShow: (node: admin.ServerNode) => void;
}) {
  return (
    <div className="ant-list ant-list-vertical ant-list-split v2board-table">
      <div className="ant-spin-nested-loading">
        <div className="ant-spin-container">
          {nodes.length ? (
            <ul className="ant-list-items">
              {nodes.map((node) => (
                <li
                  key={node.id}
                  className={`ant-list-item ant-list-item-no-flex v2board_node_mobile ${
                    node.parent_id ? 'child_node' : ''
                  }`}
                >
                  <div className="ant-list-item-main">
                    <div className="ant-list-item-meta">
                      <div className="ant-list-item-meta-content">
                        <h4 className="ant-list-item-meta-title">
                          <LegacyStatusBadge
                            status={getLegacyAvailableStatus(node.available_status)}
                          />
                          {node.name}
                        </h4>
                        <div className="ant-list-item-meta-description">
                          {node.host}:{node.port}
                        </div>
                      </div>
                    </div>
                    <ul className="ant-list-item-action">
                      <li>
                        <span>
                          {getServerTypeTag(
                            node.type,
                            node.parent_id ? `${node.id} => ${node.parent_id}` : node.id,
                          )}
                          <LegacyTag>
                            <LegacyUserIcon /> {node.online || 0}
                          </LegacyTag>
                          <LegacyTag>{node.rate} x</LegacyTag>
                        </span>
                      </li>
                    </ul>
                  </div>
                  <div className="ant-list-item-extra">
                    <LegacySwitch
                      size="small"
                      checked={parseInt(String(node.show), 10) as unknown as boolean}
                      onChange={() => onToggleNodeShow(node)}
                    />
                    <div className="ant-divider ant-divider-vertical" />
                    <span>
                      <LegacyDropdown
                        trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}
                        overlay={actionMenu(node)}
                      >
                        <a ref={legacyHref()}>
                          操作 <LegacyCaretDownIcon />
                        </a>
                      </LegacyDropdown>
                    </span>
                  </div>
                </li>
              ))}
            </ul>
          ) : (
            <div className="ant-list-empty-text">
              <LegacyEmpty />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function ServerManagePage() {
  const { message } = App.useApp();
  const nodes = useServerNodes();
  const groups = useServerGroups();
  const routes = useServerRoutes();
  const update = useUpdateServerMutation();
  const drop = useDropServerMutation();
  const copy = useCopyServerMutation();
  const sort = useSortServerNodesMutation();
  const [searchKey, setSearchKey] = useState<string | undefined>();
  const [sortMode, setSortMode] = useState(false);
  const [orderedNodes, setOrderedNodes] = useState<admin.ServerNode[]>(() => nodes.data ?? []);
  const [sortingLoading, setSortingLoading] = useState(false);
  const [pageSize, setPageSize] = useState(readLegacyServerPageSize);
  const [contextRecord, setContextRecord] = useState<admin.ServerNode | null>(null);
  const [contextMenu, setContextMenu] = useState<{ top: number; left: number } | null>(null);
  const orderRef = useRef(orderedNodes);
  const mobile = isLegacyMobile();

  useEffect(() => {
    if (nodes.data) {
      setOrderedNodes(nodes.data);
      setSortingLoading(false);
      setSortMode(false);
    }
  }, [nodes.data]);

  orderRef.current = orderedNodes;

  const filteredNodes =
    searchKey && orderedNodes
      ? orderedNodes.filter((node) => JSON.stringify(node).includes(searchKey))
      : orderedNodes;

  const sortServerNodes = (fromIndex: number, toIndex: number) => {
    setOrderedNodes(moveServerNodeByLegacyDragIndexes(orderRef.current, fromIndex, toIndex));
  };

  const groupName = (ids: admin.ServerNode['group_id']) =>
    ids.map((id) => groups.data?.find((group) => group.id === Number(id))?.name).filter(Boolean);

  const toggleNodeShow = (row: admin.ServerNode) => {
    const checked = parseInt(String(row.show), 10);
    update.mutate(
      {
        type: row.type as admin.ServerTypeName,
        id: row.id,
        key: 'show',
        value: checked ? 0 : 1,
      },
      {
        onSuccess: () => {
          void nodes.refetch();
        },
      },
    );
  };

  const runNodeAction = (key: string, row: admin.ServerNode) => {
    setContextMenu(null);
    if (key === 'copy') {
      copy.mutate(
        { type: row.type as admin.ServerTypeName, id: row.id },
        {
          onSuccess: () => {
            void nodes.refetch();
          },
        },
      );
    }
    if (key === 'delete') {
      drop.mutate(
        { type: row.type as admin.ServerTypeName, id: row.id },
        {
          onSuccess: () => {
            void nodes.refetch();
          },
        },
      );
    }
  };

  const actionMenu = (row: admin.ServerNode) => (
    <LegacyDropdownMenu>
      <LegacyDropdownMenuItem onContextMenu={(event) => event.stopPropagation()}>
        <LegacyNodeEditMenuTrigger
          key={row.id}
          type={row.type as admin.ServerTypeName}
          record={row}
          nodes={nodes.data ?? []}
          groups={groups.data ?? []}
          routes={routes.data ?? []}
          onSaved={() => nodes.refetch()}
        >
          <a>
            <LegacyEditIcon /> 编辑
          </a>
        </LegacyNodeEditMenuTrigger>
      </LegacyDropdownMenuItem>
      <LegacyDropdownMenuItem onClick={() => runNodeAction('copy', row)}>
        <LegacyCopyIcon /> 复制
      </LegacyDropdownMenuItem>
      <LegacyDropdownMenuItem
        style={{ color: '#ff4d4f' }}
        onClick={() => runNodeAction('delete', row)}
      >
        <LegacyDeleteIcon /> 删除
      </LegacyDropdownMenuItem>
    </LegacyDropdownMenu>
  );

  const tableClassName = [
    'ant-table',
    'ant-table-default',
    filteredNodes.length ? '' : 'ant-table-empty',
    'ant-table-scroll-position-left',
  ]
    .filter(Boolean)
    .join(' ');
  const visibleNodes = sortMode ? filteredNodes : filteredNodes.slice(0, pageSize);
  const changeServerPageSize = (_current: number, size: number) => {
    setPageSize(size);
    writeLegacyHabit(LEGACY_SERVER_PAGE_SIZE_KEY, size);
  };
  const rowHandlers = (record: admin.ServerNode) =>
    sortMode
      ? {}
      : ({
          onClick: () => setContextMenu(null),
          onContextMenu: (event: ReactMouseEvent<HTMLTableRowElement>) => {
            event.preventDefault();
            setContextRecord(record);
            setContextMenu({ top: event.clientY, left: event.clientX });
          },
        } satisfies HTMLAttributes<HTMLTableRowElement>);
  const actionCell = (row: admin.ServerNode) => (
    <div>
      <LegacyDropdown trigger={LEGACY_DROPDOWN_CLICK_TRIGGER} overlay={actionMenu(row)}>
        <a ref={legacyHref()}>
          操作 <LegacyCaretDownIcon />
        </a>
      </LegacyDropdown>
    </div>
  );
  const headerColumn = (title: ReactNode, sorter?: ReactNode) => (
    <span className="ant-table-header-column">
      <div>
        <span className="ant-table-column-title">{title}</span>
        {sorter ?? <span className="ant-table-column-sorter" />}
      </div>
    </span>
  );
  const filterIcon = (
    <LegacyFilterIcon title="筛选" tabIndex={-1} className="ant-dropdown-trigger" />
  );
  const sorterIcon = (
    <span className="ant-table-column-sorter">
      <div
        title="排序"
        className="ant-table-column-sorter-inner ant-table-column-sorter-inner-full"
      >
        <LegacyCaretUpIcon className="ant-table-column-sorter-up off" />
        <LegacyCaretDownIcon className="ant-table-column-sorter-down off" />
      </div>
    </span>
  );
  const contextDropdown = (
    <div
      id="v2board-table-dropdown"
      className="ant-dropdown ant-dropdown-placement-bottomLeft"
      style={{
        display: contextMenu && !sortMode ? 'unset' : 'none',
        position: 'fixed',
        top: contextMenu?.top ?? 0,
        left: contextMenu?.left ?? 0,
      }}
      onClick={() => setContextMenu(null)}
    >
      <ul className="ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical">
        <li className="ant-dropdown-menu-item">
          {contextRecord ? (
            <LegacyNodeEditMenuTrigger
              key={Math.random()}
              type={contextRecord.type as admin.ServerTypeName}
              record={contextRecord}
              nodes={nodes.data ?? []}
              groups={groups.data ?? []}
              routes={routes.data ?? []}
              onSaved={() => nodes.refetch()}
            >
              <a>
                <LegacyFormIcon /> 编辑
              </a>
            </LegacyNodeEditMenuTrigger>
          ) : null}
        </li>
        <li className="ant-dropdown-menu-item">
          <a onClick={() => contextRecord && runNodeAction('copy', contextRecord)}>
            <LegacyCopyIcon /> 复制
          </a>
        </li>
        <li className="ant-dropdown-menu-item">
          <a
            style={{ color: '#ff4d4f' }}
            onClick={() => contextRecord && runNodeAction('delete', contextRecord)}
          >
            <LegacyDeleteIcon /> 删除
          </a>
        </li>
      </ul>
    </div>
  );

  return (
    <LegacySpin loading={nodes.isFetching || sortingLoading}>
      <LegacyServerSortPrompt when={sortMode} />
      <div className="block block-bottom undefined">
        <div className="bg-white">
          <div className="v2board-table-action" style={{ padding: 15 }}>
            <LegacyDropdown
              overlay={
                <LegacyDropdownMenu>
                  {SERVER_TYPES.map((type) => (
                    <LegacyDropdownMenuItem key={type}>
                      <LegacyNodeEditMenuTrigger
                        key={Math.random()}
                        type={type}
                        nodes={nodes.data ?? []}
                        groups={groups.data ?? []}
                        routes={routes.data ?? []}
                        onSaved={() => nodes.refetch()}
                      >
                        <a ref={legacyHref()}>{getServerTypeTag(type, SERVER_TYPE_LABELS[type])}</a>
                      </LegacyNodeEditMenuTrigger>
                    </LegacyDropdownMenuItem>
                  ))}
                </LegacyDropdownMenu>
              }
            >
              <LegacyButton className="ant-btn">
                <LegacyPlusIcon />
              </LegacyButton>
            </LegacyDropdown>
            <LegacyInput
              placeholder="输入任意关键字搜索"
              style={{ width: 200 }}
              className="ant-input ml-2"
              onChange={(event) => setSearchKey(event.target.value)}
            />
            {!mobile && (
              <LegacyButton
                style={{ float: 'right' }}
                className="ant-btn ant-btn-primary"
                onClick={() => {
                  if (!sortMode) {
                    setSortMode(true);
                    return;
                  }
                  setSortingLoading(true);
                  sort.mutate(createServerSortPayload(orderedNodes), {
                    onSuccess: () => {
                      void nodes.refetch();
                    },
                    onSettled: () => setSortingLoading(false),
                  });
                }}
              >
                {sortMode ? '保存排序' : '编辑排序'}
              </LegacyButton>
            )}
          </div>
          {mobile ? (
            <LegacyServerMobileNodeList
              nodes={filteredNodes}
              actionMenu={actionMenu}
              onToggleNodeShow={toggleNodeShow}
            />
          ) : (
            <LegacyDragSort
              onDragEnd={(fromIndex, toIndex) => sortServerNodes(fromIndex, toIndex)}
              nodeSelector="tr"
              handleSelector="i"
            >
              <div className="ant-table-wrapper">
                <div className="ant-spin-nested-loading">
                  <div className="ant-spin-container">
                    <div className={tableClassName}>
                      <div className="ant-table-content">
                        <div className="ant-table-scroll">
                          <div
                            tabIndex={-1}
                            className="ant-table-body"
                            style={{ overflowX: 'scroll' }}
                          >
                            <table className="ant-table-fixed" style={{ width: 1300 }}>
                              <colgroup>
                                <col style={{ width: 150, minWidth: 150 }} />
                                <col />
                                <col />
                                <col />
                                <col style={{ width: 130, minWidth: 130 }} />
                                <col />
                                <col />
                                <col style={{ width: 100, minWidth: 100 }} />
                              </colgroup>
                              {sortMode ? (
                                <>
                                  <thead className="ant-table-thead">
                                    <tr>
                                      <th
                                        className="ant-table-align-left"
                                        style={{ textAlign: 'left' }}
                                      >
                                        {headerColumn('排序')}
                                      </th>
                                      <th className="ant-table-row-cell-break-word">
                                        {headerColumn('节点ID')}
                                      </th>
                                      <th className="">{headerColumn('节点')}</th>
                                    </tr>
                                  </thead>
                                  <tbody className="ant-table-tbody">
                                    {visibleNodes.map((node, index) => (
                                      <tr
                                        {...legacyRowKey(index)}
                                        key={index}
                                        className={`ant-table-row ant-table-row-level-0${node.parent_id ? ' child_node' : ''}`}
                                      >
                                        <td>
                                          <div>
                                            <span style={{ cursor: 'move' }} title="拖动排序">
                                              <LegacyMenuIcon />
                                            </span>
                                          </div>
                                        </td>
                                        <td>
                                          {getServerTypeTag(
                                            node.type,
                                            node.parent_id
                                              ? `${node.id} => ${node.parent_id}`
                                              : node.id,
                                          )}
                                        </td>
                                        <td>{node.name}</td>
                                      </tr>
                                    ))}
                                  </tbody>
                                </>
                              ) : (
                                <>
                                  <thead className="ant-table-thead">
                                    <tr>
                                      <th className="ant-table-column-has-actions ant-table-column-has-filters ant-table-row-cell-break-word">
                                        {headerColumn('节点ID')}
                                        {filterIcon}
                                      </th>
                                      <th className="">{headerColumn('显隐')}</th>
                                      <th className="">
                                        {headerColumn(
                                          <span>
                                            <span>
                                              节点 <LegacyQuestionCircleIcon />
                                            </span>
                                          </span>,
                                        )}
                                      </th>
                                      <th className="">{headerColumn('地址')}</th>
                                      <th
                                        className="ant-table-column-has-actions ant-table-column-has-sorters ant-table-align-left ant-table-row-cell-break-word"
                                        style={{ textAlign: 'left' }}
                                      >
                                        <span className="ant-table-header-column">
                                          <div className="ant-table-column-sorters">
                                            <span className="ant-table-column-title">
                                              <span>
                                                <span>
                                                  人数 <LegacyQuestionCircleIcon />
                                                </span>
                                              </span>
                                            </span>
                                            {sorterIcon}
                                          </div>
                                        </span>
                                      </th>
                                      <th
                                        className="ant-table-align-center"
                                        style={{ textAlign: 'center' }}
                                      >
                                        {headerColumn(
                                          <span>
                                            倍率 <LegacyQuestionCircleIcon />
                                          </span>,
                                        )}
                                      </th>
                                      <th className="ant-table-column-has-actions ant-table-column-has-filters">
                                        {headerColumn('权限组')}
                                        {filterIcon}
                                      </th>
                                      <th
                                        className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-break-word ant-table-row-cell-last"
                                        style={{ textAlign: 'right' }}
                                      >
                                        {headerColumn('操作')}
                                      </th>
                                    </tr>
                                  </thead>
                                  <tbody className="ant-table-tbody">
                                    {visibleNodes.map((node, index) => {
                                      const checked = parseInt(String(node.show), 10);
                                      return (
                                        <tr
                                          {...legacyRowKey(index)}
                                          {...rowHandlers(node)}
                                          key={index}
                                          className={`ant-table-row ant-table-row-level-0${node.parent_id ? ' child_node' : ''}`}
                                        >
                                          <td>
                                            {getServerTypeTag(
                                              node.type,
                                              node.parent_id
                                                ? `${node.id} => ${node.parent_id}`
                                                : node.id,
                                            )}
                                          </td>
                                          <td>
                                            <LegacySwitch
                                              size="small"
                                              checked={checked as unknown as boolean}
                                              onChange={() => toggleNodeShow(node)}
                                            />
                                          </td>
                                          <td>
                                            <LegacyStatusBadge
                                              status={getLegacyAvailableStatus(
                                                node.available_status,
                                              )}
                                            />
                                            <span>{node.name}</span>
                                          </td>
                                          <td>
                                            <span
                                              style={{ cursor: 'pointer' }}
                                              onClick={() => {
                                                legacyCopyText(node.host);
                                                message.success('复制成功');
                                              }}
                                            >
                                              {node.host}:{node.port}
                                            </span>
                                          </td>
                                          <td style={{ textAlign: 'left' }}>
                                            <LegacyUserIcon /> {node.online || 0}
                                          </td>
                                          <td style={{ textAlign: 'center' }}>
                                            <LegacyTag style={{ minWidth: 60 }}>
                                              {node.rate} x
                                            </LegacyTag>
                                          </td>
                                          <td>
                                            {groupName(node.group_id).map((name) => (
                                              <LegacyTag key={name}>{name}</LegacyTag>
                                            ))}
                                          </td>
                                          <td
                                            className="ant-table-fixed-columns-in-body"
                                            style={{ textAlign: 'right' }}
                                          >
                                            {actionCell(node)}
                                          </td>
                                        </tr>
                                      );
                                    })}
                                  </tbody>
                                </>
                              )}
                            </table>
                          </div>
                          {filteredNodes.length === 0 && (
                            <div className="ant-table-placeholder">
                              <LegacyEmpty />
                            </div>
                          )}
                        </div>
                        {!sortMode && (
                          <>
                            <div className="ant-table-fixed-right">
                              <div className="ant-table-body-outer">
                                <div className="ant-table-body-inner">
                                  <table className="ant-table-fixed">
                                    <colgroup>
                                      <col style={{ width: 100, minWidth: 100 }} />
                                    </colgroup>
                                    <thead className="ant-table-thead">
                                      <tr style={{ height: 54 }}>
                                        <th
                                          className="ant-table-align-right ant-table-row-cell-break-word ant-table-row-cell-last"
                                          style={{ textAlign: 'right' }}
                                        >
                                          {headerColumn('操作')}
                                        </th>
                                      </tr>
                                    </thead>
                                    <tbody className="ant-table-tbody">
                                      {visibleNodes.map((node, index) => (
                                        <tr
                                          {...legacyRowKey(index)}
                                          key={index}
                                          className={`ant-table-row ant-table-row-level-0${node.parent_id ? ' child_node' : ''}`}
                                        >
                                          <td style={{ textAlign: 'right' }}>{actionCell(node)}</td>
                                        </tr>
                                      ))}
                                    </tbody>
                                  </table>
                                </div>
                              </div>
                            </div>
                          </>
                        )}
                      </div>
                      {!sortMode && (
                        <div style={{ position: 'absolute', top: 0, left: 0, width: '100%' }}>
                          <div>
                            <div className="ant-dropdown  ant-dropdown-placement-bottomRight  ant-dropdown-hidden">
                              <div className="ant-table-filter-dropdown">
                                <ul
                                  className="ant-dropdown-menu ant-dropdown-menu-without-submenu ant-dropdown-menu-root ant-dropdown-menu-vertical"
                                  role="menu"
                                  tabIndex={0}
                                >
                                  {(groups.data ?? []).map((group) => (
                                    <li
                                      className="ant-dropdown-menu-item"
                                      role="menuitem"
                                      key={group.id}
                                    >
                                      <label className="ant-checkbox-wrapper">
                                        <span className="ant-checkbox">
                                          <LegacyCheckboxInput
                                            className="ant-checkbox-input"
                                            value=""
                                          />
                                          <span className="ant-checkbox-inner" />
                                        </span>
                                      </label>
                                      <span>{group.name}</span>
                                    </li>
                                  ))}
                                </ul>
                                <div className="ant-table-filter-dropdown-btns">
                                  <a className="ant-table-filter-dropdown-link confirm">确定</a>
                                  <a className="ant-table-filter-dropdown-link clear">重置</a>
                                </div>
                              </div>
                            </div>
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              </div>
              {!sortMode && filteredNodes.length > pageSize && (
                <div className="ant-table-pagination ant-pagination">
                  <select
                    value={pageSize}
                    onChange={(event) => changeServerPageSize(1, Number(event.target.value))}
                  >
                    {[10, 50, 100, 500].map((size) => (
                      <option key={size} value={size}>
                        {size} 条/页
                      </option>
                    ))}
                  </select>
                </div>
              )}
              {contextDropdown}
            </LegacyDragSort>
          )}
        </div>
      </div>
    </LegacySpin>
  );
}

function NodeEditDrawer({
  open,
  type,
  id,
  record,
  nodes,
  groups,
  routes,
  onSaved,
  onClose,
}: {
  open: boolean;
  type: admin.ServerTypeName;
  id?: number;
  record?: Partial<admin.ServerNode>;
  nodes: admin.ServerNode[];
  groups: admin.ServerGroup[];
  routes: admin.ServerRoute[];
  onSaved?: () => void | Promise<unknown>;
  onClose: () => void;
}) {
  const { message } = App.useApp();
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);
  const [childDrawer, setChildDrawer] = useState<{
    open: boolean;
    title?: string;
    field?: string;
  }>({ open: false });
  const parentCandidates = nodes.filter((node) => node.type === type && node.id !== id);
  const parentOptions: LegacySelectOption[] = [
    { value: '', label: '无' },
    ...parentCandidates.map((node) => ({ value: node.id, label: node.name })),
  ];
  const groupOptions: LegacySelectOption[] = groups.map((group) => ({
    value: group.id,
    label: group.name,
  }));
  const routeOptions: LegacySelectOption[] = routes.map((route) => ({
    value: route.id,
    label: route.remarks,
  }));

  const showChildDrawer = (title?: string, field?: string) => {
    setChildDrawer((current) => ({
      open: !current.open,
      title,
      field,
    }));
  };
  const initialValues = getLegacyServerInitialValues(type, record);

  return (
    <LegacyDrawer
      id="server"
      maskClosable
      open={open}
      title={id ? '编辑节点' : '新建节点'}
      onClose={onClose}
      width="80%"
    >
      <Form
        component={false}
        form={form}
        initialValues={initialValues}
        onFinish={async (values) => {
          setSaving(true);
          try {
            const payload = prepareLegacyServerPayload(type, values, id);
            await admin.saveServer(apiClient, type, payload);
            void onSaved?.();
            onClose();
          } catch (e) {
            if (e instanceof SyntaxError) {
              message.error('传输协议配置格式有误');
            } else if (e instanceof Error) {
              message.error(i18nGet(e.message));
            }
          } finally {
            setSaving(false);
          }
        }}
      >
        <div>
          <div className="row">
            <div className="form-group col-8">
              <label>节点名称</label>
              <Form.Item noStyle name="name">
                <Input placeholder="请输入节点名称" />
              </Form.Item>
            </div>
            <div className="form-group col-4">
              <label>倍率</label>
              <Form.Item noStyle name="rate">
                <Input addonAfter="x" placeholder="请输入节点倍率" />
              </Form.Item>
            </div>
          </div>
          <div className="form-group">
            <label>节点标签</label>
            <Form.Item
              noStyle
              name="tags"
              getValueFromEvent={normalizeLegacyNullableArray}
              getValueProps={(value) => ({ value: Array.isArray(value) ? value : [] })}
            >
              <LegacySelect
                mode="tags"
                style={{ width: '100%' }}
                placeholder="输入后回车添加标签"
                options={[]}
              />
            </Form.Item>
          </div>
          <div className="form-group">
            <label>
              权限组{' '}
              <LegacyTooltip>
                <a ref={legacyHref('javascript:(0);')}>添加权限组</a>
              </LegacyTooltip>
            </label>
            <Form.Item noStyle name="group_id">
              <LegacySelect
                mode="multiple"
                placeholder="请选择权限组"
                style={{ width: '100%' }}
                options={groupOptions}
              />
            </Form.Item>
          </div>
          {type === 'v2node' ? (
            <div className="row">
              <div className="form-group col-md-6 col-xs-12">
                <label>连接地址</label>
                <Form.Item noStyle name="host">
                  <Input placeholder="地址或IP" />
                </Form.Item>
              </div>
              <div className="form-group col-md-6 col-xs-12">
                <label>监听地址</label>
                <Form.Item noStyle name="listen_ip">
                  <Input placeholder="地址或IP默认为0.0.0.0" />
                </Form.Item>
              </div>
            </div>
          ) : type === 'vmess' || type === 'vless' ? (
            <div className="row">
              <div className="form-group col-md-8 col-xs-12">
                <label>节点地址</label>
                <Form.Item noStyle name="host">
                  <Input placeholder="请输入连接地址" />
                </Form.Item>
              </div>
              {type === 'vmess' ? (
                <VmessTlsField showChildDrawer={showChildDrawer} />
              ) : (
                <VlessSecurityField form={form} showChildDrawer={showChildDrawer} />
              )}
            </div>
          ) : (
            <div className="row">
              <div className="form-group col-md-12 col-xs-12">
                <label>节点地址</label>
                <Form.Item noStyle name="host">
                  <Input placeholder="地址或IP" />
                </Form.Item>
              </div>
            </div>
          )}
          {type === 'trojan' || type === 'hysteria' || type === 'tuic' || type === 'anytls' ? (
            <div className="row">
              <div className="form-group col-md-4 col-xs-12">
                <label>连接端口</label>
                <Form.Item noStyle name="port">
                  <Input placeholder="用户连接端口" />
                </Form.Item>
              </div>
              <div className="form-group col-md-4 col-xs-12">
                <label>服务端口</label>
                <Form.Item noStyle name="server_port">
                  <Input placeholder="服务端开放端口" />
                </Form.Item>
              </div>
              {type === 'trojan' ? <TrojanAllowInsecureField /> : <ServerInsecureField />}
            </div>
          ) : type === 'v2node' ? (
            <div className="row">
              <div className="form-group col-md-6 col-xs-12">
                <label>连接端口</label>
                <Form.Item noStyle name="port">
                  <Input placeholder="用户连接端口" />
                </Form.Item>
              </div>
              <div className="form-group col-md-6 col-xs-12">
                <label>服务端口</label>
                <Form.Item noStyle name="server_port">
                  <Input placeholder="服务端开放端口" />
                </Form.Item>
              </div>
            </div>
          ) : (
            <div className="row">
              <div className="form-group col-md-6 col-xs-12">
                <label>连接端口</label>
                <Form.Item noStyle name="port">
                  <Input placeholder="用户连接端口" />
                </Form.Item>
              </div>
              <div className="form-group col-md-6 col-xs-12">
                <label>服务端口</label>
                <Form.Item noStyle name="server_port">
                  <Input placeholder="非NAT同连接端口" />
                </Form.Item>
              </div>
            </div>
          )}
        </div>
        <ServerTypeFields type={type} form={form} showChildDrawer={showChildDrawer} />
        <div className="form-group">
          <label>
            <LegacyTooltip placement="top">
              父节点{' '}
              <a
                target="_blank"
                href="https://docs.v2board.com/use/node.html#父节点与子节点关系"
                rel="noreferrer"
              >
                {type === 'vmess' || type === 'vless' ? <LegacyReadIcon /> : '更多解答'}
              </a>
            </LegacyTooltip>
          </label>
          <Form.Item noStyle name="parent_id" getValueProps={(value) => ({ value: value || '' })}>
            <LegacySelect style={{ width: '100%' }} options={parentOptions} />
          </Form.Item>
        </div>
        <div className="form-group">
          <label>路由组</label>
          <Form.Item
            noStyle
            name="route_id"
            getValueFromEvent={normalizeLegacyNullableArray}
            getValueProps={(value) => ({ value: Array.isArray(value) ? value : [] })}
          >
            <LegacySelect
              mode="multiple"
              placeholder="请选择路由组"
              style={{ width: '100%' }}
              options={routeOptions}
            />
          </Form.Item>
        </div>
        <Form.Item noStyle name="show">
          <Input type="hidden" />
        </Form.Item>
        {type === 'v2node' ? (
          <div className="form-group">
            <label>一键安装指令</label>
            <Form.Item noStyle name="install_command">
              <Input.TextArea
                rows={4}
                readOnly
                style={{ backgroundColor: '#f5f5f5a0', cursor: 'text' }}
              />
            </Form.Item>
          </div>
        ) : null}
        {childDrawer.field ? (
          <LegacyDrawer
            closable={false}
            id="server"
            width="80%"
            title={childDrawer.title}
            open={childDrawer.open}
            onClose={() => showChildDrawer()}
          >
            <ServerChildDrawerField type={type} field={childDrawer.field} form={form} />
          </LegacyDrawer>
        ) : null}
      </Form>
      <div className="v2board-drawer-action">
        <LegacyButton className="ant-btn" style={{ marginRight: 8 }} onClick={onClose}>
          取消
        </LegacyButton>
        <LegacyButton
          className={`ant-btn ant-btn-primary${saving ? ' ant-btn-loading' : ''}`}
          onClick={() => form.submit()}
        >
          {saving ? <LegacyLoadingIcon /> : null}
          提交
        </LegacyButton>
      </div>
    </LegacyDrawer>
  );
}

function parseLegacyJsonPayloadField(payload: Record<string, unknown>, field: string) {
  const value = payload[field];
  if (!value) {
    payload[field] = null;
    return;
  }
  payload[field] = typeof value === 'string' ? JSON.parse(value) : value;
}

function normalizeLegacyNullableArray(value: unknown) {
  return Array.isArray(value) && value.length === 0 ? null : value;
}

function prepareLegacyServerPayload(
  type: admin.ServerTypeName,
  values: Record<string, unknown>,
  id?: number,
) {
  const payload: Record<string, unknown> = { ...values, id };
  if (type === 'vmess') {
    parseLegacyJsonPayloadField(payload, 'networkSettings');
    const dnsSettings = payload.dnsSettings as { servers?: unknown[] } | undefined;
    if (!dnsSettings?.servers?.length) payload.dnsSettings = null;
  }
  if (type === 'trojan' || type === 'vless' || type === 'v2node') {
    parseLegacyJsonPayloadField(payload, 'network_settings');
  }
  if (type === 'v2node') {
    delete payload.install_command;
  }
  return payload;
}

export function getLegacyServerInitialValues(
  type: admin.ServerTypeName,
  record?: Partial<admin.ServerNode>,
): Record<string, unknown> {
  const normalizedRecord: Record<string, unknown> = record
    ? { ...(record as Record<string, unknown>) }
    : {};
  if (normalizedRecord.networkSettings && typeof normalizedRecord.networkSettings === 'object') {
    normalizedRecord.networkSettings = JSON.stringify(normalizedRecord.networkSettings, null, 2);
  }
  if (normalizedRecord.network_settings && typeof normalizedRecord.network_settings === 'object') {
    normalizedRecord.network_settings = JSON.stringify(normalizedRecord.network_settings, null, 2);
  }
  const tuicDefaults =
    type === 'tuic'
      ? {
          insecure: 0,
          disable_sni: 0,
          udp_relay_mode: 'native',
          zero_rtt_handshake: 0,
          congestion_control: 'cubic',
        }
      : {};
  const shadowsocksDefaults = type === 'shadowsocks' ? { cipher: 'chacha20-ietf-poly1305' } : {};
  const vmessDefaults = type === 'vmess' ? { tls: 0 } : {};
  const trojanDefaults = type === 'trojan' ? { tls: 0 } : {};
  const hysteriaDefaults = type === 'hysteria' ? { insecure: 0, version: 1 } : {};
  const vlessDefaults = type === 'vless' ? { tls: 0, flow: null } : {};
  const anyTlsDefaults = type === 'anytls' ? { insecure: 0 } : {};
  const v2nodeDefaults =
    type === 'v2node'
      ? {
          tls: 0,
          network: 'tcp',
          disable_sni: 0,
          zero_rtt_handshake: 0,
          flow: null,
        }
      : {};

  return {
    rate: 1,
    ...tuicDefaults,
    ...shadowsocksDefaults,
    ...vmessDefaults,
    ...trojanDefaults,
    ...hysteriaDefaults,
    ...vlessDefaults,
    ...anyTlsDefaults,
    ...v2nodeDefaults,
    ...normalizedRecord,
  };
}

export function getLegacyNetworkSettingsPlaceholder(type: admin.ServerTypeName, network: unknown) {
  return LEGACY_NETWORK_SETTINGS_PLACEHOLDERS[type]?.[String(network)] || '';
}

export function getLegacyV2nodeSecurityValue(protocol: unknown, tls: unknown) {
  const parsedTls = parseInt(String(tls ?? 0), 10);
  if (parsedTls) return parsedTls;
  const protocolValue = protocol == null ? null : String(protocol);
  return protocolValue && LEGACY_V2NODE_SECURITY_FALLBACK_PROTOCOLS.includes(protocolValue) ? 1 : 0;
}

function getLegacyV2nodeSecurityOptions(protocol: unknown): LegacySelectOption[] {
  const protocolValue = protocol == null ? null : String(protocol);
  return [
    ...(protocolValue === 'vless' || protocolValue === 'vmess'
      ? [LEGACY_SECURITY_NONE_OPTION]
      : []),
    LEGACY_SECURITY_TLS_OPTION,
    ...(protocolValue === 'vless' || protocolValue === 'anytls'
      ? [LEGACY_SECURITY_REALITY_OPTION]
      : []),
  ];
}

function getLegacyV2nodeTransportOptions(protocol: unknown): LegacySelectOption[] {
  return protocol === 'trojan' ? LEGACY_TROJAN_NETWORK_OPTIONS : LEGACY_V2NODE_TRANSPORT_OPTIONS;
}

function getLegacyVlessFlowOptions(network: unknown): LegacySelectOption[] {
  return String(network) === 'tcp' ? LEGACY_VLESS_FLOW_OPTIONS : LEGACY_VLESS_FLOW_NONE_OPTIONS;
}

export function getLegacyNumericSelectValue(value: unknown, fallback = 0) {
  return parseInt(String(value ?? fallback), 10) || fallback;
}

export function getLegacyBinarySelectValue(value: unknown) {
  return getLegacyNumericSelectValue(value) ? 1 : 0;
}

function legacyNumericSelectValueProps(value: unknown, fallback = 0) {
  return { value: getLegacyNumericSelectValue(value, fallback) };
}

function legacyBinarySelectValueProps(value: unknown) {
  return { value: getLegacyBinarySelectValue(value) };
}

function normalizeLegacySettings(
  value: unknown,
  defaults: Record<string, unknown>,
): Record<string, unknown> {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    return { ...defaults, ...(value as Record<string, unknown>) };
  }
  if (typeof value === 'string' && value.trim()) {
    try {
      const parsed = JSON.parse(value) as unknown;
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return { ...defaults, ...(parsed as Record<string, unknown>) };
      }
    } catch {
      return { ...defaults };
    }
  }
  return { ...defaults };
}

function legacyText(value: unknown) {
  return value == null ? '' : String(value);
}

function legacySelectValue(value: unknown): LegacySelectValue | undefined {
  if (value == null) return undefined;
  return typeof value === 'number' || typeof value === 'string' ? value : String(value);
}

function legacyInputValue(value: unknown) {
  return value as string | number | readonly string[] | undefined;
}

function legacyBool(value: unknown) {
  return parseInt(String(value ?? 0), 10) !== 0;
}

function ServerChildDrawerField({
  type,
  field,
  form,
}: {
  type: admin.ServerTypeName;
  field: string;
  form: FormInstance;
}) {
  const network = Form.useWatch('network', form);
  const tls = Form.useWatch('tls', form);
  const encryption = Form.useWatch('encryption', form);
  const settings = Form.useWatch(field, form);

  if (field === 'network_settings' || field === 'networkSettings') {
    return (
      <div id="v2ray-protocol">
        <div className="form-group">
          <label>
            协议详细配置
            <a href="https://www.v2ray.com/chapter_02/05_transport.html">
              <LegacyLinkIcon />
              参考
            </a>
          </label>
          <Form.Item noStyle name={field}>
            <Input.TextArea
              rows={8}
              placeholder={getLegacyNetworkSettingsPlaceholder(type, network)}
            />
          </Form.Item>
        </div>
      </div>
    );
  }

  if (field === 'padding_scheme') {
    return (
      <div id="anytls-padding-scheme">
        <div className="form-group">
          <Form.Item noStyle name="padding_scheme">
            <Input.TextArea rows={8} placeholder={ANYTLS_PADDING_SCHEME_PLACEHOLDER} />
          </Form.Item>
        </div>
      </div>
    );
  }

  if (field === 'tls_settings' || field === 'tlsSettings') {
    return (
      <LegacyTlsSettingsField
        field={field}
        form={form}
        settings={settings}
        tls={tls}
        certApply={field === 'tls_settings'}
      />
    );
  }

  if (field === 'encryption_settings') {
    return (
      <LegacyEncryptionSettingsField form={form} settings={settings} encryption={encryption} />
    );
  }

  return (
    <Form.Item noStyle name={field}>
      <Input.TextArea rows={8} />
    </Form.Item>
  );
}

function LegacyTlsSettingsField({
  field,
  form,
  settings,
  tls,
  certApply,
}: {
  field: string;
  form: FormInstance;
  settings: unknown;
  tls: unknown;
  certApply: boolean;
}) {
  const value = normalizeLegacySettings(settings, LEGACY_TLS_SETTINGS_DEFAULTS);
  const tlsValue = parseInt(String(tls ?? 0), 10);
  const change = (key: string, next: unknown) => {
    form.setFieldsValue({ [field]: { ...value, [key]: next } });
  };

  return (
    <div>
      <div className="form-group">
        <label>Server Name(SNI)</label>
        <Input
          value={legacyText(value.server_name)}
          onChange={(event) => change('server_name', event.target.value)}
          placeholder={tlsValue === 2 ? 'REALITY必填，与后端保持一致' : ''}
        />
      </div>
      {tlsValue === 1 && certApply ? (
        <div className="form-group">
          <label>证书模式Cert Mode</label>
          <LegacySelect
            value={legacySelectValue(value.cert_mode ?? 'self')}
            style={{ width: '100%' }}
            options={LEGACY_TLS_CERT_MODE_OPTIONS}
            onChange={(next) => change('cert_mode', next)}
          />
        </div>
      ) : null}
      {value.cert_mode === 'dns' && certApply ? (
        <div className="form-group">
          <label>
            DNS解析提供商Provider{' '}
            <a
              target="_blank"
              href="https://go-acme.github.io/lego/dns/index.html"
              rel="noreferrer"
            >
              填写参考
            </a>
          </label>
          <Input
            value={legacyText(value.provider)}
            onChange={(event) => change('provider', event.target.value)}
            placeholder="书写格式cloudflare"
          />
        </div>
      ) : null}
      {value.cert_mode === 'dns' && certApply ? (
        <div className="form-group">
          <label>DNS env</label>
          <Input
            value={legacyText(value.dns_env)}
            onChange={(event) => change('dns_env', event.target.value)}
            placeholder="书写格式CF_DNS_API_TOKEN=xxxxxxx如有多条使用逗号,分隔"
          />
        </div>
      ) : null}
      {tlsValue === 1 && value.cert_mode !== 'none' && certApply ? (
        <div className="form-group">
          <label>证书公钥文件地址Cert File Path</label>
          <Input
            value={legacyText(value.cert_file)}
            onChange={(event) => change('cert_file', event.target.value)}
            placeholder="留空在/etc/v2node/目录自动生成"
          />
        </div>
      ) : null}
      {tlsValue === 1 && value.cert_mode !== 'none' && certApply ? (
        <div className="form-group">
          <label>证书私钥文件地址Key File Path</label>
          <Input
            value={legacyText(value.key_file)}
            onChange={(event) => change('key_file', event.target.value)}
            placeholder="留空在/etc/v2node/目录自动生成"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="form-group">
          <label>Server Address</label>
          <Input
            value={legacyText(value.dest)}
            onChange={(event) => change('dest', event.target.value)}
            placeholder="REALITY目标地址,默认使用SNI"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="form-group">
          <label>Server Port</label>
          <Input
            value={legacyText(value.server_port)}
            onChange={(event) => change('server_port', event.target.value)}
            placeholder="REALITY目标端口,默认443"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="form-group">
          <label>Proxy Protocol</label>
          <LegacySelect
            value={parseInt(String(value.xver ?? 0), 10) || 0}
            style={{ width: '100%' }}
            options={LEGACY_PROXY_PROTOCOL_OPTIONS}
            onChange={(next) => change('xver', next)}
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="form-group">
          <label>Private Key</label>
          <Input
            value={legacyText(value.private_key)}
            onChange={(event) => change('private_key', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="form-group">
          <label>Public Key</label>
          <Input
            value={legacyText(value.public_key)}
            onChange={(event) => change('public_key', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="form-group">
          <label>ShortId</label>
          <Input
            value={legacyText(value.short_id)}
            onChange={(event) => change('short_id', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
      <div className="form-group">
        <label>FingerPrint</label>
        <LegacySelect
          value={legacySelectValue(value.fingerprint)}
          style={{ width: '100%' }}
          options={LEGACY_TLS_FINGERPRINT_OPTIONS}
          onChange={(next) => change('fingerprint', next)}
          placeholder="TLS指纹默认Chrome"
        />
      </div>
      {tlsValue === 1 && certApply ? (
        <div className="form-group">
          <label>Reject unknown sni</label>
          <div>
            <LegacySwitch
              checked={legacyBool(value.reject_unknown_sni)}
              onChange={(checked) => change('reject_unknown_sni', checked ? '1' : '0')}
            />
          </div>
        </div>
      ) : null}
      <div className="form-group">
        <label>Allow Insecure</label>
        <div>
          <LegacySwitch
            checked={legacyBool(value.allow_insecure)}
            onChange={(checked) => change('allow_insecure', checked ? '1' : '0')}
          />
        </div>
      </div>
      <div className="form-group">
        <label>ECH (Encrypted Client Hello)</label>
        <LegacySelect
          value={legacyText(value.ech)}
          style={{ width: '100%' }}
          options={LEGACY_ECH_MODE_OPTIONS}
          onChange={(next) => change('ech', next)}
          placeholder="选择 ECH 模式"
        />
      </div>
      {value.ech === 'cloudflare' ? (
        <div
          className="form-group"
          style={{
            background: '#f6ffed',
            padding: '8px 12px',
            borderRadius: 4,
            border: '1px solid #b7eb8f',
          }}
        >
          <span style={{ color: '#52c41a' }}>
            ✓ Cloudflare 托管 ECH，密钥由 Cloudflare 自动管理，客户端从 DNS
            自动获取配置，服务端无需配置
          </span>
        </div>
      ) : null}
      {value.ech === 'custom' ? (
        <div className="form-group">
          <label>ECH Server Name (伪装域名/外层SNI)</label>
          <Input
            value={legacyText(value.ech_server_name)}
            onChange={(event) => change('ech_server_name', event.target.value)}
            placeholder="必填"
          />
        </div>
      ) : null}
      {value.ech === 'custom' ? (
        <div className="form-group">
          <label>ECH Key (服务端私钥)</label>
          <Input
            value={legacyText(value.ech_key)}
            onChange={(event) => change('ech_key', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
      {value.ech === 'custom' ? (
        <div className="form-group">
          <label>ECH Config (客户端配置)</label>
          <Input
            value={legacyText(value.ech_config)}
            onChange={(event) => change('ech_config', event.target.value)}
            placeholder="留空自动生成"
          />
        </div>
      ) : null}
    </div>
  );
}

function LegacyEncryptionSettingsField({
  form,
  settings,
}: {
  form: FormInstance;
  settings: unknown;
  encryption: unknown;
}) {
  const value = useMemo(
    () => normalizeLegacySettings(settings, LEGACY_ENCRYPTION_SETTINGS_DEFAULTS),
    [settings],
  );
  useEffect(() => {
    form.setFieldsValue({ encryption_settings: value });
  }, [form, value]);
  const change = (key: string, next: unknown) => {
    form.setFieldsValue({ encryption_settings: { ...value, [key]: next } });
  };

  return (
    <div>
      <div className="form-group">
        <label>Mode</label>
        <LegacySelect
          value={legacyText(value.mode) || 'native'}
          style={{ width: '100%' }}
          options={LEGACY_ENCRYPTION_MODE_OPTIONS}
          onChange={(next) => change('mode', next)}
        />
      </div>
      <div className="row">
        <div className="form-group col-md-6 col-xs-12">
          <label>RTT</label>
          <LegacySelect
            value={legacyText(value.rtt) || '0rtt'}
            style={{ width: '100%' }}
            options={LEGACY_ENCRYPTION_RTT_OPTIONS}
            onChange={(next) => change('rtt', next)}
          />
        </div>
        {value.rtt === '0rtt' ? (
          <div className="form-group col-md-6 col-xs-12">
            <label>Ticket time</label>
            <Input
              value={legacyText(value.ticket)}
              onChange={(event) => change('ticket', event.target.value)}
              placeholder="最长允许时间"
            />
          </div>
        ) : null}
      </div>
      <div className="form-group">
        <label>Server Padding</label>
        <Input
          value={legacyText(value.server_padding)}
          onChange={(event) => change('server_padding', event.target.value)}
          placeholder="留空使用默认值100-111-1111.75-0-111.50-0-3333"
        />
      </div>
      <div className="form-group">
        <label>Private Key</label>
        <Input
          value={legacyText(value.private_key)}
          onChange={(event) => change('private_key', event.target.value)}
          placeholder="留空自动生成，需抗量子加密请自行替换"
        />
      </div>
      <div className="form-group">
        <label>Client Padding</label>
        <Input
          value={legacyText(value.client_padding)}
          onChange={(event) => change('client_padding', event.target.value)}
          placeholder="留空使用默认值100-111-1111.75-0-111.50-0-3333"
        />
      </div>
      <div className="form-group">
        <label>Password</label>
        <Input
          value={legacyText(value.password)}
          onChange={(event) => change('password', event.target.value)}
          placeholder="留空自动生成，需抗量子加密请自行替换"
        />
      </div>
    </div>
  );
}

function TrojanAllowInsecureField() {
  return (
    <div className="form-group col-md-4 col-xs-12">
      <label>
        <LegacyTooltip placement="top" title="使用自签名证书需要允许不安全，用户才可以连接">
          允许不安全 <LegacyQuestionCircleIcon />
        </LegacyTooltip>
      </label>
      <Form.Item
        noStyle
        name="allow_insecure"
        initialValue={0}
        getValueProps={legacyBinarySelectValueProps}
      >
        <LegacySelect
          placeholder="允许不安全"
          style={{ width: '100%' }}
          options={LEGACY_BINARY_SELECT_OPTIONS}
        />
      </Form.Item>
    </div>
  );
}

function ServerInsecureField() {
  return (
    <div className="form-group col-md-4 col-xs-12">
      <label>
        <LegacyTooltip placement="top" title="使用自签名证书需要允许不安全，用户才可以连接">
          允许不安全 <LegacyQuestionCircleIcon />
        </LegacyTooltip>
      </label>
      <Form.Item
        noStyle
        name="insecure"
        initialValue={0}
        getValueProps={legacyBinarySelectValueProps}
      >
        <LegacySelect
          placeholder="允许不安全"
          style={{ width: '100%' }}
          options={LEGACY_BINARY_SELECT_OPTIONS}
        />
      </Form.Item>
    </div>
  );
}

function VmessTlsField({
  showChildDrawer,
}: {
  showChildDrawer: (title?: string, field?: string) => void;
}) {
  return (
    <div className="form-group col-md-4 col-xs-12">
      <label>
        TLS{' '}
        <a ref={legacyHref()} onClick={() => showChildDrawer('编辑TLS配置', 'tlsSettings')}>
          编辑配置
        </a>
      </label>
      <Form.Item noStyle name="tls" initialValue={0} getValueProps={legacyBinarySelectValueProps}>
        <LegacySelect
          placeholder="是否支持TLS"
          style={{ width: '100%' }}
          options={LEGACY_TLS_SUPPORT_OPTIONS}
        />
      </Form.Item>
    </div>
  );
}

function VlessSecurityField({
  form,
  showChildDrawer,
}: {
  form: FormInstance;
  showChildDrawer: (title?: string, field?: string) => void;
}) {
  const security = Form.useWatch('tls', form);

  return (
    <div className="form-group col-md-4 col-xs-12">
      <label>
        安全性{' '}
        {parseInt(String(security ?? 0), 10) !== 0 ? (
          <a ref={legacyHref()} onClick={() => showChildDrawer('编辑安全性配置', 'tls_settings')}>
            编辑配置
          </a>
        ) : null}
      </label>
      <Form.Item noStyle name="tls" initialValue={0} getValueProps={legacyNumericSelectValueProps}>
        <LegacySelect
          style={{ width: '100%' }}
          options={[
            LEGACY_SECURITY_NONE_OPTION,
            LEGACY_SECURITY_TLS_OPTION,
            LEGACY_SECURITY_REALITY_OPTION,
          ]}
        />
      </Form.Item>
    </div>
  );
}

function V2nodeFields({
  form,
  showChildDrawer,
}: {
  form: FormInstance;
  showChildDrawer: (title?: string, field?: string) => void;
}) {
  const protocol = Form.useWatch('protocol', form);
  const tls = Form.useWatch('tls', form);
  const obfs = Form.useWatch('obfs', form);
  const encryption = Form.useWatch('encryption', form);
  const protocolValue = protocol == null ? null : String(protocol);
  const securityValue = getLegacyV2nodeSecurityValue(protocolValue, tls);

  const changeProtocol = (value: LegacySelectValue) => {
    const nextProtocol = value == null ? '' : String(value);
    form.setFieldsValue({
      protocol: nextProtocol,
      ...(LEGACY_TLS_FORCED_PROTOCOLS.includes(nextProtocol) ? { tls: 1 } : {}),
    });
  };

  return (
    <>
      <div className="row">
        <div className="form-group col-md-6 col-xs-12">
          <label>节点协议</label>
          <Form.Item noStyle name="protocol">
            <LegacySelect
              style={{ width: '100%' }}
              options={LEGACY_V2NODE_PROTOCOL_OPTIONS}
              onChange={changeProtocol}
            />
          </Form.Item>
        </div>
        {protocolValue != null && protocolValue !== 'shadowsocks' ? (
          <div className="form-group col-md-6 col-xs-12">
            <label>
              安全性{' '}
              {securityValue ? (
                <a
                  ref={legacyHref()}
                  onClick={() => showChildDrawer('编辑安全性配置', 'tls_settings')}
                >
                  编辑配置
                </a>
              ) : null}
            </label>
            <Form.Item
              noStyle
              name="tls"
              initialValue={0}
              getValueProps={(value) => ({
                value: getLegacyV2nodeSecurityValue(protocolValue, value),
              })}
            >
              <LegacySelect
                style={{ width: '100%' }}
                options={getLegacyV2nodeSecurityOptions(protocolValue)}
              />
            </Form.Item>
          </div>
        ) : null}
      </div>
      {protocolValue === 'shadowsocks' ? (
        <div className="row">
          <div className="form-group col-md-12 col-xs-12">
            <label>
              传输协议{' '}
              <a
                ref={legacyHref()}
                onClick={() => showChildDrawer('编辑协议配置', 'network_settings')}
              >
                编辑配置
              </a>
            </label>
            <Form.Item noStyle name="network" initialValue="tcp">
              <LegacySelect
                placeholder="选择传输协议"
                style={{ width: '100%' }}
                options={LEGACY_V2NODE_SHADOWSOCKS_NETWORK_OPTIONS}
              />
            </Form.Item>
          </div>
        </div>
      ) : null}
      {protocolValue != null &&
      protocolValue !== 'hysteria2' &&
      protocolValue !== 'shadowsocks' &&
      protocolValue !== 'tuic' ? (
        <div className="row">
          <div className="form-group col-md-12 col-xs-12">
            <label>
              传输协议{' '}
              <a
                ref={legacyHref()}
                onClick={() => showChildDrawer('编辑协议配置', 'network_settings')}
              >
                编辑配置
              </a>
            </label>
            <Form.Item noStyle name="network" initialValue="tcp">
              <LegacySelect
                placeholder="选择传输协议"
                style={{ width: '100%' }}
                options={getLegacyV2nodeTransportOptions(protocolValue)}
              />
            </Form.Item>
          </div>
        </div>
      ) : null}
      {protocolValue === 'anytls' ? (
        <div className="row">
          <div className="form-group col-md-12 col-xs-12">
            <label>
              <a
                ref={legacyHref()}
                onClick={() => showChildDrawer('编辑填充方案', 'padding_scheme')}
              >
                编辑填充方案
              </a>
            </label>
          </div>
        </div>
      ) : null}
      {protocolValue === 'hysteria2' ? (
        <>
          <div className="row">
            <div className="form-group col-md-6 col-xs-12">
              <label>混淆方式obfs</label>
              <Form.Item noStyle name="obfs">
                <LegacySelect style={{ width: '100%' }} options={LEGACY_HYSTERIA2_OBFS_OPTIONS} />
              </Form.Item>
            </div>
            {obfs === 'salamander' ? (
              <div className="form-group col-md-6 col-xs-12">
                <label>混淆密码obfs_password</label>
                <Form.Item noStyle name="obfs_password">
                  <Input placeholder="留空自动生成" />
                </Form.Item>
              </div>
            ) : null}
          </div>
          <div className="form-group">
            <label>上行带宽</label>
            <Form.Item noStyle name="up_mbps">
              <Input addonAfter="Mbps" placeholder="服务端发送带宽,留空或填0使用BBR" />
            </Form.Item>
          </div>
          <div className="form-group">
            <label>下行带宽</label>
            <Form.Item noStyle name="down_mbps">
              <Input addonAfter="Mbps" placeholder="服务端接收带宽,留空或填0使用BBR" />
            </Form.Item>
          </div>
        </>
      ) : null}
      {protocolValue === 'tuic' ? (
        <>
          <div className="row">
            <div className="form-group col-md-6 col-xs-12">
              <label>禁用SNI</label>
              <Form.Item
                noStyle
                name="disable_sni"
                initialValue={0}
                getValueProps={legacyBinarySelectValueProps}
              >
                <LegacySelect style={{ width: '100%' }} options={LEGACY_BINARY_SELECT_OPTIONS} />
              </Form.Item>
            </div>
            <div className="form-group col-md-6 col-xs-12">
              <label>数据包中继模式</label>
              <Form.Item noStyle name="udp_relay_mode" initialValue="native">
                <LegacySelect style={{ width: '100%' }} options={LEGACY_TUIC_RELAY_MODE_OPTIONS} />
              </Form.Item>
            </div>
          </div>
          <div className="row">
            <div className="form-group col-md-6 col-xs-12">
              <label>拥塞控制算法</label>
              <Form.Item noStyle name="congestion_control" initialValue="cubic">
                <LegacySelect
                  style={{ width: '100%' }}
                  options={LEGACY_TUIC_CONGESTION_CONTROL_OPTIONS}
                />
              </Form.Item>
            </div>
            <div className="form-group col-md-6 col-xs-12">
              <label>客户端启用 0-RTT</label>
              <Form.Item
                noStyle
                name="zero_rtt_handshake"
                initialValue={0}
                getValueProps={legacyBinarySelectValueProps}
              >
                <LegacySelect style={{ width: '100%' }} options={LEGACY_BINARY_SELECT_OPTIONS} />
              </Form.Item>
            </div>
          </div>
        </>
      ) : null}
      {protocolValue === 'shadowsocks' ? (
        <div className="form-group">
          <label>加密算法</label>
          <Form.Item noStyle name="cipher" initialValue="aes-128-gcm">
            <LegacySelect style={{ width: '100%' }} options={LEGACY_SHADOWSOCKS_CIPHER_OPTIONS} />
          </Form.Item>
        </div>
      ) : null}
      {protocolValue === 'vless' ? (
        <>
          <div className="row">
            <div className="form-group col-md-12 col-xs-12">
              <label>
                加密方式{' '}
                {encryption ? (
                  <a
                    ref={legacyHref()}
                    onClick={() => showChildDrawer('编辑加密配置', 'encryption_settings')}
                  >
                    编辑配置
                  </a>
                ) : null}
              </label>
              <Form.Item noStyle name="encryption">
                <LegacySelect
                  placeholder="选择加密方式"
                  style={{ width: '100%' }}
                  options={LEGACY_VLESS_ENCRYPTION_OPTIONS}
                />
              </Form.Item>
            </div>
          </div>
          <div className="row">
            <div className="form-group col-md-12 col-xs-12">
              <label>XTLS流控算法</label>
              <Form.Item noStyle name="flow">
                <LegacySelect
                  placeholder="选择XTLS流控算法"
                  style={{ width: '100%' }}
                  options={LEGACY_VLESS_FLOW_OPTIONS}
                />
              </Form.Item>
            </div>
          </div>
        </>
      ) : null}
    </>
  );
}

function ServerTypeFields({
  type,
  form,
  showChildDrawer,
}: {
  type: admin.ServerTypeName;
  form: FormInstance;
  showChildDrawer: (title?: string, field?: string) => void;
}) {
  const shadowsocksObfs = Form.useWatch('obfs', form);
  const vlessNetwork = Form.useWatch('network', form);
  const vlessEncryption = Form.useWatch('encryption', form);
  const hysteriaVersion = Form.useWatch('version', form);
  const hysteriaObfs = Form.useWatch('obfs', form);
  const tuicDisableSni = Form.useWatch('disable_sni', form);

  if (type === 'v2node') {
    return <V2nodeFields form={form} showChildDrawer={showChildDrawer} />;
  }

  if (type === 'shadowsocks') {
    return (
      <>
        <div className="form-group">
          <label>加密算法</label>
          <Form.Item noStyle name="cipher" initialValue="chacha20-ietf-poly1305">
            <LegacySelect style={{ width: '100%' }} options={LEGACY_SHADOWSOCKS_CIPHER_OPTIONS} />
          </Form.Item>
        </div>
        <div className="form-group">
          <label>混淆</label>
          <Form.Item noStyle name="obfs" initialValue="">
            <LegacySelect style={{ width: '100%' }} options={LEGACY_SHADOWSOCKS_OBFS_OPTIONS} />
          </Form.Item>
          <div>
            {shadowsocksObfs === 'http' ? (
              <div className="row mt-2">
                <div className="form-group col-4 mb-0">
                  <Form.Item noStyle name={['obfs_settings', 'path']}>
                    <Input placeholder="路径" />
                  </Form.Item>
                </div>
                <div className="form-group col-8 mb-0">
                  <Form.Item noStyle name={['obfs_settings', 'host']}>
                    <Input placeholder="Host" />
                  </Form.Item>
                </div>
              </div>
            ) : null}
          </div>
        </div>
      </>
    );
  }
  if (type === 'vmess') {
    return (
      <div className="row">
        <div className="form-group col-md-12 col-xs-12">
          <label>
            传输协议{' '}
            <a
              ref={legacyHref()}
              onClick={() => showChildDrawer('编辑协议配置', 'networkSettings')}
            >
              编辑配置
            </a>
          </label>
          <Form.Item noStyle name="network">
            <LegacySelect
              placeholder="选择传输协议"
              style={{ width: '100%' }}
              options={LEGACY_STREAM_NETWORK_OPTIONS}
            />
          </Form.Item>
        </div>
      </div>
    );
  }
  if (type === 'trojan') {
    return (
      <>
        <div className="form-group">
          <label>服务器名称指示(sni)</label>
          <Form.Item noStyle name="server_name">
            <Input placeholder="当节点地址与证书不一致时用于证书验证" />
          </Form.Item>
        </div>
        <div className="row">
          <div className="form-group col-md-12 col-xs-12">
            <label>
              传输协议{' '}
              <a
                ref={legacyHref()}
                onClick={() => showChildDrawer('编辑协议配置', 'network_settings')}
              >
                编辑配置
              </a>
            </label>
            <Form.Item noStyle name="network">
              <LegacySelect
                placeholder="选择传输协议"
                style={{ width: '100%' }}
                options={LEGACY_TROJAN_NETWORK_OPTIONS}
              />
            </Form.Item>
          </div>
        </div>
      </>
    );
  }
  if (type === 'tuic') {
    return (
      <>
        <div className="row">
          <div className="form-group col-md-6 col-xs-12">
            <label>禁用SNI</label>
            <Form.Item
              noStyle
              name="disable_sni"
              initialValue={0}
              getValueProps={legacyBinarySelectValueProps}
            >
              <LegacySelect style={{ width: '100%' }} options={LEGACY_BINARY_SELECT_OPTIONS} />
            </Form.Item>
          </div>
          <div className="form-group col-md-6 col-xs-12">
            <label>数据包中继模式</label>
            <Form.Item noStyle name="udp_relay_mode" initialValue="native">
              <LegacySelect style={{ width: '100%' }} options={LEGACY_TUIC_RELAY_MODE_OPTIONS} />
            </Form.Item>
          </div>
        </div>
        {parseInt(String(tuicDisableSni ?? 0), 10) ? null : (
          <div className="form-group">
            <label>服务器名称指示(sni)</label>
            <Form.Item noStyle name="server_name">
              <Input placeholder="当节点地址与证书不一致时用于证书验证" />
            </Form.Item>
          </div>
        )}
        <div className="row">
          <div className="form-group col-md-6 col-xs-12">
            <label>拥塞控制算法</label>
            <Form.Item noStyle name="congestion_control" initialValue="cubic">
              <LegacySelect
                style={{ width: '100%' }}
                options={LEGACY_TUIC_CONGESTION_CONTROL_OPTIONS}
              />
            </Form.Item>
          </div>
          <div className="form-group col-md-6 col-xs-12">
            <label>客户端启用 0-RTT</label>
            <Form.Item
              noStyle
              name="zero_rtt_handshake"
              initialValue={0}
              getValueProps={legacyBinarySelectValueProps}
            >
              <LegacySelect style={{ width: '100%' }} options={LEGACY_BINARY_SELECT_OPTIONS} />
            </Form.Item>
          </div>
        </div>
      </>
    );
  }
  if (type === 'vless') {
    return (
      <>
        <div className="row">
          <div className="form-group col-md-12 col-xs-12">
            <label>
              传输协议{' '}
              <a
                ref={legacyHref()}
                onClick={() => showChildDrawer('编辑协议配置', 'network_settings')}
              >
                编辑配置
              </a>
            </label>
            <Form.Item noStyle name="network">
              <LegacySelect
                placeholder="选择传输协议"
                style={{ width: '100%' }}
                options={LEGACY_STREAM_NETWORK_OPTIONS}
              />
            </Form.Item>
          </div>
        </div>
        <div className="row">
          <div className="form-group col-md-12 col-xs-12">
            <label>
              加密方式{' '}
              {vlessEncryption ? (
                <a
                  ref={legacyHref()}
                  onClick={() => showChildDrawer('编辑加密配置', 'encryption_settings')}
                >
                  编辑配置
                </a>
              ) : null}
            </label>
            <Form.Item noStyle name="encryption">
              <LegacySelect
                placeholder="选择加密方式"
                style={{ width: '100%' }}
                options={LEGACY_VLESS_ENCRYPTION_OPTIONS}
              />
            </Form.Item>
          </div>
        </div>
        <div className="row">
          <div className="form-group col-md-12 col-xs-12">
            <label>XTLS流控算法</label>
            <Form.Item noStyle name="flow">
              <LegacySelect
                placeholder="选择XTLS流控算法"
                style={{ width: '100%' }}
                options={getLegacyVlessFlowOptions(vlessNetwork)}
              />
            </Form.Item>
          </div>
        </div>
      </>
    );
  }
  if (type === 'hysteria') {
    const version = parseInt(String(hysteriaVersion ?? 1), 10);
    const obfs = hysteriaObfs == null ? null : String(hysteriaObfs);

    return (
      <>
        <div className="row">
          <div className="form-group col-md-3 col-xs-12">
            <label>HYSTERIA版本</label>
            <Form.Item
              noStyle
              name="version"
              initialValue={1}
              getValueProps={(value) => legacyNumericSelectValueProps(value, 1)}
            >
              <LegacySelect style={{ width: '100%' }} options={LEGACY_HYSTERIA_VERSION_OPTIONS} />
            </Form.Item>
          </div>
        </div>
        <div className="form-group">
          <label>服务器名称指示(sni)</label>
          <Form.Item noStyle name="server_name">
            <Input placeholder="当节点地址与证书不一致时用于证书验证" />
          </Form.Item>
        </div>
        <div className="row">
          {version === 1 ? (
            <div className="form-group col-md-6 col-xs-12">
              <label>混淆方式obfs</label>
              <Form.Item noStyle name="obfs">
                <LegacySelect style={{ width: '100%' }} options={LEGACY_HYSTERIA_V1_OBFS_OPTIONS} />
              </Form.Item>
            </div>
          ) : null}
          {version === 1 && obfs === 'xplus' ? (
            <div className="form-group col-md-6 col-xs-12">
              <label>混淆密码obfsParam</label>
              <Form.Item noStyle name="obfs_password">
                <Input placeholder="留空自动生成" />
              </Form.Item>
            </div>
          ) : null}
          {version === 2 ? (
            <div className="form-group col-md-6 col-xs-12">
              <label>混淆方式obfs</label>
              <Form.Item noStyle name="obfs">
                <LegacySelect style={{ width: '100%' }} options={LEGACY_HYSTERIA2_OBFS_OPTIONS} />
              </Form.Item>
            </div>
          ) : null}
          {version === 2 && obfs === 'salamander' ? (
            <div className="form-group col-md-6 col-xs-12">
              <label>混淆密码obfs_password</label>
              <Form.Item noStyle name="obfs_password">
                <Input placeholder="留空自动生成" />
              </Form.Item>
            </div>
          ) : null}
        </div>
        <div className="form-group">
          <label>上行带宽</label>
          <Form.Item noStyle name="up_mbps">
            <Input addonAfter="Mbps" placeholder="服务端发送带宽,留空或填0使用BBR" />
          </Form.Item>
        </div>
        <div className="form-group">
          <label>下行带宽</label>
          <Form.Item noStyle name="down_mbps">
            <Input addonAfter="Mbps" placeholder="服务端接收带宽,留空或填0使用BBR" />
          </Form.Item>
        </div>
      </>
    );
  }
  if (type === 'anytls') {
    return (
      <>
        <div className="form-group">
          <label>服务器名称指示(sni)</label>
          <Form.Item noStyle name="server_name">
            <Input placeholder="当节点地址与证书不一致时用于证书验证" />
          </Form.Item>
        </div>
        <div className="row">
          <div className="form-group col-md-12 col-xs-12">
            <label>
              <a
                ref={legacyHref()}
                onClick={() => showChildDrawer('编辑填充方案', 'padding_scheme')}
              >
                编辑填充方案
              </a>
            </label>
          </div>
        </div>
      </>
    );
  }
  return null;
}
