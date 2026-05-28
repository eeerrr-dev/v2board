import { useMemo, useState } from 'react';
import { Layout, Menu, Avatar, Dropdown, theme } from 'antd';
import type { MenuProps } from 'antd';
import {
  DashboardOutlined,
  UserOutlined,
  ShoppingCartOutlined,
  AppstoreOutlined,
  CloudServerOutlined,
  CustomerServiceOutlined,
  CreditCardOutlined,
  GiftOutlined,
  BookOutlined,
  NotificationOutlined,
  MonitorOutlined,
  SettingOutlined,
  BarChartOutlined,
  LogoutOutlined,
  TranslationOutlined,
} from '@ant-design/icons';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { SUPPORTED_LOCALES, type SupportedLocale } from '@v2board/i18n';
import { logout } from '@/lib/auth';

const { Sider, Header, Content } = Layout;

// The OneUI admin uses a blue top bar (#0665D0) over a white, grouped sidebar.
const BRAND_BLUE = '#0665D0';

export function AdminLayout() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
  const { token } = theme.useToken();
  const version = (window as unknown as { settings?: { version?: string } }).settings?.version;

  // Menu grouped to mirror the original admin's information architecture
  // (设置 / 服务器 / 财务 / 用户), keeping the new admin's extra pages
  // (统计报表, 系统状态) in the closest matching place.
  const items = useMemo<MenuProps['items']>(
    () => [
      { key: '/dashboard', icon: <DashboardOutlined />, label: t('admin.nav.dashboard') },
      { key: '/stats', icon: <BarChartOutlined />, label: t('admin.nav.stat') },
      {
        type: 'group',
        label: t('admin.nav.group_setting'),
        children: [
          { key: '/config', icon: <SettingOutlined />, label: t('admin.nav.config') },
          { key: '/payments', icon: <CreditCardOutlined />, label: t('admin.nav.payments') },
          { key: '/system', icon: <MonitorOutlined />, label: t('admin.nav.system') },
        ],
      },
      {
        type: 'group',
        label: t('admin.nav.group_server'),
        children: [{ key: '/servers', icon: <CloudServerOutlined />, label: t('admin.nav.servers') }],
      },
      {
        type: 'group',
        label: t('admin.nav.group_finance'),
        children: [
          { key: '/plans', icon: <AppstoreOutlined />, label: t('admin.nav.plans') },
          { key: '/orders', icon: <ShoppingCartOutlined />, label: t('admin.nav.orders') },
          { key: '/coupons', icon: <GiftOutlined />, label: t('admin.nav.coupons') },
        ],
      },
      {
        type: 'group',
        label: t('admin.nav.group_user'),
        children: [
          { key: '/users', icon: <UserOutlined />, label: t('admin.nav.users') },
          { key: '/notices', icon: <NotificationOutlined />, label: t('admin.nav.notices') },
          { key: '/tickets', icon: <CustomerServiceOutlined />, label: t('admin.nav.tickets') },
          { key: '/knowledge', icon: <BookOutlined />, label: t('admin.nav.knowledge') },
        ],
      },
    ],
    [t],
  );

  const pageTitle = useMemo(() => {
    const flat = new Map<string, string>();
    for (const item of items ?? []) {
      if (!item) continue;
      if ('children' in item && item.children) {
        for (const child of item.children) {
          if (child && 'key' in child && child.key) flat.set(String(child.key), String((child as { label?: unknown }).label ?? ''));
        }
      } else if ('key' in item && item.key) {
        flat.set(String(item.key), String((item as { label?: unknown }).label ?? ''));
      }
    }
    return flat.get(location.pathname) ?? '';
  }, [items, location.pathname]);

  return (
    <Layout className="min-h-screen" style={{ minHeight: '100vh' }}>
      <Sider
        collapsible
        collapsed={collapsed}
        onCollapse={setCollapsed}
        breakpoint="lg"
        trigger={null}
        theme="light"
        style={{ display: 'flex', flexDirection: 'column', borderRight: `1px solid ${token.colorBorderSecondary}` }}
      >
        <div
          style={{
            height: 64,
            display: 'flex',
            alignItems: 'center',
            justifyContent: collapsed ? 'center' : 'flex-start',
            paddingInline: 24,
            fontWeight: 600,
            color: '#fff',
            fontSize: 18,
            letterSpacing: 0.5,
            background: BRAND_BLUE,
          }}
        >
          {collapsed ? 'V2' : 'V2Board'}
        </div>
        <div style={{ flex: 1, overflowY: 'auto' }}>
          <Menu
            mode="inline"
            selectedKeys={[location.pathname]}
            items={items}
            onClick={(e) => navigate(e.key)}
            style={{ borderInlineEnd: 'none' }}
          />
        </div>
        {!collapsed && (
          <div style={{ padding: '12px 24px', color: token.colorTextQuaternary, fontSize: 12 }}>
            V2Board{version ? ` v${version}` : ''}
          </div>
        )}
      </Sider>
      <Layout>
        <Header
          style={{
            background: BRAND_BLUE,
            padding: '0 24px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 16,
            height: 64,
            lineHeight: '64px',
          }}
        >
          <span style={{ color: '#fff', fontSize: 16, fontWeight: 500 }}>{pageTitle}</span>
          <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
            <Dropdown
              menu={{
                items: SUPPORTED_LOCALES.map((l) => ({
                  key: l.code,
                  label: l.label,
                })),
                selectedKeys: [i18n.resolvedLanguage ?? 'en-US'],
                onClick: (e) => i18n.changeLanguage(e.key as SupportedLocale),
              }}
              trigger={['click']}
            >
              <Avatar
                size="small"
                icon={<TranslationOutlined />}
                style={{ cursor: 'pointer', background: 'rgba(255,255,255,0.2)' }}
              />
            </Dropdown>
            <Dropdown
              menu={{
                items: [
                  {
                    key: 'logout',
                    icon: <LogoutOutlined />,
                    label: t('common.logout'),
                  },
                ],
                onClick: ({ key }) => {
                  if (key === 'logout') {
                    logout();
                    navigate('/login', { replace: true });
                  }
                },
              }}
              trigger={['click']}
            >
              <Avatar
                size="small"
                icon={<UserOutlined />}
                style={{ cursor: 'pointer', background: 'rgba(255,255,255,0.2)' }}
              />
            </Dropdown>
          </div>
        </Header>
        <Content style={{ padding: 24, background: token.colorBgLayout }}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}
