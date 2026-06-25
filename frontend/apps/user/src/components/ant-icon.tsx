import { useTranslation } from 'react-i18next';
import type { ComponentPropsWithoutRef } from 'react';
import { getLocaleAntdMessages } from '@v2board/i18n';
import { cn } from '@/lib/cn';
import { ANT_ICONS, type AntIconName } from '@/lib/ant-icons';

// The original renders every anticon as an inline antd SVG (its umi.css bundles
// Font Awesome only for the author's explicit `fa-*` chrome icons). These
// components reproduce antd v3's Icon DOM verbatim from the shared path data in
// `lib/ant-icons`, so every status/action icon matches the original pixel-for-pixel
// instead of a Font Awesome glyph.

// antd v3 ships an Icon locale word only for zh-CN ("图标"); every other locale
// (incl. zh-TW/ja-JP) falls back to en_US's "icon". Sourced from the shared registry.
function useIconWord() {
  const { i18n } = useTranslation();
  return getLocaleAntdMessages(i18n.language).iconWord;
}

// antd v3's Icon spreads every passed prop onto the <i> and forwards events, so a
// parent (e.g. antd Tooltip cloning the icon to attach hover handlers) reaches the DOM
// node directly; mirror that by spreading the rest of the <i> props.
type AntIconProps = ComponentPropsWithoutRef<'i'>;

// <i aria-label="<word>: <name>" class="anticon anticon-<name>"><svg ...><path/>…
function AntIcon({ name, className, ...rest }: AntIconProps & { name: AntIconName }) {
  const word = useIconWord();
  const { viewBox, paths } = ANT_ICONS[name];
  return (
    <i
      {...rest}
      aria-label={`${word}: ${name}`}
      className={cn(`anticon anticon-${name}`, className)}
    >
      <svg
        viewBox={viewBox}
        focusable="false"
        className=""
        data-icon={name}
        width="1em"
        height="1em"
        fill="currentColor"
        aria-hidden="true"
      >
        {paths.map((d) => (
          <path key={d} d={d} />
        ))}
      </svg>
    </i>
  );
}

export const TransactionIcon = (props: AntIconProps) => <AntIcon name="transaction" {...props} />;
export const PayCircleIcon = (props: AntIconProps) => <AntIcon name="pay-circle" {...props} />;
export const QuestionCircleIcon = (props: AntIconProps) => <AntIcon name="question-circle" {...props} />;
export const CheckCircleIcon = (props: AntIconProps) => <AntIcon name="check-circle" {...props} />;
export const InfoCircleIcon = (props: AntIconProps) => <AntIcon name="info-circle" {...props} />;
export const ExclamationCircleIcon = (props: AntIconProps) => (
  <AntIcon name="exclamation-circle" {...props} />
);
export const WarningIcon = (props: AntIconProps) => <AntIcon name="warning" {...props} />;
export const SearchIcon = (props: AntIconProps) => <AntIcon name="search" {...props} />;
export const CloseIcon = (props: AntIconProps) => <AntIcon name="close" {...props} />;
export const LeftIcon = (props: AntIconProps) => <AntIcon name="left" {...props} />;
export const RightIcon = (props: AntIconProps) => <AntIcon name="right" {...props} />;
export const DoubleLeftIcon = (props: AntIconProps) => <AntIcon name="double-left" {...props} />;
export const DoubleRightIcon = (props: AntIconProps) => <AntIcon name="double-right" {...props} />;
export const DownIcon = (props: AntIconProps) => <AntIcon name="down" {...props} />;
