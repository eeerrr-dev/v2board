import { useTranslation } from 'react-i18next';
import { useEffect, useRef, useState } from 'react';
import { SUPPORTED_LOCALES, RTL_LOCALES, type SupportedLocale } from '@v2board/i18n';
import { setLegacyCookie } from '@/lib/legacy-cookie';

function getEnabledLocales() {
  const legacyI18n = window.settings?.i18n;
  if (!Array.isArray(legacyI18n)) return SUPPORTED_LOCALES;
  const enabled = new Set([...legacyI18n].sort());
  return [...enabled]
    .map((code) => SUPPORTED_LOCALES.find((locale) => locale.code === code))
    .filter((locale): locale is (typeof SUPPORTED_LOCALES)[number] => Boolean(locale));
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
  const { i18n } = useTranslation();
  const rootRef = useRef<HTMLSpanElement | null>(null);
  const [open, setOpen] = useState(false);
  const current = (i18n.resolvedLanguage ?? 'zh-CN') as SupportedLocale;
  const locales = getEnabledLocales();
  const currentLabel = locales.find((locale) => locale.code === current)?.label ?? current;

  const selectLocale = (locale: SupportedLocale) => {
    window.localStorage.setItem('umi_locale', locale);
    setLegacyCookie('i18n', locale);
    setOpen(false);
    void i18n.changeLanguage(locale);
  };

  useEffect(() => {
    const isRtl = RTL_LOCALES.includes(current);
    window.localStorage.setItem('umi_locale', current);
    document.documentElement.dir = isRtl ? 'rtl' : 'ltr';
    document.documentElement.lang = current;
  }, [current]);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      setOpen(false);
    };
    document.addEventListener('click', close);
    return () => document.removeEventListener('click', close);
  }, [open]);

  const popover = open ? (
    <div className="ant-popover ant-popover-placement-top v2board-language-popover">
      <div className="ant-popover-content">
        <div className="ant-popover-arrow" />
        <div className="ant-popover-inner" role="tooltip">
          <div className="ant-popover-inner-content">
            <ul className="ant-menu ant-menu-light ant-menu-root ant-menu-vertical">
              {locales.map((locale) => (
                <li
                  key={locale.code}
                  className="ant-menu-item"
                  onClick={(event) => {
                    event.stopPropagation();
                    selectLocale(locale.code);
                  }}
                >
                  {locale.label}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </div>
  ) : null;

  if (showLabel && legacyIcon) {
    return (
      <span
        ref={rootRef}
        className={`${triggerClassName ?? 'v2board-login-i18n-btn'} v2board-language-popover-wrapper`}
        onClick={() => setOpen((value) => !value)}
      >
        <i className="si si-globe pr-1" aria-hidden />
        <span className="font-size-sm text-muted" style={{ verticalAlign: 'text-bottom' }}>
          {currentLabel}
        </span>
        {popover}
      </span>
    );
  }

  return (
    <span ref={rootRef} className="v2board-language-popover-wrapper">
      <button
        type="button"
        className={triggerClassName ?? (showLabel ? 'v2board-login-i18n-btn' : 'btn')}
        onClick={() => setOpen((value) => !value)}
      >
        <i className="far fa fa-language" aria-hidden />
        {showLabel && (
          <span className="font-size-sm text-muted" style={{ verticalAlign: 'text-bottom' }}>
            {currentLabel}
          </span>
        )}
      </button>
      {popover}
    </span>
  );
}
