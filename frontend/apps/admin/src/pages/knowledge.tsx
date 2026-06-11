import {
  cloneElement,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactElement,
} from 'react';
import { App } from 'antd';
import dayjs from 'dayjs';
import MarkdownIt from 'markdown-it';
import { admin } from '@v2board/api-client';
import type { Knowledge, KnowledgeSummary } from '@v2board/types';
import { apiClient } from '@/lib/api';
import {
  useAdminKnowledge,
  useAdminKnowledgeCategories,
  useDropKnowledgeMutation,
  useSaveKnowledgeMutation,
  useShowKnowledgeMutation,
  useSortKnowledgeMutation,
} from '@/lib/queries';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';
import { LegacyDragSort, LegacyMenuIcon } from '@/components/legacy-drag-sort';
import { LegacyButton } from '@/components/legacy-button';
import { legacyConfirm } from '@/components/legacy-confirm';
import { LegacyLoadingIcon, LegacyPlusIcon } from '@/components/legacy-ant-icon';
import { LegacySelect } from '@/components/legacy-select';
import { LegacyInput } from '@/components/legacy-input';
import { LegacyDrawer } from '@/components/legacy-drawer';
import {
  LegacyStandaloneTable,
  legacyTableRowKey,
  type LegacyStandaloneTableHeader,
} from '@/components/legacy-standalone-table';
import { LegacyDivider } from '@/components/legacy-divider';
import { LegacySwitch } from '@/components/legacy-switch';

type SaveKnowledgePayload = Parameters<typeof admin.saveKnowledge>[1];

const legacyAdminMarkdown = new MarkdownIt({ html: true, linkify: true, typographer: true });
const LEGACY_KNOWLEDGE_I18N_TEXT = {
  'zh-CN': '简体中文',
  'zh-TW': '繁體中文',
  'en-US': 'English',
  'ja-JP': '日本語',
  'vi-VN': 'Tiếng Việt',
  'ko-KR': '한국어',
} as const;
type LegacyKnowledgeLocale = keyof typeof LEGACY_KNOWLEDGE_I18N_TEXT;
const LEGACY_KNOWLEDGE_LOCALES = (
  Object.keys(LEGACY_KNOWLEDGE_I18N_TEXT) as LegacyKnowledgeLocale[]
).sort();
const LEGACY_KNOWLEDGE_LOCALE_OPTIONS = LEGACY_KNOWLEDGE_LOCALES.map((locale) => ({
  value: locale,
  label: LEGACY_KNOWLEDGE_I18N_TEXT[locale],
}));

function renderLegacyAdminMarkdown(markdown: string) {
  return legacyAdminMarkdown.render(markdown);
}

function normalizeLegacyMarkdownValue(value: unknown) {
  if (typeof value === 'undefined') return '';
  return (typeof value === 'string' ? value : String(value).toString()).replace(/\u21b5/g, '\n');
}

const LEGACY_MARKDOWN_LABELS = {
  enUS: {
    clearTip: 'Are you sure you want to clear all contents?',
    btnHeader: 'Header',
    btnClear: 'Clear',
    btnBold: 'Bold',
    btnItalic: 'Italic',
    btnUnderline: 'Underline',
    btnStrikethrough: 'Strikethrough',
    btnUnordered: 'Unordered list',
    btnOrdered: 'Ordered list',
    btnQuote: 'Quote',
    btnLineBreak: 'Line break',
    btnInlineCode: 'Inline code',
    btnCode: 'Code',
    btnTable: 'Table',
    btnImage: 'Image',
    btnLink: 'Link',
    btnUndo: 'Undo',
    btnRedo: 'Redo',
    btnFullScreen: 'Full screen',
    btnExitFullScreen: 'Exit full screen',
    btnModeEditor: 'Only display editor',
    btnModePreview: 'Only display preview',
    btnModeAll: 'Display both editor and preview',
  },
  zhCN: {
    clearTip: '您确定要清空所有内容吗？',
    btnHeader: '标题',
    btnClear: '清空',
    btnBold: '加粗',
    btnItalic: '斜体',
    btnUnderline: '下划线',
    btnStrikethrough: '删除线',
    btnUnordered: '无序列表',
    btnOrdered: '有序列表',
    btnQuote: '引用',
    btnLineBreak: '换行',
    btnInlineCode: '行内代码',
    btnCode: '代码块',
    btnTable: '表格',
    btnImage: '图片',
    btnLink: '链接',
    btnUndo: '撤销',
    btnRedo: '重做',
    btnFullScreen: '全屏',
    btnExitFullScreen: '退出全屏',
    btnModeEditor: '仅显示编辑器',
    btnModePreview: '仅显示预览',
    btnModeAll: '显示编辑器与预览',
  },
} as const;
type LegacyMarkdownLocaleKey = keyof typeof LEGACY_MARKDOWN_LABELS;

function normalizeLegacyMarkdownLocale(locale?: string) {
  if (!locale) return null;
  const parts = locale.split('-');
  const key = `${parts[0]}${parts.length > 1 ? parts[parts.length - 1]?.toUpperCase() : ''}`;
  return key in LEGACY_MARKDOWN_LABELS ? (key as LegacyMarkdownLocaleKey) : null;
}

function getLegacyMarkdownLabels() {
  if (typeof navigator === 'undefined') return LEGACY_MARKDOWN_LABELS.enUS;
  const browserNavigator = navigator as Navigator & { browserLanguage?: string };
  const locale =
    normalizeLegacyMarkdownLocale(browserNavigator.language) ??
    normalizeLegacyMarkdownLocale(browserNavigator.browserLanguage);
  return LEGACY_MARKDOWN_LABELS[locale ?? 'enUS'];
}

const LEGACY_TABLE_ROWS = 4;
const LEGACY_TABLE_COLS = 6;
const LEGACY_TABLE_CELL_GAP = 3;
const LEGACY_TABLE_CELL_STEP = 23;
const LEGACY_LOGGER_MAX_SIZE = 100;
const LEGACY_LOGGER_INTERVAL = 600;

type LegacyMarkdownView = { md: boolean; html: boolean };
type LegacyHeaderTag = `h${1 | 2 | 3 | 4 | 5 | 6}`;
type LegacySelection = { start: number; end: number; selected: string };
type LegacySelectionRange = { start: number; end: number };
type LegacyShortcutKey = 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey';
type LegacySyncScrollSource = 'md' | 'html';

function legacyTableMarkdown(row: number, col: number) {
  const head = ['|'];
  const divider = ['|'];
  const data = ['|'];
  for (let index = 1; index <= col; index += 1) {
    head.push(' Head |');
    divider.push(' --- |');
    data.push(' Data |');
  }

  let rows = '';
  for (let index = 1; index <= row; index += 1) {
    rows += `\n${data.join('')}`;
  }

  return `${head.join('')}\n${divider.join('')}${rows}`;
}

function legacyListMarkdown(type: 'ordered' | 'unordered', selected: string) {
  let text = selected;
  if (text.substr(0, 1) !== '\n') {
    text = `\n${text}`;
  }
  if (type === 'unordered') {
    return text.length > 1 ? text.replace(/\n/g, '\n* ').trim() : '* ';
  }

  let index = 1;
  return text.length > 1 ? text.replace(/\n/g, () => `\n${index++}. `).trim() : '1. ';
}

function LegacyMarkdownEditor({
  value,
  onChange,
}: {
  value?: unknown;
  onChange: (value: string) => void;
}) {
  const text = normalizeLegacyMarkdownValue(value);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const htmlWrapperRef = useRef<HTMLDivElement | null>(null);
  const imageInputRef = useRef<HTMLInputElement | null>(null);
  const composingRef = useRef(false);
  const shouldSyncScrollRef = useRef<LegacySyncScrollSource>('md');
  const hasContentChangedRef = useRef(true);
  const isSyncingScrollRef = useRef(false);
  const scrollScaleRef = useRef(1);
  const loggerInitRef = useRef(text);
  const loggerTimerRef = useRef<number | null>(null);
  const undoStackRef = useRef<string[]>([]);
  const redoStackRef = useRef<string[]>([]);
  const lastPopRef = useRef<string | null>(null);
  const [view, setView] = useState<LegacyMarkdownView>({ md: true, html: true });
  const [fullScreen, setFullScreen] = useState(false);
  const [headerMenuVisible, setHeaderMenuVisible] = useState(false);
  const [tableMenuVisible, setTableMenuVisible] = useState(false);
  const [tableHover, setTableHover] = useState<{ row: number; col: number } | null>(null);
  const [, setUndoStack] = useState<string[]>([]);
  const [redoStack, setRedoStack] = useState<string[]>([]);
  const html = useMemo(() => renderLegacyAdminMarkdown(text), [text]);
  const labels = useMemo(() => getLegacyMarkdownLabels(), []);

  useEffect(
    () => () => {
      if (loggerTimerRef.current) {
        window.clearTimeout(loggerTimerRef.current);
      }
    },
    [],
  );

  useEffect(() => {
    hasContentChangedRef.current = true;
  }, [text]);

  const nextViewInfo = () => {
    if (view.md && view.html) {
      return { view: { md: true, html: false }, icon: 'keyboard', title: labels.btnModeEditor };
    }
    if (view.md) {
      return { view: { md: false, html: true }, icon: 'visibility', title: labels.btnModePreview };
    }
    return {
      view: { md: true, html: true },
      icon: 'view-split',
      title: labels.btnModeAll,
    };
  };

  const getSelection = (): LegacySelection => {
    const textarea = textareaRef.current;
    const start = textarea?.selectionStart ?? text.length;
    const end = textarea?.selectionEnd ?? text.length;
    return { start, end, selected: text.slice(start, end) };
  };

  const syncLoggerStacks = (nextUndoStack: string[], nextRedoStack = redoStackRef.current) => {
    undoStackRef.current = nextUndoStack;
    redoStackRef.current = nextRedoStack;
    setUndoStack(nextUndoStack);
    setRedoStack(nextRedoStack);
  };

  const pushLoggerRecord = (nextText: string) => {
    const nextStack = [...undoStackRef.current, nextText].slice(-LEGACY_LOGGER_MAX_SIZE);
    undoStackRef.current = nextStack;
    setUndoStack(nextStack);
  };

  const pauseLogger = () => {
    if (!loggerTimerRef.current) return;
    window.clearTimeout(loggerTimerRef.current);
    loggerTimerRef.current = null;
  };

  const recordLoggerChange = (nextText: string, immediate = false) => {
    if (undoStackRef.current[undoStackRef.current.length - 1] === nextText) return;
    if (lastPopRef.current !== null && lastPopRef.current === nextText) return;

    redoStackRef.current = [];
    setRedoStack([]);

    if (immediate) {
      pushLoggerRecord(nextText);
      lastPopRef.current = null;
      return;
    }

    pauseLogger();
    loggerTimerRef.current = window.setTimeout(() => {
      if (undoStackRef.current[undoStackRef.current.length - 1] !== nextText) {
        pushLoggerRecord(nextText);
      }
      lastPopRef.current = null;
      loggerTimerRef.current = null;
    }, LEGACY_LOGGER_INTERVAL);
  };

  const applyTextChange = (nextText: string, immediate = true) => {
    if (nextText === text) return;
    const normalizedNextText = normalizeLegacyMarkdownValue(nextText);
    hasContentChangedRef.current = true;
    recordLoggerChange(normalizedNextText, immediate);
    onChange(normalizedNextText);
  };

  const handleSyncScroll = (source: LegacySyncScrollSource) => {
    if (source !== shouldSyncScrollRef.current) return;
    const textarea = textareaRef.current;
    const htmlWrapper = htmlWrapperRef.current;
    if (!textarea || !htmlWrapper) return;

    if (hasContentChangedRef.current) {
      scrollScaleRef.current = textarea.scrollHeight / htmlWrapper.scrollHeight;
      hasContentChangedRef.current = false;
    }
    if (isSyncingScrollRef.current) return;

    isSyncingScrollRef.current = true;
    requestAnimationFrame(() => {
      const nextTextarea = textareaRef.current;
      const nextHtmlWrapper = htmlWrapperRef.current;
      if (nextTextarea && nextHtmlWrapper) {
        if (source === 'md') {
          nextHtmlWrapper.scrollTop = nextTextarea.scrollTop / scrollScaleRef.current;
        } else {
          nextTextarea.scrollTop = nextHtmlWrapper.scrollTop * scrollScaleRef.current;
        }
      }
      isSyncingScrollRef.current = false;
    });
  };

  const replaceSelection = (
    replacement: string,
    selection = getSelection(),
    nextSelection?: LegacySelectionRange,
  ) => {
    applyTextChange(`${text.slice(0, selection.start)}${replacement}${text.slice(selection.end)}`);
    if (nextSelection) {
      restoreSelection(selection.start + nextSelection.start, selection.start + nextSelection.end);
    }
  };

  const restoreSelection = (start: number, end = start) => {
    window.setTimeout(() => {
      const textarea = textareaRef.current;
      if (!textarea) return;
      textarea.setSelectionRange(start, end, 'forward');
      textarea.focus();
    });
  };

  const matchesLegacyShortcut = (
    event: KeyboardEvent<HTMLTextAreaElement>,
    key: string,
    keyCode: number,
    withKeys: LegacyShortcutKey[],
    aliasCommand = false,
  ) => {
    const modifierState = {
      ctrlKey: event.ctrlKey || (aliasCommand && event.metaKey),
      metaKey: event.metaKey,
      altKey: event.altKey,
      shiftKey: event.shiftKey,
    };

    if (withKeys.length > 0) {
      for (const withKey of withKeys) {
        if (!modifierState[withKey]) return false;
      }
    } else if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return false;
    }

    return event.key ? event.key === key : event.keyCode === keyCode;
  };

  const wrapSelection = (before: string, after = before) => {
    const selection = getSelection();
    replaceSelection(`${before}${selection.selected}${after}`, selection, {
      start: before.length,
      end: before.length + selection.selected.length,
    });
  };

  const insertMarkdownBlock = (before: string, after = '') => {
    const selection = getSelection();
    replaceSelection(`${before}${selection.selected}${after}`, selection, {
      start: before.length,
      end: before.length + selection.selected.length,
    });
  };

  const insertLegacyNewBlock = (
    markdown: string,
    selection = getSelection(),
    nextSelection?: LegacySelectionRange,
  ) => {
    const lines = text.split('\n');
    const beforeLines = text.substr(0, selection.start).split('\n');
    const col = beforeLines[beforeLines.length - 1]?.length ?? 0;
    const curLine = lines[beforeLines.length - 1] ?? '';
    let replacement = markdown;
    let selectionOffset = nextSelection;

    if (col > 0 && curLine.length > 0) {
      replacement = `\n${replacement}`;
      if (selectionOffset) {
        selectionOffset = {
          start: selectionOffset.start + 1,
          end: selectionOffset.end + 1,
        };
      }
    }

    const afterText = text.substr(selection.end);
    if (afterText.trim() !== '' && afterText.substr(0, 2) !== '\n\n') {
      if (afterText.substr(0, 1) !== '\n') {
        replacement += '\n';
      }
      replacement += '\n';
    }

    replaceSelection(replacement, selection, selectionOffset);
    if (!selectionOffset) {
      restoreSelection(selection.start);
    }
  };

  const insertHeader = (tag: LegacyHeaderTag) => {
    insertMarkdownBlock(`\n${'#'.repeat(Number(tag.slice(1)))} `, '\n');
    setHeaderMenuVisible(false);
  };

  const insertTable = (row: number, col: number) => {
    insertLegacyNewBlock(legacyTableMarkdown(row, col));
    setTableMenuVisible(false);
    setTableHover(null);
  };

  const insertImage = (label = '') => {
    const selection = getSelection();
    replaceSelection(`![${selection.selected || label}]()`, selection, {
      start: 2,
      end: selection.selected.length + 2,
    });
  };

  const clearMarkdown = () => {
    if (text !== '' && window.confirm && typeof window.confirm === 'function') {
      const confirmed = window.confirm(labels.clearTip);
      if (confirmed) applyTextChange('');
    }
  };

  const handleEditorKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((event.keyCode === 13 || event.key === 'Enter') && !composingRef.current) {
      const textarea = event.currentTarget;
      const cursor = textarea.selectionStart;
      const value = textarea.value;
      const lines = value.split('\n');
      const beforeLines = value.substr(0, cursor).split('\n');
      const curLine = lines[beforeLines.length - 1] ?? '';
      const removeCurrentListPrefix = () => {
        const lineStart = cursor - curLine.length;
        applyTextChange(`${value.substr(0, lineStart)}${value.substr(cursor)}`);
        restoreSelection(lineStart);
        event.preventDefault();
      };
      const insertNextListPrefix = (prefix: string) => {
        const insertion = `\n${prefix}`;
        applyTextChange(`${value.slice(0, cursor)}${insertion}${value.slice(cursor)}`);
        restoreSelection(cursor + prefix.length + 1);
        event.preventDefault();
      };
      const unordered = curLine.match(/^(\s*?)\* /);
      if (unordered) {
        if (/^(\s*?)\* $/.test(curLine)) {
          removeCurrentListPrefix();
        } else {
          insertNextListPrefix(unordered[0]);
        }
        return;
      }

      const ordered = curLine.match(/^(\s*?)(\d+)\. /);
      if (ordered) {
        if (/^(\s*?)(\d+)\. $/.test(curLine)) {
          removeCurrentListPrefix();
        } else {
          insertNextListPrefix(`${ordered[1]}${Number.parseInt(ordered[2]!, 10) + 1}. `);
        }
        return;
      }
    }

    const selection = getSelection();
    const applyShortcut = (callback: () => void) => {
      event.preventDefault();
      callback();
    };

    if (matchesLegacyShortcut(event, 'b', 66, ['ctrlKey'], true)) {
      applyShortcut(() => wrapSelection('**'));
      return;
    }
    if (matchesLegacyShortcut(event, 'i', 73, ['ctrlKey'], true)) {
      applyShortcut(() => wrapSelection('*'));
      return;
    }
    if (matchesLegacyShortcut(event, 'u', 85, ['ctrlKey'])) {
      applyShortcut(() => wrapSelection('++'));
      return;
    }
    if (matchesLegacyShortcut(event, 'd', 68, ['ctrlKey'], true)) {
      applyShortcut(() => wrapSelection('~~'));
      return;
    }
    if (matchesLegacyShortcut(event, '8', 56, ['ctrlKey', 'shiftKey'], true)) {
      const markdown = legacyListMarkdown('unordered', selection.selected);
      applyShortcut(() => {
        insertLegacyNewBlock(markdown, selection, {
          start: markdown.length,
          end: markdown.length,
        });
      });
      return;
    }
    if (matchesLegacyShortcut(event, '7', 55, ['ctrlKey', 'shiftKey'], true)) {
      const markdown = legacyListMarkdown('ordered', selection.selected);
      applyShortcut(() => {
        insertLegacyNewBlock(markdown, selection, {
          start: markdown.length,
          end: markdown.length,
        });
      });
      return;
    }
    if (matchesLegacyShortcut(event, 'k', 75, ['ctrlKey'], true)) {
      applyShortcut(() =>
        replaceSelection(`[${selection.selected}]()`, selection, {
          start: 1,
          end: selection.selected.length + 1,
        }),
      );
      return;
    }
    if (matchesLegacyShortcut(event, 'y', 89, ['ctrlKey'])) {
      applyShortcut(redoMarkdown);
      return;
    }
    if (matchesLegacyShortcut(event, 'z', 90, ['metaKey', 'shiftKey'])) {
      applyShortcut(redoMarkdown);
      return;
    }
    if (matchesLegacyShortcut(event, 'z', 90, ['ctrlKey'], true)) {
      applyShortcut(undoMarkdown);
    }
  };

  const bindImageInput = (node: HTMLInputElement | null) => {
    imageInputRef.current = node;
    if (!node) return;
    node.setAttribute('type', 'file');
    node.setAttribute('accept', '');
    node.setAttribute(
      'style',
      'position: absolute; z-index: -1; left: 0px; top: 0px; width: 0px; height: 0px; opacity: 0;',
    );
  };

  const undoMarkdown = () => {
    pauseLogger();
    const nextUndoStack = [...undoStackRef.current];
    const nextRedoStack = [...redoStackRef.current];
    const popped = nextUndoStack.pop();
    let previous: string;

    if (popped === undefined) {
      previous = loggerInitRef.current;
    } else if (popped !== text) {
      nextRedoStack.push(popped);
      previous = popped;
    } else {
      const next = nextUndoStack.pop();
      nextRedoStack.push(popped);
      previous = next === undefined ? loggerInitRef.current : next;
    }

    lastPopRef.current = previous;
    syncLoggerStacks(nextUndoStack, nextRedoStack);
    onChange(previous);
  };

  const redoMarkdown = () => {
    const nextRedoStack = [...redoStackRef.current];
    const next = nextRedoStack.pop();
    if (next === undefined) return;

    lastPopRef.current = next;
    syncLoggerStacks(undoStackRef.current, nextRedoStack);
    pushLoggerRecord(next);
    onChange(next);
  };

  const mode = nextViewInfo();

  return (
    <div className={`rc-md-editor ${fullScreen ? 'full' : ''} `} style={{ height: 500 }}>
      <div className="rc-md-navigation visible">
        <div className="navigation-nav left">
          <div className="button-wrap">
            <span
              className="button button-type-header"
              title={labels.btnHeader}
              onMouseEnter={() => setHeaderMenuVisible(true)}
              onMouseLeave={() => setHeaderMenuVisible(false)}
            >
              <i className="rmel-iconfont rmel-icon-font-size" />
              <div
                className={`drop-wrap ${headerMenuVisible ? 'show' : 'hidden'}`}
                onClick={(event) => {
                  event.stopPropagation();
                  setHeaderMenuVisible(false);
                }}
              >
                <ul className="header-list">
                  <li className="list-item">
                    <h1 onClick={() => insertHeader('h1')}>H1</h1>
                  </li>
                  <li className="list-item">
                    <h2 onClick={() => insertHeader('h2')}>H2</h2>
                  </li>
                  <li className="list-item">
                    <h3 onClick={() => insertHeader('h3')}>H3</h3>
                  </li>
                  <li className="list-item">
                    <h4 onClick={() => insertHeader('h4')}>H4</h4>
                  </li>
                  <li className="list-item">
                    <h5 onClick={() => insertHeader('h5')}>H5</h5>
                  </li>
                  <li className="list-item">
                    <h6 onClick={() => insertHeader('h6')}>H6</h6>
                  </li>
                </ul>
              </div>
            </span>
            <span
              className="button button-type-bold"
              title={labels.btnBold}
              onClick={() => wrapSelection('**')}
            >
              <i className="rmel-iconfont rmel-icon-bold" />
            </span>
            <span
              className="button button-type-italic"
              title={labels.btnItalic}
              onClick={() => wrapSelection('*')}
            >
              <i className="rmel-iconfont rmel-icon-italic" />
            </span>
            <span
              className="button button-type-underline"
              title={labels.btnUnderline}
              onClick={() => wrapSelection('++')}
            >
              <i className="rmel-iconfont rmel-icon-underline" />
            </span>
            <span
              className="button button-type-strikethrough"
              title={labels.btnStrikethrough}
              onClick={() => wrapSelection('~~')}
            >
              <i className="rmel-iconfont rmel-icon-strikethrough" />
            </span>
            <span
              className="button button-type-unordered"
              title={labels.btnUnordered}
              onClick={() => {
                const selection = getSelection();
                const markdown = legacyListMarkdown('unordered', selection.selected);
                insertLegacyNewBlock(markdown, selection, {
                  start: markdown.length,
                  end: markdown.length,
                });
              }}
            >
              <i className="rmel-iconfont rmel-icon-list-unordered" />
            </span>
            <span
              className="button button-type-ordered"
              title={labels.btnOrdered}
              onClick={() => {
                const selection = getSelection();
                const markdown = legacyListMarkdown('ordered', selection.selected);
                insertLegacyNewBlock(markdown, selection, {
                  start: markdown.length,
                  end: markdown.length,
                });
              }}
            >
              <i className="rmel-iconfont rmel-icon-list-ordered" />
            </span>
            <span
              className="button button-type-quote"
              title={labels.btnQuote}
              onClick={() => insertMarkdownBlock('\n> ', '\n')}
            >
              <i className="rmel-iconfont rmel-icon-quote" />
            </span>
            <span
              className="button button-type-wrap"
              title={labels.btnLineBreak}
              onClick={() => insertLegacyNewBlock('---', getSelection(), { start: 3, end: 3 })}
            >
              <i className="rmel-iconfont rmel-icon-wrap" />
            </span>
            <span
              className="button button-type-code-inline"
              title={labels.btnInlineCode}
              onClick={() => wrapSelection('`')}
            >
              <i className="rmel-iconfont rmel-icon-code" />
            </span>
            <span
              className="button button-type-code-block"
              title={labels.btnCode}
              onClick={() => insertMarkdownBlock('\n```\n', '\n```\n')}
            >
              <i className="rmel-iconfont rmel-icon-code-block" />
            </span>
            <span
              className="button button-type-table"
              title={labels.btnTable}
              onMouseEnter={() => setTableMenuVisible(true)}
              onMouseLeave={() => {
                setTableMenuVisible(false);
                setTableHover(null);
              }}
            >
              <i className="rmel-iconfont rmel-icon-grid" />
              <div
                className={`drop-wrap ${tableMenuVisible ? 'show' : 'hidden'}`}
                onClick={(event) => {
                  event.stopPropagation();
                  setTableMenuVisible(false);
                  setTableHover(null);
                }}
              >
                <ul
                  className="table-list wrap"
                  style={{
                    width: LEGACY_TABLE_CELL_STEP * LEGACY_TABLE_COLS - LEGACY_TABLE_CELL_GAP,
                    height: LEGACY_TABLE_CELL_STEP * LEGACY_TABLE_ROWS - LEGACY_TABLE_CELL_GAP,
                  }}
                >
                  {Array.from({ length: LEGACY_TABLE_ROWS }).map((_, row) =>
                    Array.from({ length: LEGACY_TABLE_COLS }).map((__, col) => (
                      <li
                        key={`${row}-${col}`}
                        className={`list-item ${
                          tableHover && row <= tableHover.row && col <= tableHover.col
                            ? 'active'
                            : ''
                        }`}
                        style={{
                          top: LEGACY_TABLE_CELL_STEP * row,
                          left: LEGACY_TABLE_CELL_STEP * col,
                        }}
                        onMouseOver={() => setTableHover({ row, col })}
                        onClick={(event) => {
                          event.stopPropagation();
                          insertTable(row + 1, col + 1);
                        }}
                      />
                    )),
                  )}
                </ul>
              </div>
            </span>
            <span
              className="button button-type-image"
              title={labels.btnImage}
              style={{ position: 'relative' }}
              onClick={() => insertImage()}
            >
              <i className="rmel-iconfont rmel-icon-image" />
              <input
                ref={bindImageInput}
                onChange={(event) => {
                  const file = event.currentTarget.files?.[0];
                  if (file) insertImage(file.name);
                  event.currentTarget.value = '';
                }}
              />
            </span>
            <span
              className="button button-type-link"
              title={labels.btnLink}
              onClick={() => {
                const selection = getSelection();
                replaceSelection(`[${selection.selected}]()`, selection, {
                  start: 1,
                  end: selection.selected.length + 1,
                });
              }}
            >
              <i className="rmel-iconfont rmel-icon-link" />
            </span>
            <span
              className="button button-type-clear"
              title={labels.btnClear}
              onClick={clearMarkdown}
            >
              <i className="rmel-iconfont rmel-icon-delete" />
            </span>
            <span
              className={`button button-type-undo ${
                loggerInitRef.current !== text ? '' : 'disabled'
              }`}
              title={labels.btnUndo}
              onClick={undoMarkdown}
            >
              <i className="rmel-iconfont rmel-icon-undo" />
            </span>
            <span
              className={`button button-type-redo ${redoStack.length ? '' : 'disabled'}`}
              title={labels.btnRedo}
              onClick={redoMarkdown}
            >
              <i className="rmel-iconfont rmel-icon-redo" />
            </span>
          </div>
        </div>
        <div className="navigation-nav right">
          <div className="button-wrap">
            <span
              className="button button-type-mode"
              title={mode.title}
              onClick={() => setView(mode.view)}
            >
              <i className={`rmel-iconfont rmel-icon-${mode.icon}`} />
            </span>
            <span
              className="button button-type-fullscreen"
              title={fullScreen ? labels.btnExitFullScreen : labels.btnFullScreen}
              onClick={() => setFullScreen((current) => !current)}
            >
              <i
                className={`rmel-iconfont rmel-icon-${
                  fullScreen ? 'fullscreen-exit' : 'fullscreen'
                }`}
              />
            </span>
          </div>
        </div>
      </div>
      <div className="editor-container">
        <section className={`section sec-md ${view.md ? 'visible' : 'in-visible'}`}>
          <textarea
            ref={textareaRef}
            name="textarea"
            value={text}
            className="section-container input "
            wrap="hard"
            onChange={(event) => applyTextChange(event.target.value, false)}
            onScroll={() => handleSyncScroll('md')}
            onMouseOver={() => {
              shouldSyncScrollRef.current = 'md';
            }}
            onKeyDown={handleEditorKeyDown}
            onCompositionStart={() => {
              composingRef.current = true;
            }}
            onCompositionEnd={() => {
              composingRef.current = false;
            }}
          />
        </section>
        <section className={`section sec-html ${view.html ? 'visible' : 'in-visible'}`}>
          <div
            ref={htmlWrapperRef}
            className="section-container html-wrap"
            onMouseOver={() => {
              shouldSyncScrollRef.current = 'html';
            }}
            onScroll={() => handleSyncScroll('html')}
          >
            <div className="custom-html-style" dangerouslySetInnerHTML={{ __html: html }} />
          </div>
        </section>
      </div>
    </div>
  );
}

function KnowledgeEditor({
  id,
  children,
  onSave,
  onSaved,
  saveLoading,
}: {
  id?: number;
  children: ReactElement<{ onClick?: () => void }>;
  onSave: (payload: SaveKnowledgePayload) => Promise<unknown>;
  onSaved: () => void | Promise<unknown>;
  saveLoading?: boolean;
}) {
  const { message } = App.useApp();
  const [visible, setVisible] = useState(false);
  const [loading, setLoading] = useState(false);
  const [knowledge, setKnowledge] = useState<Partial<Knowledge>>({});
  const [editorKey, setEditorKey] = useState(Math.random());

  const show = async () => {
    setVisible(true);
    setEditorKey(Math.random());
    if (!id) {
      setKnowledge({});
      return;
    }

    setLoading(true);
    try {
      setKnowledge(await admin.knowledgeDetail(apiClient, id));
    } finally {
      setLoading(false);
    }
  };

  const hide = () => {
    setVisible(false);
    setKnowledge({});
  };

  const formChange = (key: keyof Knowledge, value: unknown) => {
    setKnowledge((current) => ({ ...current, [key]: value }));
  };

  const save = async () => {
    await onSave({ ...knowledge });
    await onSaved();
    message.success('保存成功');
  };

  return (
    <>
      {cloneElement(children, { onClick: show })}
      <LegacyDrawer
        width="80%"
        id="knowledge"
        open={visible}
        title={id ? '编辑知识' : '新增知识'}
        onClose={hide}
      >
        {loading ? (
          <LegacyLoadingIcon />
        ) : (
          <div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">标题</label>
              <LegacyInput
                className="ant-input"
                placeholder="请输入知识标题"
                value={knowledge.title}
                onChange={(event) => formChange('title', event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">分类</label>
              <LegacyInput
                className="ant-input"
                placeholder="请输入分类，分类将会自动归集"
                value={knowledge.category}
                onChange={(event) => formChange('category', event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">语言</label>
              <LegacySelect
                placeholder="请选择知识语言"
                defaultValue={knowledge.language || 1}
                style={{ width: '100%' }}
                value={knowledge.language}
                options={LEGACY_KNOWLEDGE_LOCALE_OPTIONS}
                onChange={(value) => formChange('language', value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">内容</label>
              <LegacyMarkdownEditor
                key={editorKey}
                value={knowledge.body}
                onChange={(value) => formChange('body', value)}
              />
            </div>
          </div>
        )}
        <div className="v2board-drawer-action">
          <LegacyButton className="ant-btn" style={{ marginRight: 8 }} onClick={hide}>
            取消
          </LegacyButton>
          <LegacyButton
            className={`ant-btn ant-btn-primary${saveLoading ? ' ant-btn-loading' : ''}`}
            onClick={() => void save()}
          >
            {saveLoading ? <LegacyLoadingIcon /> : null}
            提交
          </LegacyButton>
        </div>
      </LegacyDrawer>
    </>
  );
}

export default function KnowledgePage() {
  const list = useAdminKnowledge();
  useAdminKnowledgeCategories();
  const save = useSaveKnowledgeMutation();
  const drop = useDropKnowledgeMutation();
  const show = useShowKnowledgeMutation();
  const sort = useSortKnowledgeMutation();
  const [orderedKnowledge, setOrderedKnowledge] = useState<KnowledgeSummary[]>(
    () => list.data ?? [],
  );
  const [sortingLoading, setSortingLoading] = useState(false);
  const orderRef = useRef(orderedKnowledge);

  useEffect(() => {
    if (list.data) {
      setOrderedKnowledge(list.data);
      setSortingLoading(false);
    }
  }, [list.data]);

  orderRef.current = orderedKnowledge;

  const sortKnowledge = (fromIndex: number, toIndex: number) => {
    const next = [...orderRef.current];
    const moved = next[fromIndex];
    if (!moved) return;
    if (fromIndex < toIndex) {
      next.splice(toIndex + 1, 0, moved);
      next.splice(fromIndex, 1);
    } else {
      next.splice(toIndex, 0, moved);
      next.splice(fromIndex + 1, 1);
    }
    setSortingLoading(true);
    setOrderedKnowledge(next);
    sort.mutate(
      next.map((knowledge) => knowledge.id),
      {
        onSuccess: () => {
          void list.refetch();
        },
      },
    );
  };

  const saveKnowledge = (payload: SaveKnowledgePayload) => save.mutateAsync(payload);
  const refetchKnowledge = () => list.refetch();

  const headers: LegacyStandaloneTableHeader[] = [
    { title: '排序' },
    { title: '文章ID' },
    { title: '显示' },
    { title: '标题' },
    { title: '分类' },
    { title: '更新时间', alignRight: true },
    { title: '操作', alignRight: true, fixedRight: true },
  ];

  const renderKnowledgeShowSwitch = (value: 0 | 1 | undefined, row: KnowledgeSummary) => (
    <LegacySwitch
      size="small"
      onChange={() =>
        show.mutate(row.id, {
          onSuccess: () => {
            void list.refetch();
          },
        })
      }
      checked={value as unknown as boolean}
    />
  );

  const renderKnowledgeActions = (row: KnowledgeSummary) => (
    <>
      <KnowledgeEditor
        id={row.id}
        onSave={saveKnowledge}
        onSaved={refetchKnowledge}
        saveLoading={save.isPending}
      >
        <a ref={legacyHref()}>编辑</a>
      </KnowledgeEditor>
      <LegacyDivider type="vertical" />
      <a
        ref={legacyHref()}
        onClick={() => {
          void legacyConfirm({
            title: '警告',
            content: '确定要删除该条项目吗？',
            onOk: () => {
              void drop.mutateAsync(row.id).then(() => {
                void list.refetch();
              });
            },
            okText: '确定',
            cancelText: '取消',
          });
        }}
      >
        删除
      </a>
    </>
  );

  return (
    <LegacySpin loading={list.isFetching || sortingLoading}>
      <div className="block border-bottom">
        <div className="bg-white">
          <div style={{ padding: 15 }}>
            <KnowledgeEditor
              onSave={saveKnowledge}
              onSaved={refetchKnowledge}
              saveLoading={save.isPending}
            >
              <LegacyButton className="ant-btn">
                <LegacyPlusIcon />
                <span>新增</span>
              </LegacyButton>
            </KnowledgeEditor>
          </div>
          <LegacyDragSort
            onDragEnd={(fromIndex, toIndex) => sortKnowledge(fromIndex, toIndex)}
            nodeSelector="tr"
            handleSelector="i"
          >
            <LegacyStandaloneTable
              headers={headers}
              isEmpty={orderedKnowledge.length === 0}
              scrollX={750}
              fixedRightChildren={orderedKnowledge.map((row, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  style={{ height: 54 }}
                  {...legacyTableRowKey(index)}
                >
                  <td
                    className="ant-table-align-right ant-table-row-cell-last"
                    style={{ textAlign: 'right' }}
                  >
                    {renderKnowledgeActions(row)}
                  </td>
                </tr>
              ))}
            >
              {orderedKnowledge.map((row, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  {...legacyTableRowKey(index)}
                >
                  <td className="">
                    <LegacyMenuIcon style={{ cursor: 'move' }} />
                  </td>
                  <td className="">{row.id}</td>
                  <td className="">{renderKnowledgeShowSwitch(row.show, row)}</td>
                  <td className="">{row.title}</td>
                  <td className="">{row.category}</td>
                  <td className="ant-table-align-right" style={{ textAlign: 'right' }}>
                    {dayjs(1000 * row.updated_at).format('YYYY/MM/DD HH:mm')}
                  </td>
                  <td
                    className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"
                    style={{ textAlign: 'right' }}
                  >
                    {renderKnowledgeActions(row)}
                  </td>
                </tr>
              ))}
            </LegacyStandaloneTable>
          </LegacyDragSort>
        </div>
      </div>
    </LegacySpin>
  );
}
