import type { ComponentPropsWithoutRef } from 'react';

const LEGACY_ANT_ICONS = {
  plus: {
    viewBox: '64 64 896 896',
    paths: [
      'M482 152h60q8 0 8 8v704q0 8-8 8h-60q-8 0-8-8V160q0-8 8-8z',
      'M176 474h672q8 0 8 8v60q0 8-8 8H176q-8 0-8-8v-60q0-8 8-8z',
    ],
  },
} as const;

type LegacyAntIconName = keyof typeof LEGACY_ANT_ICONS;
type LegacyAntIconProps = ComponentPropsWithoutRef<'i'>;

function classNames(...values: Array<string | undefined>) {
  return values.filter(Boolean).join(' ');
}

function LegacyAntIcon({
  name,
  className,
  ...rest
}: LegacyAntIconProps & { name: LegacyAntIconName }) {
  const icon = LEGACY_ANT_ICONS[name];

  return (
    <i
      {...rest}
      aria-label={`图标: ${name}`}
      className={classNames('anticon', `anticon-${name}`, className)}
    >
      <svg
        viewBox={icon.viewBox}
        focusable="false"
        className=""
        data-icon={name}
        width="1em"
        height="1em"
        fill="currentColor"
        aria-hidden="true"
      >
        {icon.paths.map((d) => (
          <path key={d} d={d} />
        ))}
      </svg>
    </i>
  );
}

export const LegacyPlusIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="plus" {...props} />
);
