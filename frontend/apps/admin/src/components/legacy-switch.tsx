export function LegacySwitch({
  checked,
  onChange,
  size,
}: {
  checked: boolean | number | string;
  onChange?: (checked: boolean) => void;
  size?: 'small';
}) {
  const enabled = Boolean(checked);

  return (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      className={`${size === 'small' ? 'ant-switch-small ' : ''}ant-switch${
        enabled ? ' ant-switch-checked' : ''
      }`}
      onClick={() => onChange?.(!enabled)}
    >
      <span className="ant-switch-inner" />
    </button>
  );
}
