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
    expect(source).toContain("const text = value ?? '';");
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
    expect(source).toContain('<LegacyInput');
    expect(source).toContain('className="ant-input"');
    expect(source).toContain('defaultValue={knowledge.title}');
    expect(source).toContain('defaultValue={knowledge.category}');
    expect(source).toContain('<LegacySelect');
    expect(source).toContain('<LegacyDrawer');
    expect(source).toContain('<LegacyButton className="ant-btn" style={{ marginRight: 8 }}');
    expect(source).toContain(
      "className={`ant-btn ant-btn-primary${saveLoading ? ' ant-btn-loading' : ''}`}",
    );
    expect(source).not.toContain('<Drawer');
    expect(source).not.toContain('<Button');
    expect(source).not.toContain(' Button,');
    expect(source).not.toContain(' Drawer,');
    expect(source).not.toContain('Input,');
    expect(source).not.toContain(" Input } from 'antd'");
    expect(source).not.toContain('<Input');
    expect(source).not.toContain('value={knowledge.title}');
    expect(source).not.toContain('value={knowledge.category}');
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

    expect(toolbar).toContain('title="Underline"');
    expect(toolbar).toContain('title="Strikethrough"');
    expect(toolbar).toContain('title="Ordered list"');
    expect(toolbar).toContain('title="Line break"');
    expect(toolbar).toContain('title="Inline code"');
    expect(toolbar).toContain('title="Table"');
    expect(toolbar).toContain('title="Image"');
    expect(toolbar).toContain('title="Clear"');
    expect(toolbar).toContain('title="Undo"');
    expect(toolbar).toContain('title="Redo"');
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
    expect(source).toContain('function legacyTableMarkdown(row: number, col: number)');
    expect(source).toContain("function legacyListMarkdown(type: 'ordered' | 'unordered'");
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
    expect(source).toContain('const [undoStack, setUndoStack] = useState<string[]>([]);');
    expect(source).toContain('const [redoStack, setRedoStack] = useState<string[]>([]);');
    expect(source).toContain("title: 'Only display editor'");
    expect(source).toContain("title: 'Only display preview'");
    expect(source).toContain("title: 'Display both editor and preview'");
    expect(source).not.toContain('仅显示编辑器');
    expect(source).not.toContain('仅显示预览');
    expect(source).not.toContain('显示编辑器与预览');
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
    expect(source).toContain('options={LEGACY_KNOWLEDGE_LOCALE_OPTIONS}');
    expect(source).toContain("import { LegacySelect } from '@/components/legacy-select';");
    expect(source).not.toContain("Select } from 'antd'");
    expect(source).not.toContain('<Select');
    expect(source).not.toContain('<Select.Option');
    expect(source).not.toContain('defaultValue={knowledge.language}');
    expect(source).not.toContain('defaultValue={knowledge.language || 1}');
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
    expect(editorSaveBlock).toContain('void onSaved();');
    expect(editorSaveBlock).toContain("message.success('保存成功');");
    expect(editorSaveBlock.indexOf('await onSave({ ...knowledge });')).toBeLessThan(
      editorSaveBlock.indexOf('void onSaved();'),
    );
    expect(editorSaveBlock.indexOf('void onSaved();')).toBeLessThan(
      editorSaveBlock.indexOf("message.success('保存成功');"),
    );
    expect(source).toContain(
      'const saveKnowledge = (payload: SaveKnowledgePayload) => save.mutateAsync(payload);',
    );
    expect(source).toContain('const refetchKnowledge = () => list.refetch();');
    expect(source).toContain("message.success('保存成功');");
    expect(source).toContain('onSaved: () => void | Promise<unknown>;');
    expect(source).toContain('    void onSaved();\n    message.success');
    expect(source).not.toContain('await onSaved();');
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
    expect(source).toContain('function LegacyKnowledgeSwitch');
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
    expect(source).toContain(
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
    expect(source).toContain('useAdminKnowledgeCategories();');
    expect(source).toContain('setSortingLoading(true)');
    expect(source).toContain('setSortingLoading(false)');
    expect(source).toContain('const sortKnowledge = (fromIndex: number, toIndex: number) => {');
    expect(source).toContain('next.splice(toIndex + 1, 0, moved);');
    expect(source).toContain('next.splice(fromIndex + 1, 1);');
    expect(source).toContain('sort.mutate(\n      next.map((knowledge) => knowledge.id),');
    expect(source).toContain('onSuccess: () => {\n          void list.refetch();\n        },');
  });

  it('keeps knowledge mutations fetching from the page after successful requests', () => {
    const saveStart = source.indexOf(
      'const saveKnowledge = (payload: SaveKnowledgePayload) => save.mutateAsync(payload);',
    );
    const saveRefetch = source.indexOf('const refetchKnowledge = () => list.refetch();', saveStart);
    const editorSaveStart = source.indexOf('await onSave({ ...knowledge });');
    const editorRefetch = source.indexOf('void onSaved();', editorSaveStart);
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
