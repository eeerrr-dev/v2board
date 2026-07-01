import { useMemo } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Copy, QrCode } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { copyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/toast';
import clashForAndroidIcon from '../assets/images/icon/Clash For Android.png';
import clashForWindowsIcon from '../assets/images/icon/Clash For Windows.png';
import clashMetaForAndroidIcon from '../assets/images/icon/ClashMeta For Android.png';
import clashMetaForWindowsIcon from '../assets/images/icon/ClashMeta For Windows.png';
import clashMetaIcon from '../assets/images/icon/ClashMeta.png';
import clashXIcon from '../assets/images/icon/ClashX.png';
import hiddifyIcon from '../assets/images/icon/Hiddify.png';
import nekoBoxForAndroidIcon from '../assets/images/icon/NekoBox For Android.png';
import quantumultXIcon from '../assets/images/icon/QuantumultX.png';
import shadowrocketIcon from '../assets/images/icon/Shadowrocket.png';
import singBoxIcon from '../assets/images/icon/Sing-box.png';
import stashIcon from '../assets/images/icon/Stash.png';
import surfboardIcon from '../assets/images/icon/Surfboard.png';
import surgeIcon from '../assets/images/icon/Surge.png';

interface DashboardSubscribeMenuProps {
  onOpenQr: () => void;
  subscribeUrl: string;
}

const SUBSCRIBE_TARGET_ICONS: Record<string, string> = {
  'Clash For Android': clashForAndroidIcon,
  'Clash For Windows': clashForWindowsIcon,
  'ClashMeta For Android': clashMetaForAndroidIcon,
  'ClashMeta For Windows': clashMetaForWindowsIcon,
  ClashMeta: clashMetaIcon,
  ClashX: clashXIcon,
  Hiddify: hiddifyIcon,
  'NekoBox For Android': nekoBoxForAndroidIcon,
  QuantumultX: quantumultXIcon,
  Shadowrocket: shadowrocketIcon,
  'Sing-box': singBoxIcon,
  Stash: stashIcon,
  Surfboard: surfboardIcon,
  Surge: surgeIcon,
};

export function DashboardSubscribeMenu({
  onOpenQr,
  subscribeUrl,
}: DashboardSubscribeMenuProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const subscribeTargets = useMemo(
    () => (subscribeUrl ? getSubscribeTargets(subscribeUrl) : []),
    [subscribeUrl],
  );

  const copyUrl = async () => {
    if (await copyText(subscribeUrl)) toast.success(t('dashboard.copy_success'));
  };

  return (
    <div data-testid="dashboard-subscribe-menu" className="grid gap-1 p-2">
      <button
        type="button"
        data-testid="dashboard-subscribe-copy"
        className="flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
        onClick={copyUrl}
      >
        <Copy className="size-4 text-muted-foreground" />
        <span>{t('dashboard.copy_subscribe')}</span>
      </button>
      <button
        type="button"
        data-testid="dashboard-subscribe-qrcode"
        className="flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
        onClick={onOpenQr}
      >
        <QrCode className="size-4 text-muted-foreground" />
        <span>{t('dashboard.scan_qrcode_subscribe')}</span>
      </button>
      {subscribeTargets.map((target) => (
        <button
          type="button"
          key={target.title}
          data-testid="dashboard-subscribe-target"
          data-subscribe-target={subscribeTargetSlug(target.title)}
          className="flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
          onClick={() => {
            window.location.href = target.href;
          }}
        >
          <img className="size-5 rounded-sm" src={SUBSCRIBE_TARGET_ICONS[target.title]} alt="" />
          <span>
            {t('dashboard.import_to')} {target.title}
          </span>
        </button>
      ))}
      <div className="px-1 pb-1 pt-2">
        <Button
          type="button"
          data-testid="dashboard-subscribe-tutorial"
          className="w-full"
          onClick={() => navigate('/knowledge')}
        >
          {t('dashboard.use_tutorial')}
        </Button>
      </div>
    </div>
  );
}

export function getSubscribeTargets(url: string) {
  const title = window.settings!.title;
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
