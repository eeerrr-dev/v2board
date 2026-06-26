import type { ReactNode } from 'react';
import { Checkbox } from '@/components/ui/checkbox';

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
        className="text-primary underline underline-offset-4 transition-colors hover:text-primary/80"
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
    <div className="flex items-start gap-2 text-sm text-muted-foreground">
      <Checkbox
        id={id}
        checked={checked}
        onCheckedChange={onToggle}
        aria-labelledby={textId}
        className="mt-0.5"
      />
      <span id={textId}>{renderTosSentence(template, url)}</span>
    </div>
  );
}
