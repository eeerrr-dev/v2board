import type { ComponentPropsWithoutRef } from 'react';

const LEGACY_ANT_ICONS = {
  plus: {
    viewBox: '64 64 896 896',
    paths: [
      'M482 152h60q8 0 8 8v704q0 8-8 8h-60q-8 0-8-8V160q0-8 8-8z',
      'M176 474h672q8 0 8 8v60q0 8-8 8H176q-8 0-8-8v-60q0-8 8-8z',
    ],
  },
  filter: {
    viewBox: '64 64 896 896',
    paths: [
      'M349 838c0 17.7 14.2 32 31.8 32h262.4c17.6 0 31.8-14.3 31.8-32V642H349v196zm531.1-684H143.9c-24.5 0-39.8 26.7-27.5 48l221.3 376h348.8l221.3-376c12.1-21.3-3.2-48-27.7-48z',
    ],
  },
  'question-circle': {
    viewBox: '64 64 896 896',
    paths: [
      'M512 64C264.6 64 64 264.6 64 512s200.6 448 448 448 448-200.6 448-448S759.4 64 512 64zm0 820c-205.4 0-372-166.6-372-372s166.6-372 372-372 372 166.6 372 372-166.6 372-372 372z',
      'M623.6 316.7C593.6 290.4 554 276 512 276s-81.6 14.5-111.6 40.7C369.2 344 352 380.7 352 420v7.6c0 4.4 3.6 8 8 8h48c4.4 0 8-3.6 8-8V420c0-44.1 43.1-80 96-80s96 35.9 96 80c0 31.1-22 59.6-56.1 72.7-21.2 8.1-39.2 22.3-52.1 40.9-13.1 19-19.9 41.8-19.9 64.9V620c0 4.4 3.6 8 8 8h48c4.4 0 8-3.6 8-8v-22.7a48.3 48.3 0 0 1 30.9-44.8c59-22.7 97.1-74.7 97.1-132.5.1-39.3-17.1-76-48.3-103.3zM472 732a40 40 0 1 0 80 0 40 40 0 1 0-80 0z',
    ],
  },
  'caret-up': {
    viewBox: '0 0 1024 1024',
    paths: [
      'M858.9 689L530.5 308.2c-9.4-10.9-27.5-10.9-37 0L165.1 689c-12.2 14.2-1.2 35 18.5 35h656.8c19.7 0 30.7-20.8 18.5-35z',
    ],
  },
  'caret-down': {
    viewBox: '0 0 1024 1024',
    paths: [
      'M840.4 300H183.6c-19.7 0-30.7 20.8-18.5 35l328.4 380.8c9.4 10.9 27.5 10.9 37 0L858.9 335c12.2-14.2 1.2-35-18.5-35z',
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
      aria-label={`图标: ${name}`}
      {...rest}
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
export const LegacyFilterIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="filter" {...props} />
);
export const LegacyQuestionCircleIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="question-circle" {...props} />
);
export const LegacyCaretUpIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="caret-up" {...props} />
);
export const LegacyCaretDownIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="caret-down" {...props} />
);
