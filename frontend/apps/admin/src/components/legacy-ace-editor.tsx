import { useMemo, useState, type CSSProperties, type TextareaHTMLAttributes } from 'react';

interface LegacyAceJsonEditorProps extends Omit<
  TextareaHTMLAttributes<HTMLTextAreaElement>,
  'onChange'
> {
  onChange?: (value: string) => void;
}

const editorStyle: CSSProperties = {
  width: 500,
  height: 500,
  fontSize: 14,
};

function lineNumbers(value: string | number | readonly string[] | undefined) {
  const text = Array.isArray(value) ? value.join('\n') : value == null ? '' : String(value);
  return Array.from({ length: Math.max(1, text.split('\n').length) }, (_line, index) => index + 1);
}

function renderHighlightedJsonLine(line: string) {
  const tokens =
    /("(?:\\.|[^"\\])*")|(-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b)|\b(true|false|null)\b/g;
  const parts = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = tokens.exec(line))) {
    if (match.index > lastIndex) parts.push(line.slice(lastIndex, match.index));
    if (match[1]) {
      parts.push(
        <span key={`${match.index}-string`} className="ace_string">
          {match[1]}
        </span>,
      );
    } else if (match[2]) {
      parts.push(
        <span key={`${match.index}-number`} className="ace_constant ace_numeric">
          {match[2]}
        </span>,
      );
    } else {
      parts.push(
        <span key={`${match.index}-constant`} className="ace_constant ace_language">
          {match[3]}
        </span>,
      );
    }
    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < line.length) parts.push(line.slice(lastIndex));
  return parts.length ? parts : '\u00a0';
}

function renderAceLines(value: string | number | readonly string[] | undefined) {
  const text = Array.isArray(value) ? value.join('\n') : value == null ? '' : String(value);
  return (text || '').split('\n').map((line, index) => (
    <div key={index} className="ace_line">
      {renderHighlightedJsonLine(line)}
    </div>
  ));
}

export function LegacyAceJsonEditor({
  onChange,
  placeholder,
  value,
  ...rest
}: LegacyAceJsonEditorProps) {
  const [focused, setFocused] = useState(false);
  const lines = useMemo(() => lineNumbers(value), [value]);

  return (
    <div
      className={`ace_editor ace-github${focused ? ' ace_focus' : ''}`}
      data-legacy-mode="json"
      data-legacy-theme="github"
      data-legacy-show-print-margin="true"
      data-legacy-show-gutter="true"
      data-legacy-highlight-active-line="true"
      data-legacy-enable-basic-autocompletion="false"
      data-legacy-enable-live-autocompletion="false"
      data-legacy-enable-snippets="false"
      data-legacy-show-line-numbers="true"
      data-legacy-tab-size="2"
      style={editorStyle}
    >
      <div className="ace_gutter">
        <div className="ace_layer ace_gutter-layer">
          {lines.map((line) => (
            <div key={line} className="ace_gutter-cell">
              {line}
            </div>
          ))}
        </div>
      </div>
      <div className="ace_scroller">
        <div className="ace_content">
          <div className="ace_layer ace_marker-layer">
            <div className="ace_active-line" />
          </div>
          <div className="ace_layer ace_text-layer">{renderAceLines(value)}</div>
          <div className="ace_layer ace_cursor-layer">
            <div className="ace_cursor" />
          </div>
          {value ? null : <div className="ace_placeholder">{placeholder}</div>}
        </div>
      </div>
      <div className="ace_layer ace_print-margin-layer">
        <div className="ace_print-margin" />
      </div>
      <textarea
        {...rest}
        className="ace_text-input legacy-ace-json-input"
        value={value}
        onFocus={(event) => {
          setFocused(true);
          rest.onFocus?.(event);
        }}
        onBlur={(event) => {
          setFocused(false);
          rest.onBlur?.(event);
        }}
        onChange={(event) => onChange?.(event.target.value)}
      />
    </div>
  );
}
