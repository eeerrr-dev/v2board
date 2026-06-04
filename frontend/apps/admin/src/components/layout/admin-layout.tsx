import { useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { user } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { logout } from '@/lib/auth';
import { isDarkModeEnabled, setDarkMode } from '@/lib/dark-mode';
import { legacyHref } from '@/lib/legacy-href';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';

interface LegacyNavItem {
  title: string;
  type: 'heading' | 'item';
  href?: string;
  icon?: string;
}

interface LegacyLayoutSearch {
  placeholder?: string;
  defaultValue?: string;
  onChange: (value: string) => void;
}

interface AdminLayoutProps {
  loading?: boolean;
  search?: LegacyLayoutSearch;
  title?: string;
}

const LEGACY_NAV: LegacyNavItem[] = [
  { title: '仪表盘', type: 'item', href: '/dashboard', icon: 'si si-speedometer' },
  { title: '设置', type: 'heading' },
  { title: '系统配置', type: 'item', href: '/config/system', icon: 'si si-equalizer' },
  { title: '支付配置', type: 'item', href: '/config/payment', icon: 'si si-credit-card' },
  { title: '主题配置', type: 'item', href: '/config/theme', icon: 'si si-magic-wand' },
  { title: '服务器', type: 'heading' },
  { title: '节点管理', type: 'item', href: '/server/manage', icon: 'si si-layers' },
  { title: '权限组管理', type: 'item', href: '/server/group', icon: 'si si-wrench' },
  { title: '路由管理', type: 'item', href: '/server/route', icon: 'si si-shuffle' },
  { title: '财务', type: 'heading' },
  { title: '订阅管理', type: 'item', href: '/plan', icon: 'si si-bag' },
  { title: '订单管理', type: 'item', href: '/order', icon: 'si si-list' },
  { title: '优惠券管理', type: 'item', href: '/coupon', icon: 'si si-present' },
  { title: '礼品卡管理', type: 'item', href: '/giftcard', icon: 'si si-star' },
  { title: '用户', type: 'heading' },
  { title: '用户管理', type: 'item', href: '/user', icon: 'si si-users' },
  { title: '公告管理', type: 'item', href: '/notice', icon: 'si si-speech' },
  { title: '工单管理', type: 'item', href: '/ticket', icon: 'si si-support' },
  { title: '知识库管理', type: 'item', href: '/knowledge', icon: 'si si-bulb' },
  { title: '指标', type: 'heading' },
  { title: '队列监控', type: 'item', href: '/queue', icon: 'si si-bar-chart' },
];

const ROUTE_TITLES: Record<string, string> = {
  '/dashboard': '仪表盘',
  '/config/system': '系统配置',
  '/config/payment': '支付配置',
  '/config/theme': '主题配置',
  '/server/manage': '节点管理',
  '/server/group': '权限组管理',
  '/server/route': '路由管理',
  '/plan': '订阅管理',
  '/order': '订单管理',
  '/coupon': '优惠券管理',
  '/giftcard': '礼品卡管理',
  '/user': '用户管理',
  '/notice': '公告管理',
  '/ticket': '工单管理',
  '/knowledge': '知识库管理',
  '/queue': '队列监控',
};

function getSettings() {
  return window.settings ?? {};
}

function getTheme() {
  return getSettings().theme ?? { sidebar: 'light', header: 'dark', color: 'default' };
}

function getSiteTitle() {
  return getSettings().title || 'V2Board';
}

export function AdminLayout({ loading, search, title: titleProp }: AdminLayoutProps = {}) {
  const navigate = useNavigate();
  const location = useLocation();
  const [showNav, setShowNav] = useState(false);
  const [showAvatarMenu, setShowAvatarMenu] = useState(false);
  const [showSearchBar, setShowSearchBar] = useState(false);
  const [email, setEmail] = useState('');
  const [darkMode, setDarkModeState] = useState(() => isDarkModeEnabled());
  const theme = getTheme();
  const title = titleProp ?? ROUTE_TITLES[location.pathname] ?? '';
  const pageClassName =
    `sidebar-o ${theme.sidebar === 'dark' ? 'sidebar-dark' : ''} ` +
    `${theme.header === 'dark' ? 'page-header-dark' : ''} ` +
    `side-scroll page-header-fixed main-content-boxed side-trans-enabled ${showNav && 'sidebar-o-xs'}`;

  useEffect(() => {
    user.info(apiClient)
      .then((info) => setEmail(info.email ?? ''))
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    window.scrollTo(0, 0);
  }, [location.pathname, location.search]);

  useEffect(() => {
    if (!showAvatarMenu) return;
    const close = () => setShowAvatarMenu(false);
    document.addEventListener('click', close);
    return () => document.removeEventListener('click', close);
  }, [showAvatarMenu]);

  const navItems = useMemo(() => LEGACY_NAV, []);

  const closeMobileNav = () => setShowNav(false);

  const toggleDarkMode = () => {
    const next = !darkMode;
    setDarkModeState(next);
    setDarkMode(next);
  };

  const handleLogout = () => {
    logout();
    navigate('/login');
  };

  const handleNavClick = (href: string | undefined) => {
    if (href) navigate(href);
    closeMobileNav();
  };

  return (
    <div id="page-container" className={pageClassName}>
      <div
        onClick={() => setShowNav((value) => !value)}
        className="v2board-nav-mask"
        style={{ display: showNav ? 'block' : 'none' }}
      />

      <nav id="sidebar">
        <div className="smini-hidden bg-header-dark">
          <div className="content-header justify-content-lg-center bg-black-10">
            <a className="link-fx font-size-lg text-white" href="/">
              <span className="text-white-75">{getSiteTitle()}</span>
            </a>
            <div className="d-lg-none">
              <a
                className="text-white ml-2"
                data-toggle="layout"
                data-action="sidebar_close"
                ref={legacyHref()}
                onClick={() => setShowNav((value) => !value)}
              >
                <i className="fa fa-times-circle" />
              </a>
            </div>
          </div>
        </div>

        <div className="content-side content-side-full">
          <ul className="nav-main">
            {navItems.map((item, index) =>
              item.type === 'heading' ? (
                <li key={Math.random()} className="nav-main-heading">
                  {item.title}
                </li>
              ) : (
                <li key={Math.random()} className="nav-main-item">
                  <a
                    className={`nav-main-link ${location.pathname === item.href && 'active'}`}
                    onClick={() => handleNavClick(item.href)}
                  >
                    {item.icon ? <i className={`nav-main-link-icon ${item.icon}`} /> : null}
                    <span className="nav-main-link-name">{item.title}</span>
                  </a>
                </li>
              ),
            )}
          </ul>
        </div>

        <div className="v2board-copyright">{getSiteTitle()} v1.7.5</div>
      </nav>

      <header id="page-header">
        <div className="content-header" style={{ maxWidth: 'unset' }}>
          <div className="sidebar-toggle" style={{ display: search ? 'block' : 'none' }}>
            <button
              type="button"
              className={theme.header === 'dark' ? 'btn btn-primary mr-1 d-lg-none' : 'btn mr-1 d-lg-none'}
              onClick={() => setShowNav((value) => !value)}
            >
              <i className="fa fa-fw fa-bars" />
            </button>
            {search && (
              <button
                type="button"
                className={theme.header === 'dark' ? 'btn btn-primary' : 'btn'}
                onClick={() => setShowSearchBar(true)}
              >
                <i className="fa fa-fw fa-search" />{' '}
                <span className="ml-1 d-none d-sm-inline-block">搜索</span>
              </button>
            )}
          </div>
          <div className={theme.header === 'dark' ? 'v2board-container-title text-white' : 'v2board-container-title text-black'}>
            {title}
          </div>
          <div>
            <div className="dropdown d-inline-block">
              <button
                type="button"
                className={theme.header === 'dark' ? 'btn btn-primary mr-1' : 'btn mr-1'}
                onClick={toggleDarkMode}
              >
                <i className={darkMode ? 'far fa fa-moon' : 'far fa fa-sun'} />
              </button>
            </div>
            <div className="dropdown d-inline-block">
              <button
                type="button"
                className={theme.header === 'dark' ? 'btn btn-primary' : 'btn'}
                id="page-header-user-dropdown"
                data-toggle="dropdown"
                aria-haspopup="true"
                aria-expanded="false"
                onClick={() => setShowAvatarMenu((value) => !value)}
              >
                <i className="far fa fa-user-circle" />
                <span className="d-none d-lg-inline ml-1">{email}</span>
                <i className="fa fa-fw fa-angle-down ml-1" />
              </button>
              <div
                className={`dropdown-menu dropdown-menu-right dropdown-menu-lg p-0 ${showAvatarMenu && 'show'}`}
                aria-labelledby="page-header-user-dropdown"
              >
                <div className="p-2">
                  <a
                    className="dropdown-item d-flex justify-content-between align-items-center"
                    ref={legacyHref()}
                    onClick={handleLogout}
                  >
                    登出
                    <i className="fa fa-fw fa-sign-out-alt text-danger ml-1" />
                  </a>
                </div>
              </div>
            </div>
          </div>
          {search && (
            <div className={`overlay-header bg-dark ${showSearchBar ? 'show' : ''}`}>
              <div className="content-header bg-dark">
                <div className="w-100">
                  <div className="input-group">
                    <div className="input-group-prepend">
                      <button
                        type="button"
                        className="btn btn-dark"
                        onClick={() => setShowSearchBar(false)}
                      >
                        <i className="fa fa-fw fa-times-circle" />
                      </button>
                    </div>
                    <input
                      type="text"
                      className="form-control border-0"
                      placeholder={search.placeholder}
                      onChange={(event) => search.onChange(event.target.value)}
                      defaultValue={search.defaultValue}
                    />
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </header>

      {loading ? (
        <main id="main-container">
          <div className="content content-full text-center pt-5">
            <div className="spinner-grow text-primary" role="status">
              <span className="sr-only">Loading...</span>
            </div>
          </div>
        </main>
      ) : (
        <main id="main-container">
          <div className="p-0 p-lg-4">
            <RouteBoundaryOutlet />
          </div>
        </main>
      )}
    </div>
  );
}
