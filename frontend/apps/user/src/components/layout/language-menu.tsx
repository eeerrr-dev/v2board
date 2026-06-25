import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { legacyGetLocale, legacySetLocale, SUPPORTED_LOCALES } from '@v2board/i18n';
import { setLegacyCookie } from '@/lib/legacy-cookie';
import { useTransitionStatus } from '@/lib/use-transition-status';

const I18N_TEXT = Object.fromEntries(SUPPORTED_LOCALES.map((locale) => [locale.code, locale.label]));

function getEnabledLocales() {
  // The enabled list comes from the operator backend (window.settings.i18n); drop any
  // locale the frontend no longer bundles a label/translation for instead of rendering
  // a blank menu item.
  return window
    .settings!.i18n!.sort()
    .filter((code) => code in I18N_TEXT)
    .map((code) => ({ code, label: I18N_TEXT[code] }));
}

interface LanguageMenuProps {
  showLabel?: boolean;
  triggerClassName?: string;
  legacyIcon?: boolean;
  placement?: 'topCenter' | 'bottomCenter';
}

export function LanguageMenu({
  showLabel = false,
  triggerClassName,
  legacyIcon = false,
  placement: requestedPlacement,
}: LanguageMenuProps) {
  // The original is umi's SelectLang: an antd Dropdown (trigger:click) that clones
  // its trigger adding `ant-dropdown-trigger` (+`ant-dropdown-open` when open) and
  // portals the overlay Menu to document.body. Reproduce both the trigger className
  // and the body portal.
  const triggerRef = useRef<HTMLElement | null>(null);
  const popupRef = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{ left: number; top: number } | null>(null);
  const placement = requestedPlacement ?? (showLabel ? 'topCenter' : 'bottomCenter');
  // antd keeps the overlay mounted and runs the placement-specific leave animation on close.
  const dropdownStatus = useTransitionStatus(open, 230, 30);
  const motionName = placement === 'topCenter' ? 'slide-down' : 'slide-up';
  const slideClass =
    dropdownStatus === 'leave'
      ? `${motionName}-leave`
      : dropdownStatus === 'leaving'
        ? `${motionName}-leave ${motionName}-leave-active`
        : dropdownStatus === 'enter'
          ? `${motionName}-enter`
          : dropdownStatus === 'entering'
            ? `${motionName}-enter ${motionName}-enter-active`
            : '';
  const locales = getEnabledLocales();
  const currentLabel = SUPPORTED_LOCALES.find((locale) => locale.code === legacyGetLocale())?.label;

  const selectLocale = useCallback((locale: string) => {
    setOpen(false);
    setLegacyCookie('i18n', locale);
    legacySetLocale(locale);
  }, []);

  // rc-align positions the body-portaled overlay with absolute document coordinates.
  // Auth pages use topCenter (bottom-center to trigger top-center, [0,-4]); the
  // authenticated header uses bottomCenter (top-center to trigger bottom-center, [0,4]).
  const reposition = useCallback(() => {
    const rect = triggerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const popupWidth = popupRef.current?.offsetWidth ?? 0;
    const popupHeight = popupRef.current?.offsetHeight ?? 0;
    const center = window.scrollX + rect.left + rect.width / 2;
    const rawLeft = popupWidth ? center - popupWidth / 2 : center;
    const rawTop =
      placement === 'topCenter'
        ? window.scrollY + rect.top - 4 - popupHeight
        : window.scrollY + rect.bottom + 4;
    const maxLeft = popupWidth
      ? window.scrollX + Math.max(0, window.innerWidth - popupWidth)
      : rawLeft;
    const maxTop = popupHeight
      ? window.scrollY + Math.max(0, window.innerHeight - popupHeight)
      : rawTop;
    const next = {
      left: popupWidth ? Math.min(Math.max(rawLeft, window.scrollX), maxLeft) : rawLeft,
      top: popupHeight ? Math.min(Math.max(rawTop, window.scrollY), maxTop) : rawTop,
    };
    setCoords((current) =>
      current?.left === next.left && current.top === next.top ? current : next,
    );
  }, [placement]);

  useLayoutEffect(() => {
    if (dropdownStatus === 'exited' || !coords || !popupRef.current) return;
    reposition();
  }, [coords, dropdownStatus, reposition]);

  useEffect(() => {
    if (!open) return;
    reposition();
    window.addEventListener('scroll', reposition, true);
    window.addEventListener('resize', reposition);
    return () => {
      window.removeEventListener('scroll', reposition, true);
      window.removeEventListener('resize', reposition);
    };
  }, [open, reposition]);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      const target = event.target as Node;
      if (triggerRef.current?.contains(target) || popupRef.current?.contains(target)) return;
      setOpen(false);
    };
    document.addEventListener('click', close);
    return () => document.removeEventListener('click', close);
  }, [open]);

  const handleLocaleItemNativeSelect = useCallback(
    (event: MouseEvent) => {
      const item = event.currentTarget;
      if (!(item instanceof HTMLElement)) return;
      const locale = item.dataset.localeCode;
      if (!locale) return;
      event.preventDefault();
      event.stopPropagation();
      selectLocale(locale);
    },
    [selectLocale],
  );

  const popover =
    dropdownStatus !== 'exited' && coords
      ? createPortal(
          <div
            ref={popupRef}
            className={`ant-dropdown ant-dropdown-placement-${placement}`}
            style={{
              position: 'absolute',
              left: coords.left,
              top: coords.top,
              zIndex: 1050,
            }}
          >
            <ul
              className={[
                'ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical',
                slideClass,
              ]
                .filter(Boolean)
                .join(' ')}
              role="menu"
            >
              {locales.map((locale) => (
                <li
                  key={locale.code}
                  ref={(element) => {
                    if (element) {
                      element.onmousedown = handleLocaleItemNativeSelect;
                      element.onclick = handleLocaleItemNativeSelect;
                    }
                  }}
                  className="ant-dropdown-menu-item"
                  role="menuitem"
                  aria-disabled="false"
                  data-locale-code={locale.code}
                >
                  {locale.label}
                </li>
              ))}
            </ul>
          </div>,
          document.body,
        )
      : null;

  const triggerClass = `${triggerClassName ?? (showLabel ? 'v2board-login-i18n-btn' : 'btn')} ant-dropdown-trigger${
    open ? ' ant-dropdown-open' : ''
  }`;
  const toggleOpen = () => {
    if (!open) reposition();
    setOpen((value) => !value);
  };

  if (showLabel && legacyIcon) {
    return (
      <>
        <span
          ref={(element) => {
            triggerRef.current = element;
          }}
          className={triggerClass}
          onClick={toggleOpen}
        >
          <i className="si si-globe pr-1" />
          <span className="font-size-sm text-muted" style={{ verticalAlign: 'text-bottom' }}>
            {currentLabel}
          </span>
        </span>
        {popover}
      </>
    );
  }

  return (
    <>
      <button
        type="button"
        ref={(element) => {
          triggerRef.current = element;
        }}
        className={triggerClass}
        onClick={toggleOpen}
      >
        <i className="far fa fa-language" />
        {showLabel && (
          <span className="font-size-sm text-muted" style={{ verticalAlign: 'text-bottom' }}>
            {currentLabel}
          </span>
        )}
      </button>
      {popover}
    </>
  );
}
