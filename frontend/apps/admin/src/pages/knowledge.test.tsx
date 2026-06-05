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
    expect(html).not.toContain('ant-typography');
  });

  it('uses the legacy markdown editor structure for knowledge bodies', () => {
    expect(source).toContain('width="80%"');
    expect(source).toContain('id="knowledge"');
    expect(source).not.toContain('size="80%"');
    expect(source).toContain('new MarkdownIt({ html: true, linkify: true, typographer: true })');
    expect(source).toContain('function LegacyMarkdownEditor');
    expect(source).toContain('rc-md-editor ${fullScreen');
    expect(source).toContain('className="rc-md-navigation visible"');
    expect(source).toContain('className="section-container input"');
    expect(source).toContain('className="section-container html-wrap"');
    expect(source).toContain('dangerouslySetInnerHTML={{ __html: html }}');
    expect(source).toContain('const text = value ?? \'\';');
    expect(source).toContain('value={knowledge.body}');
    expect(source).toContain('const [editorKey, setEditorKey] = useState(Math.random());');
    expect(source).toContain('setEditorKey(Math.random());');
    expect(source).not.toContain('<Input.TextArea');
    expect(source).not.toContain('value={knowledge.body ?? \'\'}');
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
    expect(source).toContain('LEGACY_KNOWLEDGE_LOCALES.map((locale)');
    expect(source).toContain('<Select.Option value={locale}>');
    expect(source).toContain('{LEGACY_KNOWLEDGE_I18N_TEXT[locale]}');
    expect(source).not.toContain('@v2board/i18n');
    expect(source).not.toContain('SUPPORTED_LOCALES');
    expect(source).not.toContain('fa-IR');
    expect(source).not.toContain('<Select.Option key={locale} value={locale}>');
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

    expect(editorSaveBlock).toContain("await onSave({ ...knowledge });");
    expect(editorSaveBlock).toContain('await onSaved();');
    expect(editorSaveBlock).toContain("message.success('保存成功');");
    expect(editorSaveBlock.indexOf("await onSave({ ...knowledge });")).toBeLessThan(
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
    expect(source).toContain('saveLoading?: boolean;');
    expect(source).toContain('saveLoading={save.isPending}');
    expect(source).not.toContain('const [saveLoading, setSaveLoading] = useState(false);');
    expect(source).not.toContain('setSaveLoading(true);');
    expect(source).not.toContain('setSaveLoading(false);');
    expect(source).not.toContain('    onSaved();\n    message.success');
    expect(source).not.toContain('message.success(\'保存成功\');\n      hide();');
    expect(source).not.toContain("await onSave(knowledge);");
    expect(source).not.toContain('await save.mutateAsync(payload);\n    await list.refetch();');
    expect(source).toContain('checked={value as unknown as boolean}');
    expect(source).toContain('show.mutate(row.id, {');
    expect(source).toContain('void list.refetch();');
    expect(source).not.toContain('checked={Boolean(value)}');
  });

  it('keeps the legacy table keying and delete-confirm behavior', () => {
    expect(source).toContain('tableLayout="auto"');
    expect(source).toContain('pagination={false}');
    expect(source).toContain('<LegacyDragSort');
    expect(source).toContain('nodeSelector="tr"');
    expect(source).toContain('handleSelector="i"');
    expect(source).toContain('<LegacyMenuIcon />');
    expect(source).not.toContain('data-sort-index');
    expect(source).not.toContain('<MenuOutlined');
    expect(source).not.toContain('dragIndex.current');
    expect(source).not.toContain('<span\n          draggable');
    expect(source).not.toContain('data-row-key');
    expect(source).not.toContain('rowKey="id"');
    expect(source).toContain('onOk: () => {\n                  void drop.mutateAsync(row.id).then(() => {');
    expect(source).not.toContain('onOk: () => drop.mutateAsync(row.id)');
  });

  it('keeps the original vertical divider markup in the knowledge action column', () => {
    expect(source).toContain('<div className="ant-divider ant-divider-vertical" />');
    expect(source).not.toContain('<span className="ant-divider ant-divider-vertical"');
    expect(source).not.toContain('role="separator"');
  });

  it('keeps the original category request and sort loading cycle', () => {
    expect(source).toContain('useAdminKnowledgeCategories();');
    expect(source).toContain('setSortingLoading(true)');
    expect(source).toContain('setSortingLoading(false)');
    expect(source).toContain('const sortKnowledge = (fromIndex: number, toIndex: number) => {');
    expect(source).toContain('next.splice(toIndex + 1, 0, moved);');
    expect(source).toContain('next.splice(fromIndex + 1, 1);');
    expect(source).toContain('sort.mutate(next.map((knowledge) => knowledge.id),');
    expect(source).toContain('onSuccess: () => {\n                void list.refetch();\n              },');
  });

  it('keeps knowledge mutations fetching from the page after successful requests', () => {
    const saveStart = source.indexOf(
      'const saveKnowledge = (payload: SaveKnowledgePayload) => save.mutateAsync(payload);',
    );
    const saveRefetch = source.indexOf('const refetchKnowledge = () => list.refetch();', saveStart);
    const editorSaveStart = source.indexOf("await onSave({ ...knowledge });");
    const editorRefetch = source.indexOf('await onSaved();', editorSaveStart);
    const sortStart = source.indexOf('sort.mutate(next.map((knowledge) => knowledge.id),');
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
      ['export function useSortKnowledgeMutation()', 'export function useSaveServerGroupMutation()'],
    ] as const) {
      const hook = queriesSource.slice(queriesSource.indexOf(start), queriesSource.indexOf(end));
      expect(hook).not.toContain('onSuccess');
      expect(hook).not.toContain('adminKeys.knowledge');
    }
  });
});
