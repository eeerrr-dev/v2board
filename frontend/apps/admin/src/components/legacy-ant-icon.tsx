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
  form: {
    viewBox: '64 64 896 896',
    paths: [
      'M904 512h-56c-4.4 0-8 3.6-8 8v320H160V160h320c4.4 0 8-3.6 8-8V96c0-4.4-3.6-8-8-8H144c-17.7 0-32 14.3-32 32v736c0 17.7 14.3 32 32 32h736c17.7 0 32-14.3 32-32V520c0-4.4-3.6-8-8-8z',
      'M355.9 534.9L354 653.8c-.1 8.9 7.1 16.2 16 16.2h.4l118-2.9c2-.1 4-.9 5.4-2.3l415.9-415c3.1-3.1 3.1-8.2 0-11.3L785.4 114.3c-1.6-1.6-3.6-2.3-5.7-2.3s-4.1.8-5.7 2.3l-415.8 415a8.3 8.3 0 0 0-2.3 5.6zm63.5 23.6L779.7 199l39.6 39.6-360.2 359.3-40.7 1z',
    ],
  },
  copy: {
    viewBox: '64 64 896 896',
    paths: [
      'M832 64H296c-4.4 0-8 3.6-8 8v56c0 4.4 3.6 8 8 8h496v688c0 4.4 3.6 8 8 8h56c4.4 0 8-3.6 8-8V96c0-17.7-14.3-32-32-32zM704 192H192c-17.7 0-32 14.3-32 32v530.7c0 8.5 3.4 16.6 9.4 22.6l173.3 173.3c2.2 2.2 4.7 4 7.4 5.5v1.9h4.2c3.5 1.3 7.2 2 11 2H704c17.7 0 32-14.3 32-32V224c0-17.7-14.3-32-32-32zM350 856.2L263.9 770H350v86.2zM664 888H414V746c0-22.1-17.9-40-40-40H232V264h432v624z',
    ],
  },
  delete: {
    viewBox: '64 64 896 896',
    paths: [
      'M360 184h-8c4.4 0 8-3.6 8-8v8h304v-8c0 4.4 3.6 8 8 8h-8v72h72v-80c0-35.3-28.7-64-64-64H352c-35.3 0-64 28.7-64 64v80h72v-72zm504 72H160c-17.7 0-32 14.3-32 32v32c0 4.4 3.6 8 8 8h60.4l24.7 523c1.6 34.1 29.8 61 63.9 61h454c34.2 0 62.3-26.8 63.9-61l24.7-523H888c4.4 0 8-3.6 8-8v-32c0-17.7-14.3-32-32-32zM731.3 840H292.7l-24.2-512h487l-24.2 512z',
    ],
  },
  user: {
    viewBox: '64 64 896 896',
    paths: [
      'M858.5 763.6a374 374 0 0 0-80.6-119.5 375.63 375.63 0 0 0-119.5-80.6c-.4-.2-.8-.3-1.2-.5C719.5 518 760 444.7 760 362c0-137-111-248-248-248S264 225 264 362c0 82.7 40.5 156 102.8 201.1-.4.2-.8.3-1.2.5-44.8 18.9-85 46-119.5 80.6a375.63 375.63 0 0 0-80.6 119.5A371.7 371.7 0 0 0 136 901.8a8 8 0 0 0 8 8.2h60c4.4 0 7.9-3.5 8-7.8 2-77.2 33-149.5 87.8-204.3 56.7-56.7 132-87.9 212.2-87.9s155.5 31.2 212.2 87.9C779 752.7 810 825 812 902.2c.1 4.4 3.6 7.8 8 7.8h60a8 8 0 0 0 8-8.2c-1-47.8-10.9-94.3-29.5-138.2zM512 534c-45.9 0-89.1-17.9-121.6-50.4S340 407.9 340 362c0-45.9 17.9-89.1 50.4-121.6S466.1 190 512 190s89.1 17.9 121.6 50.4S684 316.1 684 362c0 45.9-17.9 89.1-50.4 121.6S557.9 534 512 534z',
    ],
  },
  database: {
    viewBox: '64 64 896 896',
    paths: [
      'M832 64H192c-17.7 0-32 14.3-32 32v832c0 17.7 14.3 32 32 32h640c17.7 0 32-14.3 32-32V96c0-17.7-14.3-32-32-32zm-600 72h560v208H232V136zm560 480H232V408h560v208zm0 272H232V680h560v208zM304 240a40 40 0 1 0 80 0 40 40 0 1 0-80 0zm0 272a40 40 0 1 0 80 0 40 40 0 1 0-80 0zm0 272a40 40 0 1 0 80 0 40 40 0 1 0-80 0z',
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
  style,
  ...rest
}: LegacyAntIconProps & { name: LegacyAntIconName }) {
  const icon = LEGACY_ANT_ICONS[name];

  return (
    <i
      aria-label={`图标: ${name}`}
      {...rest}
      className={classNames('anticon', `anticon-${name}`, className)}
      style={style}
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
export const LegacyFormIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="form" {...props} />
);
export const LegacyCopyIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="copy" {...props} />
);
export const LegacyDeleteIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="delete" {...props} />
);
export const LegacyUserIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="user" {...props} />
);
export const LegacyDatabaseIcon = (props: LegacyAntIconProps) => (
  <LegacyAntIcon name="database" {...props} />
);
