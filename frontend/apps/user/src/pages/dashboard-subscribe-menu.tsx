import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Copy, Import as ImportIcon, QrCode } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { copyText } from '@v2board/config/clipboard';
import { getSiteTitle } from '@/lib/runtime-config';
import { toast } from '@/lib/toast';

interface DashboardSubscribeMenuProps {
  onOpenQr: () => void;
  subscribeUrl: string;
}

// Shared menu-row layout for the copy / QR / per-client import buttons so a
// tweak (padding, hover, focus ring) stays a single-line change instead of
// drifting across three sibling rows.
const SUBSCRIBE_MENU_ROW_CLASS =
  'flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50';

export function DashboardSubscribeMenu({ onOpenQr, subscribeUrl }: DashboardSubscribeMenuProps) {
  const { t } = useTranslation();
  // React Compiler memoizes this derivation; no manual useMemo needed.
  const subscribeTargets = subscribeUrl ? getSubscribeTargets(subscribeUrl) : [];

  const copyUrl = async () => {
    if (await copyText(subscribeUrl)) toast.success(t(($) => $.dashboard.copy_success));
  };

  return (
    <div data-testid="dashboard-subscribe-menu" className="grid gap-1 p-2">
      <button
        type="button"
        data-testid="dashboard-subscribe-copy"
        className={SUBSCRIBE_MENU_ROW_CLASS}
        onClick={copyUrl}
      >
        <Copy className="size-4 text-muted-foreground" />
        <span>{t(($) => $.dashboard.copy_subscribe)}</span>
      </button>
      <button
        type="button"
        data-testid="dashboard-subscribe-qrcode"
        className={SUBSCRIBE_MENU_ROW_CLASS}
        onClick={onOpenQr}
      >
        <QrCode className="size-4 text-muted-foreground" />
        <span>{t(($) => $.dashboard.scan_qrcode_subscribe)}</span>
      </button>
      {subscribeTargets.map((target) => (
        <button
          type="button"
          key={target.title}
          data-testid="dashboard-subscribe-target"
          data-subscribe-target={subscribeTargetSlug(target.title)}
          className={SUBSCRIBE_MENU_ROW_CLASS}
          onClick={() => {
            window.location.href = target.href;
          }}
        >
          <ImportIcon
            className="size-5 text-muted-foreground"
            aria-hidden="true"
            focusable="false"
          />
          <span>
            {t(($) => $.dashboard.import_to)} {target.title}
          </span>
        </button>
      ))}
      <div className="px-1 pb-1 pt-2">
        <Button asChild className="w-full">
          <Link to="/knowledge" data-testid="dashboard-subscribe-tutorial">
            {t(($) => $.dashboard.use_tutorial)}
          </Link>
        </Button>
      </div>
    </div>
  );
}

export function getSubscribeTargets(url: string) {
  const title = getSiteTitle();
  const userAgent = window.navigator.userAgent;
  const lowerUserAgent = userAgent.toLowerCase();
  const isAppleMobile =
    lowerUserAgent.includes('iphone') ||
    lowerUserAgent.includes('ipad') ||
    (/Mac/.test(userAgent) && window.navigator.maxTouchPoints > 2);
  // iPadOS Safari reports a "Macintosh" desktop UA; without excluding Apple
  // mobile devices an iPad would get both the iOS targets and the macOS-only
  // ClashX entry, so the two menus overlap.
  const isMac = lowerUserAgent.includes('macintosh') && !isAppleMobile;
  const isAndroid = lowerUserAgent.includes('android');
  const isWindows = lowerUserAgent.includes('windows');
  const shadowrocketPayload = window
    .btoa(`${url}&flag=shadowrocket`)
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
  const targets = [
    { title: 'Hiddify', href: `hiddify://import/${url}&flag=sing#${title}` },
    {
      title: 'Sing-box',
      href: `sing-box://import-remote-profile?url=${encodeURIComponent(url)}#${title}`,
    },
  ];
  if (isAppleMobile) {
    targets.push(
      {
        title: 'Shadowrocket',
        href: `shadowrocket://add/sub://${shadowrocketPayload}?remark=${title}`,
      },
      {
        title: 'QuantumultX',
        href: `quantumult-x:///update-configuration?remote-resource=${encodeURI(
          JSON.stringify({ server_remote: [`${url}, tag=${title}`] }),
        )}`,
      },
      {
        title: 'Surge',
        href: `surge:///install-config?url=${encodeURIComponent(url)}&name=${title}`,
      },
      {
        title: 'Stash',
        href: `stash://install-config?url=${encodeURIComponent(url)}&name=${title}`,
      },
    );
  }
  if (isMac) {
    targets.push({
      title: 'ClashX',
      href: `clash://install-config?url=${encodeURIComponent(url)}&name=${title}`,
    });
  }
  if (isWindows) {
    targets.push({
      title: 'ClashMeta',
      href: `clash://install-config?url=${encodeURIComponent(`${url}&flag=meta`)}&name=${title}`,
    });
  }
  if (isAndroid) {
    targets.push(
      {
        title: 'NekoBox For Android',
        href: `clash://install-config?url=${encodeURIComponent(`${url}&flag=meta`)}&name=${title}`,
      },
      {
        title: 'ClashMeta For Android',
        href: `clash://install-config?url=${encodeURIComponent(`${url}&flag=meta`)}&name=${title}`,
      },
      {
        title: 'Surfboard',
        href: `surge:///install-config?url=${encodeURIComponent(url)}&name=${title}`,
      },
    );
  }
  return targets;
}

function subscribeTargetSlug(title: string) {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}
