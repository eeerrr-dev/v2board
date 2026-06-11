import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import dayjs from 'dayjs';
import { describe, expect, it, vi } from 'vitest';
import KnowledgePage from './knowledge';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'knowledge.tsx'), 'utf8');
const queriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);

vi.mock('@/lib/queries', () => ({
  useAdminKnowledge: () => ({
    isLoading: false,
    isFetching: false,
    data: [
      {
        id: 1,
        sort: 1,
        show: 1,
        title: '入门指南',
        category: '帮助',
        updated_at: 1700000000,
      },
    ],
    refetch: vi.fn(),
  }),
  useAdminKnowledgeCategories: () => ({
    data: ['帮助'],
  }),
  useSaveKnowledgeMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useDropKnowledgeMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useShowKnowledgeMutation: () => ({
    mutate: vi.fn(),
  }),
  useSortKnowledgeMutation: () => ({
    mutate: vi.fn(),
  }),
}));

describe('KnowledgePage legacy knowledge manager', () => {
  it('renders the original knowledge table shell and actions', () => {
    const html = renderToStaticMarkup(<KnowledgePage />);

    expect(html).toContain('class="block border-bottom"');
    expect(html).toContain('class="bg-white"');
    expect(html).toContain('新增');
    expect(html).toContain('class="ant-btn"');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('<span>新增</span>');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain(
      'class="ant-table ant-table-default ant-table-scroll-position-left ant-table-scroll-position-right"',
    );
    expect(html).toContain('tabindex="-1" class="ant-table-body" style="overflow-x:scroll"');
    expect(html).toContain('class="ant-table-fixed" style="width:750px"');
    expect(html).toContain('class="ant-table-fixed-right"');
    expect(html).toContain('class="ant-switch-small ant-switch ant-switch-checked"');
    expect(html).toContain('排序');
    expect(html).toContain('文章ID');
    expect(html).toContain('显示');
    expect(html).toContain('标题');
    expect(html).toContain('分类');
    expect(html).toContain('更新时间');
    expect(html).toContain('操作');
    expect(html).toContain('anticon-menu');
    expect(html).toContain('入门指南');
    expect(html).toContain('帮助');
    expect(html).toContain(dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm'));
    expect(html).toContain('编辑');
    expect(html).toContain('删除');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-table-cell');
    expect(html).not.toContain('css-dev-only');
    expect(html).not.toContain('ant-typography');
  });

  it('uses the legacy markdown editor structure for knowledge bodies', () => {
    expect(source).toContain('width="80%"');
    expect(source).toContain('id="knowledge"');
    expect(source).not.toContain('size="80%"');
    expect(source).toContain('new MarkdownIt({ html: true, linkify: true, typographer: true })');
    expect(source).toContain('function LegacyMarkdownEditor');
    expect(source).toContain("className={`rc-md-editor ${fullScreen ? 'full' : ''} `}");
    expect(source).toContain('className="rc-md-navigation visible"');
    expect(source).toContain("className={`drop-wrap ${headerMenuVisible ? 'show' : 'hidden'}`}");
    expect(source).toContain('className="header-list"');
    expect(source).toContain("<h1 onClick={() => insertHeader('h1')}>H1</h1>");
    expect(source).toContain("<h6 onClick={() => insertHeader('h6')}>H6</h6>");
    expect(source).toContain('rmel-icon-font-size');
    expect(source).not.toContain('rmel-icon-font"');
    expect(source).toContain('className="section-container input "');
    expect(source).toContain('className="section-container html-wrap"');
    expect(source).toContain('dangerouslySetInnerHTML={{ __html: html }}');
    expect(source).toContain('function normalizeLegacyMarkdownValue(value: unknown)');
    expect(source).toContain("if (typeof value === 'undefined') return '';");
    expect(source).toContain("typeof value === 'string' ? value : String(value).toString()");
    expect(source).toContain(".replace(/\\u21b5/g, '\\n')");
    expect(source).toContain('const text = normalizeLegacyMarkdownValue(value);');
    expect(source).toContain('value={knowledge.body}');
    expect(source).toContain('const [editorKey, setEditorKey] = useState(Math.random());');
    expect(source).toContain('setEditorKey(Math.random());');
    expect(source).not.toContain('<Input.TextArea');
    expect(source).not.toContain("value={knowledge.body ?? ''}");
  });

  it('uses legacy form controls in the knowledge drawer', () => {
    expect(source).toContain("import { LegacyInput } from '@/components/legacy-input';");
    expect(source).toContain("import { LegacySelect } from '@/components/legacy-select';");
    expect(source).toContain("import { LegacyDrawer } from '@/components/legacy-drawer';");
    expect(source).toContain(
      "import { LegacyLoadingIcon, LegacyPlusIcon } from '@/components/legacy-ant-icon';",
    );
    expect(source).toContain('<LegacyInput');
    expect(source).toContain('className="ant-input"');
    expect(source).toContain('value={knowledge.title}');
    expect(source).toContain('value={knowledge.category}');
    expect(source).not.toContain('key={`title-${editorKey}`}');
    expect(source).not.toContain('key={`category-${editorKey}`}');
    expect(source).toContain('<LegacySelect');
    expect(source).toContain('<LegacyDrawer');
    expect(source).toContain('<LegacyButton className="ant-btn" style={{ marginRight: 8 }}');
    expect(source).toContain(
      "className={`ant-btn ant-btn-primary${saveLoading ? ' ant-btn-loading' : ''}`}",
    );
    expect(source).toContain('{saveLoading ? <LegacyLoadingIcon /> : null}');
    expect(source).not.toContain('<Drawer');
    expect(source).not.toContain('<Button');
    expect(source).not.toContain(' Button,');
    expect(source).not.toContain(' Drawer,');
    expect(source).not.toContain('Input,');
    expect(source).not.toContain(" Input } from 'antd'");
    expect(source).not.toContain('<Input');
    expect(source).not.toContain('defaultValue={knowledge.title}');
    expect(source).not.toContain('defaultValue={knowledge.category}');
  });

  it('keeps the full bundled markdown navigation toolbar', () => {
    const toolbarStart = source.indexOf('<div className="navigation-nav left">');
    const toolbarEnd = source.indexOf('<div className="navigation-nav right">', toolbarStart);
    const toolbar = source.slice(toolbarStart, toolbarEnd);
    const orderedButtons = [
      'button-type-header',
      'button-type-bold',
      'button-type-italic',
      'button-type-underline',
      'button-type-strikethrough',
      'button-type-unordered',
      'button-type-ordered',
      'button-type-quote',
      'button-type-wrap',
      'button-type-code-inline',
      'button-type-code-block',
      'button-type-table',
      'button-type-image',
      'button-type-link',
      'button-type-clear',
      'button-type-undo',
      'button-type-redo',
    ];

    expect(toolbarStart).toBeGreaterThan(-1);
    expect(toolbarEnd).toBeGreaterThan(toolbarStart);
    for (const button of orderedButtons) {
      expect(toolbar).toContain(button);
    }
    for (let index = 1; index < orderedButtons.length; index += 1) {
      expect(toolbar.indexOf(orderedButtons[index - 1]!)).toBeLessThan(
        toolbar.indexOf(orderedButtons[index]!),
      );
    }

    expect(toolbar).toContain('title={labels.btnUnderline}');
    expect(toolbar).toContain('title={labels.btnStrikethrough}');
    expect(toolbar).toContain('title={labels.btnOrdered}');
    expect(toolbar).toContain('title={labels.btnLineBreak}');
    expect(toolbar).toContain('title={labels.btnInlineCode}');
    expect(toolbar).toContain('title={labels.btnTable}');
    expect(toolbar).toContain('title={labels.btnImage}');
    expect(toolbar).toContain('title={labels.btnClear}');
    expect(toolbar).toContain('title={labels.btnUndo}');
    expect(toolbar).toContain('title={labels.btnRedo}');
    expect(toolbar).toContain('rmel-icon-underline');
    expect(toolbar).toContain('rmel-icon-strikethrough');
    expect(toolbar).toContain('rmel-icon-list-ordered');
    expect(toolbar).toContain('rmel-icon-wrap');
    expect(toolbar).toContain('rmel-icon-code-block');
    expect(toolbar).toContain('rmel-icon-grid');
    expect(toolbar).toContain('rmel-icon-image');
    expect(toolbar).toContain('rmel-icon-delete');
    expect(toolbar).toContain('rmel-icon-undo');
    expect(toolbar).toContain('rmel-icon-redo');
    expect(toolbar).not.toContain('button-type-code"');
    expect(source).not.toContain('className="tool-bar"');
    expect(source).not.toContain('title="hidden menu"');
  });

  it('keeps the bundled markdown table, image, mode, and logger details', () => {
    expect(source).toContain('const LEGACY_TABLE_ROWS = 4;');
    expect(source).toContain('const LEGACY_TABLE_COLS = 6;');
    expect(source).toContain('const LEGACY_LOGGER_MAX_SIZE = 100;');
    expect(source).toContain('const LEGACY_LOGGER_INTERVAL = 600;');
    expect(source).toContain('const LEGACY_MARKDOWN_LABELS = {');
    expect(source).toContain("clearTip: 'Are you sure you want to clear all contents?'");
    expect(source).toContain("clearTip: '您确定要清空所有内容吗？'");
    expect(source).toContain('type LegacyMarkdownLocaleKey = keyof typeof LEGACY_MARKDOWN_LABELS;');
    expect(source).toContain('function normalizeLegacyMarkdownLocale(locale?: string)');
    expect(source).toContain("normalizeLegacyMarkdownLocale(browserNavigator.language)");
    expect(source).toContain("normalizeLegacyMarkdownLocale(browserNavigator.browserLanguage)");
    expect(source).toContain('return LEGACY_MARKDOWN_LABELS[locale ?? \'enUS\'];');
    expect(source).toContain('const labels = useMemo(() => getLegacyMarkdownLabels(), []);');
    expect(source).toContain('function legacyTableMarkdown(row: number, col: number)');
    expect(source).toContain("function legacyListMarkdown(type: 'ordered' | 'unordered'");
    expect(source).toContain('const insertLegacyNewBlock = (');
    expect(source).toContain('markdown: string,');
    expect(source).toContain('const composingRef = useRef(false);');
    expect(source).toContain("type LegacySyncScrollSource = 'md' | 'html';");
    expect(source).toContain("const shouldSyncScrollRef = useRef<LegacySyncScrollSource>('md');");
    expect(source).toContain('const hasContentChangedRef = useRef(true);');
    expect(source).toContain('const isSyncingScrollRef = useRef(false);');
    expect(source).toContain('const scrollScaleRef = useRef(1);');
    expect(source).toContain('const htmlWrapperRef = useRef<HTMLDivElement | null>(null);');
    expect(source).toContain('const loggerInitRef = useRef(text);');
    expect(source).toContain('const loggerTimerRef = useRef<number | null>(null);');
    expect(source).toContain('const undoStackRef = useRef<string[]>([]);');
    expect(source).toContain('const redoStackRef = useRef<string[]>([]);');
    expect(source).toContain('const lastPopRef = useRef<string | null>(null);');
    expect(source).toContain('const recordLoggerChange = (nextText: string, immediate = false) => {');
    expect(source).toContain('pushLoggerRecord(nextText);');
    expect(source).toContain('}, LEGACY_LOGGER_INTERVAL);');
    expect(source).toContain('const applyTextChange = (nextText: string, immediate = true) => {');
    expect(source).toContain('if (nextText === text) return;');
    expect(source).toContain('const normalizedNextText = normalizeLegacyMarkdownValue(nextText);');
    expect(source).toContain('hasContentChangedRef.current = true;');
    expect(source).toContain('recordLoggerChange(normalizedNextText, immediate);');
    expect(source).toContain('onChange(normalizedNextText);');
    expect(source).toContain('const handleSyncScroll = (source: LegacySyncScrollSource) => {');
    expect(source).toContain('if (source !== shouldSyncScrollRef.current) return;');
    expect(source).toContain(
      'scrollScaleRef.current = textarea.scrollHeight / htmlWrapper.scrollHeight;',
    );
    expect(source).toContain('requestAnimationFrame(() => {');
    expect(source).toContain(
      'nextHtmlWrapper.scrollTop = nextTextarea.scrollTop / scrollScaleRef.current;',
    );
    expect(source).toContain(
      'nextTextarea.scrollTop = nextHtmlWrapper.scrollTop * scrollScaleRef.current;',
    );
    expect(source).toContain('type LegacySelectionRange = { start: number; end: number };');
    expect(source).toContain('nextSelection?: LegacySelectionRange');
    expect(source).toContain(
      'restoreSelection(selection.start + nextSelection.start, selection.start + nextSelection.end);',
    );
    expect(source).toContain('start: before.length');
    expect(source).toContain('end: before.length + selection.selected.length');
    expect(source).toContain('start: selectionOffset.start + 1');
    expect(source).toContain('restoreSelection(selection.start);');
    expect(source).toContain('end: selection.selected.length + 2');
    expect(source).toContain('end: selection.selected.length + 1');
    expect(source).toContain('const clearMarkdown = () => {');
    expect(source).toContain('window.confirm(labels.clearTip)');
    expect(source).toContain(
      "type LegacyShortcutKey = 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey';",
    );
    expect(source).toContain('const matchesLegacyShortcut = (');
    expect(source).toContain('ctrlKey: event.ctrlKey || (aliasCommand && event.metaKey)');
    expect(source).toContain('const handleEditorKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {');
    expect(source).toContain("event.keyCode === 13 || event.key === 'Enter'");
    expect(source).toContain('curLine.match(/^(\\s*?)\\* /)');
    expect(source).toContain('insertNextListPrefix(unordered[0]);');
    expect(source).toContain('curLine.match(/^(\\s*?)(\\d+)\\. /)');
    expect(source).toContain(
      'insertNextListPrefix(`${ordered[1]}${Number.parseInt(ordered[2]!, 10) + 1}. `);',
    );
    expect(source).toContain('removeCurrentListPrefix();');
    expect(source).toContain("matchesLegacyShortcut(event, 'b', 66, ['ctrlKey'], true)");
    expect(source).toContain("matchesLegacyShortcut(event, 'i', 73, ['ctrlKey'], true)");
    expect(source).toContain("matchesLegacyShortcut(event, 'u', 85, ['ctrlKey'])");
    expect(source).toContain("matchesLegacyShortcut(event, 'd', 68, ['ctrlKey'], true)");
    expect(source).toContain("matchesLegacyShortcut(event, '8', 56, ['ctrlKey', 'shiftKey'], true)");
    expect(source).toContain("matchesLegacyShortcut(event, '7', 55, ['ctrlKey', 'shiftKey'], true)");
    expect(source).toContain("matchesLegacyShortcut(event, 'k', 75, ['ctrlKey'], true)");
    expect(source).toContain("matchesLegacyShortcut(event, 'y', 89, ['ctrlKey'])");
    expect(source).toContain("matchesLegacyShortcut(event, 'z', 90, ['metaKey', 'shiftKey'])");
    expect(source).toContain("matchesLegacyShortcut(event, 'z', 90, ['ctrlKey'], true)");
    expect(source).toContain('applyShortcut(redoMarkdown);');
    expect(source).toContain('applyShortcut(undoMarkdown);');
    expect(source).toContain('onKeyDown={handleEditorKeyDown}');
    expect(source).toContain('onChange={(event) => applyTextChange(event.target.value, false)}');
    expect(source).toContain("onScroll={() => handleSyncScroll('md')}");
    expect(source).toContain("shouldSyncScrollRef.current = 'md';");
    expect(source).toContain('ref={htmlWrapperRef}');
    expect(source).toContain("shouldSyncScrollRef.current = 'html';");
    expect(source).toContain("onScroll={() => handleSyncScroll('html')}");
    expect(source).toContain('loggerInitRef.current !== text ?');
    expect(source).not.toContain('undoStack.length > 1 || loggerInitRef.current !== text ?');
    expect(source).toContain('onCompositionStart={() => {');
    expect(source).toContain('composingRef.current = true;');
    expect(source).toContain('composingRef.current = false;');
    expect(source).toContain('onClick={clearMarkdown}');
    expect(source).not.toContain("onClick={() => applyTextChange('')}");
    expect(source).toContain("onClick={() => insertLegacyNewBlock('---', getSelection(), { start: 3, end: 3 })}");
    expect(source).not.toContain("onClick={() => replaceSelection('\\n')}");
    expect(source).toContain('insertLegacyNewBlock(legacyTableMarkdown(row, col));');
    expect(source).toContain('setTableHover(null);');
    const tableButtonStart = source.indexOf('button-type-table');
    const tableButtonEnd = source.indexOf('button-type-image', tableButtonStart);
    const tableButton = source.slice(tableButtonStart, tableButtonEnd);
    expect(tableButton).toContain("className={`drop-wrap ${tableMenuVisible ? 'show' : 'hidden'}`}");
    expect(tableButton).toContain('onClick={(event) => {');
    expect(tableButton).toContain('event.stopPropagation();');
    expect(tableButton).toContain('setTableMenuVisible(false);');
    const tableDropWrap = tableButton.slice(
      tableButton.indexOf("className={`drop-wrap ${tableMenuVisible ? 'show' : 'hidden'}`}"),
      tableButton.indexOf('<ul', tableButton.indexOf('className={`drop-wrap')),
    );
    expect(tableDropWrap).toContain('setTableHover(null);');
    expect(source).toContain('className="table-list wrap"');
    expect(source).toContain('key={`${row}-${col}`}');
    expect(source).toContain('onMouseOver={() => setTableHover({ row, col })}');
    expect(source).toContain('insertTable(row + 1, col + 1);');
    expect(source).toContain("style={{ position: 'relative' }}");
    expect(source).toContain("node.setAttribute('type', 'file');");
    expect(source).toContain("node.setAttribute('accept', '');");
    expect(source).toContain('position: absolute; z-index: -1; left: 0px; top: 0px;');
    expect(source).not.toContain('width: LEGACY_TABLE_CELL_SIZE');
    expect(source).not.toContain('height: LEGACY_TABLE_CELL_SIZE');
    expect(source).toContain('const [, setUndoStack] = useState<string[]>([]);');
    expect(source).toContain('const [redoStack, setRedoStack] = useState<string[]>([]);');
    expect(source).toContain('title: labels.btnModeEditor');
    expect(source).toContain('title: labels.btnModePreview');
    expect(source).toContain('title: labels.btnModeAll');
    expect(source).toContain("btnModeEditor: '仅显示编辑器'");
    expect(source).toContain("btnModePreview: '仅显示预览'");
    expect(source).toContain("btnModeAll: '显示编辑器与预览'");
    expect(source).toContain('replaceSelection(`[${selection.selected}]()`');
    expect(source).not.toContain('](https://)');
  });

  it('keeps the original sorted locale order in the knowledge editor', () => {
    expect(source).toContain('const LEGACY_KNOWLEDGE_I18N_TEXT = {');
    expect(source).toContain("'zh-CN': '简体中文'");
    expect(source).toContain("'zh-TW': '繁體中文'");
    expect(source).toContain("'en-US': 'English'");
    expect(source).toContain("'ja-JP': '日本語'");
    expect(source).toContain("'vi-VN': 'Tiếng Việt'");
    expect(source).toContain("'ko-KR': '한국어'");
    expect(source).toContain('Object.keys(LEGACY_KNOWLEDGE_I18N_TEXT) as LegacyKnowledgeLocale[]');
    expect(source).toContain(').sort();');
    expect(source).toContain(
      'const LEGACY_KNOWLEDGE_LOCALE_OPTIONS = LEGACY_KNOWLEDGE_LOCALES.map',
    );
    expect(source).toContain('label: LEGACY_KNOWLEDGE_I18N_TEXT[locale]');
    expect(source).toContain('<LegacySelect');
    expect(source).toContain('defaultValue={knowledge.language || 1}');
    expect(source).toContain('options={LEGACY_KNOWLEDGE_LOCALE_OPTIONS}');
    expect(source).toContain("import { LegacySelect } from '@/components/legacy-select';");
    expect(source).not.toContain("Select } from 'antd'");
    expect(source).not.toContain('<Select');
    expect(source).not.toContain('<Select.Option');
    expect(source).not.toContain('defaultValue={knowledge.language}');
    expect(source).not.toContain('@v2board/i18n');
    expect(source).not.toContain('SUPPORTED_LOCALES');
    expect(source).not.toContain('fa-IR');
  });

  it('uses the original fetchLoading-style page spinner for knowledge refetches', () => {
    expect(source).toContain('<LegacySpin loading={list.isFetching || sortingLoading}>');
    expect(source).not.toContain('<LegacySpin loading={list.isLoading}>');
  });

  it('keeps the original editor save and show-switch behavior', () => {
    const editorSaveBlock = source.slice(
      source.indexOf('const save = async () => {'),
      source.indexOf('return (', source.indexOf('const save = async () => {')),
    );

    expect(editorSaveBlock).toContain('await onSave({ ...knowledge });');
    expect(editorSaveBlock).toContain('await onSaved();');
    expect(editorSaveBlock).toContain("message.success('保存成功');");
    expect(editorSaveBlock.indexOf('await onSave({ ...knowledge });')).toBeLessThan(
      editorSaveBlock.indexOf('await onSaved();'),
    );
    expect(editorSaveBlock.indexOf('await onSaved();')).toBeLessThan(
      editorSaveBlock.indexOf("message.success('保存成功');"),
    );
    expect(source).toContain(
      'const saveKnowledge = (payload: SaveKnowledgePayload) => save.mutateAsync(payload);',
    );
    expect(source).toContain('const refetchKnowledge = () => list.refetch();');
    expect(source).toContain("message.success('保存成功');");
    expect(source).toContain('onSaved: () => void | Promise<unknown>;');
    expect(source).toContain('    await onSaved();\n    message.success');
    expect(source).not.toContain('void onSaved();');
    expect(source).toContain('saveLoading?: boolean;');
    expect(source).toContain('saveLoading={save.isPending}');
    expect(source).not.toContain('const [saveLoading, setSaveLoading] = useState(false);');
    expect(source).not.toContain('setSaveLoading(true);');
    expect(source).not.toContain('setSaveLoading(false);');
    expect(source).not.toContain('    onSaved();\n    message.success');
    expect(source).not.toContain("message.success('保存成功');\n      hide();");
    expect(source).not.toContain('await onSave(knowledge);');
    expect(source).not.toContain('await save.mutateAsync(payload);\n    await list.refetch();');
    expect(source).toContain('checked={value as unknown as boolean}');
    expect(source).toContain("import { LegacySwitch } from '@/components/legacy-switch';");
    expect(source).toContain('<LegacySwitch');
    expect(source).toContain('size="small"');
    expect(source).not.toContain('function LegacyKnowledgeSwitch');
    expect(source).not.toContain('<Switch');
    expect(source).toContain('show.mutate(row.id, {');
    expect(source).toContain('void list.refetch();');
    expect(source).not.toContain('checked={Boolean(value)}');
  });

  it('keeps the legacy table keying and delete-confirm behavior', () => {
    expect(source).toContain("import { legacyConfirm } from '@/components/legacy-confirm';");
    expect(source).toContain("import { App } from 'antd';");
    expect(source).not.toContain("import { App, Modal } from 'antd';");
    expect(source).not.toContain('Modal.confirm({');
    expect(source).toContain('void legacyConfirm({');
    expect(source).toContain('<LegacyStandaloneTable');
    expect(source).toContain('scrollX={750}');
    expect(source).toContain('{...legacyTableRowKey(index)}');
    expect(source).toContain('<LegacyDragSort');
    expect(source).toContain('nodeSelector="tr"');
    expect(source).toContain('handleSelector="i"');
    expect(source).toContain("<LegacyMenuIcon style={{ cursor: 'move' }} />");
    expect(source).toContain(
      'className="ant-table-align-right ant-table-row-cell-last"',
    );
    expect(source).toContain(
      '<td className="ant-table-align-right" style={{ textAlign: \'right\' }}>',
    );
    expect(source).toContain(
      'className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"',
    );
    expect(source).not.toContain('<Table<KnowledgeSummary>');
    expect(source).not.toContain('tableLayout="auto"');
    expect(source).not.toContain('pagination={false}');
    expect(source).not.toContain('data-sort-index');
    expect(source).not.toContain('<MenuOutlined');
    expect(source).not.toContain('dragIndex.current');
    expect(source).not.toContain('<span\n          draggable');
    expect(source).not.toContain('data-row-key');
    expect(source).not.toContain('rowKey="id"');
    expect(source).toContain(
      'onOk: () => {\n              void drop.mutateAsync(row.id).then(() => {',
    );
    expect(source).not.toContain('onOk: () => drop.mutateAsync(row.id)');
  });

  it('keeps the original vertical divider markup in the knowledge action column', () => {
    expect(source).toContain("import { LegacyDivider } from '@/components/legacy-divider';");
    expect(source).toContain('<LegacyDivider type="vertical" />');
    expect(source).not.toContain(
      '<div className="ant-divider ant-divider-vertical" role="separator" />',
    );
    expect(source).not.toContain('<span className="ant-divider ant-divider-vertical"');
  });

  it('keeps the bundled add button text flush against the plus icon', () => {
    expect(source).toContain('<LegacyPlusIcon />');
    expect(source).toContain('<span>新增</span>');
    expect(source).not.toContain('<span> 新增</span>');
    expect(source).not.toContain('<PlusOutlined /> 新增');
    expect(source).not.toContain('<PlusOutlined />');
  });

  it('keeps the original category request and sort loading cycle', () => {
    const sortBlock = source.slice(
      source.indexOf('const sortKnowledge = (fromIndex: number, toIndex: number) => {'),
      source.indexOf('const saveKnowledge =', source.indexOf('const sortKnowledge')),
    );

    expect(source).toContain('useAdminKnowledgeCategories();');
    expect(source).toContain('setSortingLoading(true)');
    expect(source).toContain('setSortingLoading(false)');
    expect(source).toContain('const sortKnowledge = (fromIndex: number, toIndex: number) => {');
    expect(source).toContain('next.splice(toIndex + 1, 0, moved);');
    expect(source).toContain('next.splice(fromIndex + 1, 1);');
    expect(source).toContain('sort.mutate(\n      next.map((knowledge) => knowledge.id),');
    expect(source).toContain('onSuccess: () => {\n          void list.refetch();\n        },');
    expect(sortBlock.indexOf('setSortingLoading(true);')).toBeLessThan(
      sortBlock.indexOf('setOrderedKnowledge(next);'),
    );
  });

  it('keeps knowledge mutations fetching from the page after successful requests', () => {
    const saveStart = source.indexOf(
      'const saveKnowledge = (payload: SaveKnowledgePayload) => save.mutateAsync(payload);',
    );
    const saveRefetch = source.indexOf('const refetchKnowledge = () => list.refetch();', saveStart);
    const editorSaveStart = source.indexOf('await onSave({ ...knowledge });');
    const editorRefetch = source.indexOf('await onSaved();', editorSaveStart);
    const sortStart = source.indexOf('sort.mutate(\n      next.map((knowledge) => knowledge.id),');
    const sortRefetch = source.indexOf('void list.refetch();', sortStart);
    const showStart = source.indexOf('show.mutate(row.id, {');
    const showRefetch = source.indexOf('void list.refetch();', showStart);
    const dropStart = source.indexOf('drop.mutateAsync(row.id).then');
    const dropRefetch = source.indexOf('void list.refetch();', dropStart);

    expect(saveStart).toBeGreaterThan(-1);
    expect(saveRefetch).toBeGreaterThan(saveStart);
    expect(editorSaveStart).toBeGreaterThan(-1);
    expect(editorRefetch).toBeGreaterThan(editorSaveStart);
    expect(sortStart).toBeGreaterThan(-1);
    expect(sortRefetch).toBeGreaterThan(sortStart);
    expect(showStart).toBeGreaterThan(-1);
    expect(showRefetch).toBeGreaterThan(showStart);
    expect(dropStart).toBeGreaterThan(-1);
    expect(dropRefetch).toBeGreaterThan(dropStart);

    for (const [start, end] of [
      ['export function useSaveKnowledgeMutation()', 'export function useDropKnowledgeMutation()'],
      ['export function useDropKnowledgeMutation()', 'export function useShowKnowledgeMutation()'],
      ['export function useShowKnowledgeMutation()', 'export function useSortKnowledgeMutation()'],
      [
        'export function useSortKnowledgeMutation()',
        'export function useSaveServerGroupMutation()',
      ],
    ] as const) {
      const hook = queriesSource.slice(queriesSource.indexOf(start), queriesSource.indexOf(end));
      expect(hook).not.toContain('onSuccess');
      expect(hook).not.toContain('adminKeys.knowledge');
    }
  });
});
