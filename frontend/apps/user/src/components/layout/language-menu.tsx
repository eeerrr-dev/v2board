import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { legacyGetLocale, legacySetLocale, SUPPORTED_LOCALES } from '@v2board/i18n';
import { setLegacyCookie } from '@/lib/legacy-cookie';
import { useTransitionStatus } from '@/lib/use-transition-status';

const I18N_TEXT = Object.fromEntries(SUPPORTED_LOCALES.map((locale) => [locale.code, locale.label]));

function getEnabledLocales() {
  return window.settings!.i18n!.sort().map((code) => ({ code, label: I18N_TEXT[code] }));
}

interface LanguageMenuProps {
  showLabel?: boolean;
  triggerClassName?: string;
  legacyIcon?: boolean;
}

export function LanguageMenu({
  showLabel = false,
  triggerClassName,
  legacyIcon = false,
}: LanguageMenuProps) {
  // The original is umi's SelectLang: an antd Dropdown (trigger:click, placement:topCenter)
  // that clones its trigger adding `ant-dropdown-trigger` (+`ant-dropdown-open` when open)
  // and portals the overlay Menu to document.body. Reproduce both — the trigger className
  // and the body portal.
  const triggerRef = useRef<HTMLElement | null>(null);
  const popupRef = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{ left: number; top: number } | null>(null);
  // antd keeps the overlay mounted and runs the "slide-down" leave animation on close.
  // For top placements, antd applies
  // it to the px-positioned popup wrapper; here the wrapper carries the translate(-50%,-100%)
  // that anchors its bottom-center to the trigger top (rc-align points ["bc","tc"], offset
  // [0,-4]), so the scaleY keyframe runs on the inner .ant-dropdown-menu instead — visually
  // identical, since the menu (with its shadow) fills the wrapper.
  const dropdownStatus = useTransitionStatus(open, 230, 30);
  const slideClass =
    dropdownStatus === 'leave'
      ? 'slide-down-leave'
      : dropdownStatus === 'leaving'
        ? 'slide-down-leave slide-down-leave-active'
        : dropdownStatus === 'enter'
          ? 'slide-down-enter'
          : dropdownStatus === 'entering'
            ? 'slide-down-enter slide-down-enter-active'
            : '';
  const locales = getEnabledLocales();
  const currentLabel = SUPPORTED_LOCALES.find((locale) => locale.code === legacyGetLocale())?.label;

  // The original SelectLang.set() calls umi setLocale(e) with one argument, then writes
  // the i18n cookie. setLocale itself decides whether a reload is needed.
  const selectLocale = (locale: string) => {
    setOpen(false);
    legacySetLocale(locale);
    setLegacyCookie('i18n', locale);
  };

  // rc-align anchors the overlay's bottom-center to the trigger's top-center with offset
  // [0,-4] (4px gap above the trigger); recompute on scroll/resize like rc-align re-aligns.
  const reposition = useCallback(() => {
    const rect = triggerRef.current?.getBoundingClientRect();
    if (rect) setCoords({ left: rect.left + rect.width / 2, top: rect.top - 4 });
  }, []);

  useLayoutEffect(() => {
    if (dropdownStatus === 'exited' || !coords || !popupRef.current) return;
    const rect = triggerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const popupHeight = popupRef.current.offsetHeight;
    const popupWidth = popupRef.current.offsetWidth;
    if (!popupHeight && !popupWidth) return;
    const minLeft = popupWidth / 2;
    const maxLeft = Math.max(minLeft, window.innerWidth - popupWidth / 2);

    const next = {
      left: popupWidth
        ? Math.min(Math.max(rect.left + rect.width / 2, minLeft), maxLeft)
        : coords.left,
      top: popupHeight ? Math.max(rect.top - 4, popupHeight) : coords.top,
    };

    if (next.left !== coords.left || next.top !== coords.top) setCoords(next);
  }, [coords, dropdownStatus]);

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

  const popover =
    dropdownStatus !== 'exited' && coords
      ? createPortal(
          // antd builds the body-portaled wrapper class as `"ant-dropdown" + " " + "" + " "
          // + placementClass`; the empty middle token leaves a double space, reproduced here.
          <div
            ref={popupRef}
            className="ant-dropdown  ant-dropdown-placement-topCenter"
            style={{
              position: 'fixed',
              left: coords.left,
              top: coords.top,
              transform: 'translate(-50%, -100%)',
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
                  className="ant-dropdown-menu-item"
                  role="menuitem"
                  aria-disabled="false"
                  onClick={(event) => {
                    event.stopPropagation();
                    selectLocale(locale.code);
                  }}
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

  if (showLabel && legacyIcon) {
    return (
      <>
        <span
          ref={(element) => {
            triggerRef.current = element;
          }}
          className={triggerClass}
          onClick={() => setOpen((value) => !value)}
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
        onClick={() => setOpen((value) => !value)}
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
