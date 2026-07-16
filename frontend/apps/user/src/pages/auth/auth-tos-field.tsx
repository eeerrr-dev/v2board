import { Trans } from 'react-i18next';
import { Checkbox } from '@/components/ui/checkbox';

export function getSafeTosHref(rawUrl: string): string | null {
  const url = rawUrl.trim().replace(/[\t\n\r]/g, '');
  if (!url) return null;

  if (/^[A-Za-z][A-Za-z0-9+.-]*:/.test(url)) {
    try {
      const parsed = new URL(url);
      return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? url : null;
    } catch {
      return null;
    }
  }

  // Relative href — browsers resolve backslashes to forward slashes, so normalize
  // before rejecting protocol-relative values that would otherwise resolve cross-origin.
  const normalized = url.replace(/\\/g, '/');
  return normalized.startsWith('//') ? null : normalized;
}

function TosSentence({ url }: { url: string }) {
  const safeHref = getSafeTosHref(url);
  const terms = safeHref ? (
    <a
      href={safeHref}
      target="_blank"
      rel="noopener noreferrer"
      className="text-primary underline underline-offset-4 transition-colors hover:text-primary/80"
    />
  ) : (
    <span />
  );

  return <Trans i18nKey={($) => $.auth.tos_html} components={{ terms }} />;
}

interface AuthTosFieldProps {
  checked: boolean;
  id: string;
  url: string;
  onToggle: () => void;
}

export function AuthTosField({ checked, id, url, onToggle }: AuthTosFieldProps) {
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
      <span id={textId}>
        <TosSentence url={url} />
      </span>
    </div>
  );
}
