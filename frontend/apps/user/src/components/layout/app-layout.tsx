import { Fragment, useEffect, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { LanguageMenu } from './language-menu';
import { useUserInfo } from '@/lib/queries';
import { logout } from '@/lib/auth';
import { cn } from '@/lib/cn';
import { isDarkModeEnabled, setDarkMode } from '@/lib/dark-mode';
import { getLegacyTheme, getLegacyTitle } from '@/lib/legacy-settings';
import { legacyHref } from '@/lib/legacy-href';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';

interface NavItem {
  to: string;
  labelKey: string;
  iconClass: string;
}

interface NavGroup {
  labelKey?: string;
  items: NavItem[];
}

const NAV: NavGroup[] = [
  {
    items: [
      { to: '/dashboard', labelKey: 'nav.dashboard', iconClass: 'si si-speedometer' },
      { to: '/knowledge', labelKey: 'nav.knowledge', iconClass: 'si si-book-open' },
    ],
  },
  {
    labelKey: 'nav.group_subscribe',
    items: [
      { to: '/plan', labelKey: 'nav.buy_subscribe', iconClass: 'si si-bag' },
      { to: '/node', labelKey: 'nav.node', iconClass: 'si si-check' },
    ],
  },
  {
    labelKey: 'nav.group_finance',
    items: [
      { to: '/order', labelKey: 'nav.orders', iconClass: 'si si-list' },
      { to: '/invite', labelKey: 'nav.invite', iconClass: 'si si-users' },
    ],
  },
  {
    labelKey: 'nav.group_user',
    items: [
      { to: '/profile', labelKey: 'nav.profile', iconClass: 'si si-user' },
      { to: '/ticket', labelKey: 'nav.tickets', iconClass: 'si si-support' },
      { to: '/traffic', labelKey: 'nav.traffic', iconClass: 'si si-bar-chart' },
    ],
  },
];

const DETAIL_LABELS: { match: RegExp; labelKey: string }[] = [
  { match: /^\/order\/[^/]+$/, labelKey: 'order.detail' },
  { match: /^\/plan\/[^/]+$/, labelKey: 'plan.checkout_title' },
];

function findActiveLabel(pathname: string): string | undefined {
  for (const d of DETAIL_LABELS) {
    if (d.match.test(pathname)) return d.labelKey;
  }
  for (const group of NAV) {
    for (const item of group.items) {
      if (pathname === item.to || pathname.startsWith(item.to + '/')) {
        return item.labelKey;
      }
    }
  }
  return undefined;
}

export function AppLayout() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const { data: user } = useUserInfo({ refetchOnMount: false });
  const [open, setOpen] = useState(false);
  const [showAvatarMenu, setShowAvatarMenu] = useState(false);
  const [darkMode, setDarkModeState] = useState(() => isDarkModeEnabled());
  const activeLabel = findActiveLabel(location.pathname);
  const legacyTheme = getLegacyTheme();
  const title = getLegacyTitle();
  const darkSidebar = legacyTheme.sidebar === 'dark';
  const darkHeader = legacyTheme.header === 'dark';
  const headerButtonClass = darkHeader ? 'btn btn-primary mr-1' : 'btn mr-1';
  const localeClass = String(window.localStorage.getItem('umi_locale'));

  const go = (to: string) => {
    navigate(to);
    setOpen(false);
  };

  const toggleNav = () => setOpen((value) => !value);

  useEffect(() => {
    window.scrollTo(0, 0);
  }, [location.pathname, location.search]);

  useEffect(() => {
    if (!showAvatarMenu) return;
    const close = () => setShowAvatarMenu(false);
    document.addEventListener('click', close);
    return () => document.removeEventListener('click', close);
  }, [showAvatarMenu]);

  return (
    <div
      id="page-container"
      // The original builds this class via raw String.concat (umi.js @909900):
      // `LOCALE+" sidebar-o "+(sidebarDark?"sidebar-dark":"")+" "+(headerDark?"page-header-dark":"")
      //  +" side-scroll page-header-fixed main-content-boxed side-trans-enabled "+(showNav&&"sidebar-o-xs")`.
      // In light/nav-closed that leaves three spaces before `side-scroll` and a trailing
      // literal `false`. A template literal reproduces the concat byte-for-byte; cn() does not.
      className={`${localeClass} sidebar-o ${darkSidebar ? 'sidebar-dark' : ''} ${darkHeader ? 'page-header-dark' : ''} side-scroll page-header-fixed main-content-boxed side-trans-enabled ${open && 'sidebar-o-xs'}`}
    >
      <div
        className="v2board-nav-mask"
        style={{ display: open ? 'block' : 'none' }}
        onClick={toggleNav}
      />

      <nav id="sidebar">
        <div className="smini-hidden bg-header-dark">
          <div className="content-header justify-content-lg-center bg-white-10">
            <a className="font-size-lg text-white" href="/">
              <span className="text-white-75">{title}</span>
            </a>
            <div className="d-lg-none">
              <a
                className="text-white ml-2"
                data-toggle="layout"
                data-action="sidebar_close"
                ref={legacyHref()}
                onClick={toggleNav}
              >
                <i className="fa fa-times-circle" />
              </a>
            </div>
          </div>
        </div>

        <div className="content-side content-side-full">
          <ul className="nav-main">
            {NAV.map((group, gi) => (
              <Fragment key={gi}>
                {group.labelKey && (
                  <li key={Math.random()} className="nav-main-heading">
                    {t(group.labelKey)}
                  </li>
                )}
                {group.items.map((item) => (
                  <li className="nav-main-item" key={Math.random()}>
                    <a
                      // Original inactive link renders the literal `false`:
                      // `"nav-main-link ".concat(pathname===to && "active")` (umi.js @895700).
                      className={`nav-main-link ${location.pathname === item.to && 'active'}`}
                      onClick={() => go(item.to)}
                    >
                      <i className={cn('nav-main-link-icon', item.iconClass)} />
                      <span className="nav-main-link-name">{t(item.labelKey)}</span>
                    </a>
                  </li>
                ))}
              </Fragment>
            ))}
          </ul>
        </div>

        <div className="v2board-copyright">{title} v1.7.4</div>
      </nav>

      <header id="page-header">
        <div className="content-header">
          <div className="sidebar-toggle" style={{ display: 'none' }}>
            <button
              type="button"
              className={darkHeader ? 'btn btn-primary mr-1 d-lg-none' : 'btn mr-1 d-lg-none'}
              onClick={toggleNav}
            >
              <i className="fa fa-fw fa-bars" />
            </button>
          </div>

          <div
            className={
              darkHeader ? 'v2board-container-title text-white' : 'v2board-container-title text-black'
            }
          >
            {activeLabel ? t(activeLabel) : ''}
          </div>

          <div>
            <div className="dropdown d-inline-block">
              <button
                type="button"
                className={headerButtonClass}
                onClick={() => {
                  const next = !darkMode;
                  setDarkMode(next);
                  setDarkModeState(next);
                }}
              >
                <i className={darkMode ? 'far fa fa-moon' : 'far fa fa-sun'} />
              </button>
            </div>

            <div className="dropdown d-inline-block">
              <LanguageMenu triggerClassName={headerButtonClass} legacyIcon />
            </div>

            <div className="dropdown d-inline-block">
              <button
                type="button"
                className={darkHeader ? 'btn btn-primary' : 'btn'}
                onClick={(event) => {
                  event.stopPropagation();
                  setShowAvatarMenu((value) => !value);
                }}
              >
                <i className="far fa fa-user-circle" />
                <span className="d-none d-lg-inline ml-1">{user?.email || 'Loading...'}</span>
                <i className="fa fa-fw fa-angle-down ml-1" />
              </button>
              <div className={`dropdown-menu dropdown-menu-right p-0 ${showAvatarMenu && 'show'}`}>
                <div className="p-2">
                  <a className="dropdown-item" href="/#/profile">
                    <i className="far fa-fw fa-user mr-1" /> {t('nav.profile')}
                  </a>
                  <a
                    className="dropdown-item"
                    ref={legacyHref()}
                    onClick={() => {
                      logout();
                      navigate('/login');
                    }}
                  >
                    <i className="far fa-fw fa-arrow-alt-circle-left mr-1" />{' '}
                    {t('common.logout')}
                  </a>
                </div>
              </div>
            </div>
          </div>
        </div>
      </header>

      <main id="main-container">
        <div className="content content-full">
          <RouteBoundaryOutlet />
        </div>
      </main>
    </div>
  );
}
