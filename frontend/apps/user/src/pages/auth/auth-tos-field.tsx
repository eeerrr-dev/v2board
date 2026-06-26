import type { ReactNode } from 'react';

export function getSafeTosHref(rawUrl: string): string | null {
  const url = rawUrl.trim();
  if (!url || url.startsWith('//')) return null;

  if (/^[A-Za-z][A-Za-z0-9+.-]*:/.test(url)) {
    try {
      const parsed = new URL(url);
      return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? url : null;
    } catch {
      return null;
    }
  }

  return url;
}

function renderTosSentence(template: string, url: string): ReactNode {
  const match = template.match(/^([\s\S]*?)<a\b[^>]*>([\s\S]*?)<\/a>([\s\S]*)$/);
  if (!match) return template;
  const [, before, linkText, after] = match;
  const safeHref = getSafeTosHref(url);

  if (!safeHref) {
    return (
      <>
        {before}
        {linkText}
        {after}
      </>
    );
  }

  return (
    <>
      {before}
      <a
        href={safeHref}
        target="_blank"
        rel="noopener noreferrer"
        className="tw:text-primary tw:underline tw:transition tw:hover:text-primary-hover"
      >
        {linkText}
      </a>
      {after}
    </>
  );
}

interface AuthTosFieldProps {
  checked: boolean;
  id: string;
  template: string;
  url: string;
  onToggle: () => void;
}

export function AuthTosField({ checked, id, template, url, onToggle }: AuthTosFieldProps) {
  const textId = `${id}-text`;

  return (
    <div className="tw:flex tw:items-start tw:gap-2 tw:text-sm tw:text-foreground-muted">
      <input
        type="checkbox"
        checked={checked}
        onChange={onToggle}
        aria-labelledby={textId}
        className="tw:mt-1 tw:size-4 tw:rounded tw:border-input tw:accent-primary"
      />
      <span id={textId}>{renderTosSentence(template, url)}</span>
    </div>
  );
}
