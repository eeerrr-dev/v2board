import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import dayjs from 'dayjs';
import NoticesPage from './notices';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'notices.tsx'), 'utf8');
const queriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);

vi.mock('@/lib/queries', () => ({
  useAdminNotices: () => ({
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
    data: {
      data: [
        {
          id: 1,
          title: '维护通知',
          content: 'content',
          img_url: null,
          tags: ['system'],
          show: 1,
          created_at: 1700000000,
          updated_at: 1700000000,
        },
      ],
      total: 1,
    },
  }),
  useSaveNoticeMutation: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
  useDropNoticeMutation: () => ({
    mutate: vi.fn(),
  }),
  useShowNoticeMutation: () => ({
    mutate: vi.fn(),
  }),
}));

describe('NoticesPage legacy notice manager', () => {
  it('renders the original notice table shell and actions', () => {
    const html = renderToStaticMarkup(<NoticesPage />);

    expect(html).toContain('class="d-flex justify-content-between align-items-center"');
    expect(html).toContain('class="block block-rounded"');
    expect(html).toContain('class="bg-white"');
    expect(html).toContain('<button type="button" class="ant-btn">');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('添加公告');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain('class="ant-spin-nested-loading"');
    expect(html).toContain(
      'class="ant-table ant-table-default ant-table-scroll-position-left ant-table-scroll-position-right"',
    );
    expect(html).toContain('class="ant-table-scroll"');
    expect(html).toContain('tabindex="-1" class="ant-table-body" style="overflow-x:scroll"');
    expect(html).toContain('class="ant-table-fixed" style="width:950px"');
    expect(html).toContain('class="ant-table-fixed-right"');
    expect(html).toContain(
      'class="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"',
    );
    expect(html).toContain('data-row-key="0"');
    expect(html).toContain('class="ant-switch-small ant-switch ant-switch-checked"');
    expect(html).toContain('class="ant-switch-inner"');
    expect(html).toContain('#');
    expect(html).toContain('显示');
    expect(html).toContain('标题');
    expect(html).toContain('创建时间');
    expect(html).toContain('操作');
    expect(html).toContain('维护通知');
    expect(html).toContain(dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm'));
    expect(html).toContain('编辑');
    expect(html).toContain('删除');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-typography');
    expect(html).not.toContain('ant-table-cell');
  });

  it('keeps the original notice save callback order after the page fetch', () => {
    const saveBlock = source.slice(
      source.indexOf('const saveNotice = async () => {'),
      source.indexOf('const headers: LegacyStandaloneTableHeader[]'),
    );
    const mutationBlock = queriesSource.slice(
      queriesSource.indexOf('export function useSaveNoticeMutation()'),
      queriesSource.indexOf('export function useDropNoticeMutation()'),
    );

    expect(saveBlock).toContain('await save.mutateAsync({ ...submit });');
    expect(saveBlock).toContain('await notices.refetch();');
    expect(saveBlock).toContain('modalVisible();');
    expect(saveBlock.indexOf('await save.mutateAsync({ ...submit });')).toBeLessThan(
      saveBlock.indexOf('await notices.refetch();'),
    );
    expect(saveBlock.indexOf('await notices.refetch();')).toBeLessThan(
      saveBlock.indexOf('modalVisible();'),
    );
    expect(saveBlock).not.toContain('void notices.refetch();\n    modalVisible();');
    expect(mutationBlock).not.toContain(
      "queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] })",
    );
    expect(source).not.toContain('LoadingOutlined');
    expect(source).not.toContain('@ant-design/icons');
    expect(source).toContain(
      "import { LegacyLoadingIcon, LegacyPlusIcon } from '@/components/legacy-ant-icon';",
    );
    expect(source).toContain('const [saveLoading] = useState<boolean | undefined>(undefined);');
    expect(source).toContain('saveLoading || void saveNotice();');
    expect(source).toContain("okText={saveLoading ? <LegacyLoadingIcon /> : '提交'}");
    expect(source).not.toContain('onOk={() => void saveNotice()}');
    expect(source).not.toContain('okText="提交"');
    expect(source).not.toContain('save.isPending');
    expect(source).not.toContain('if (save.isPending) return;');
    expect(source).not.toContain('if (!save.isPending) void saveNotice();');
    expect(source).not.toContain('await save.mutateAsync(submit);');
    expect(source).not.toContain('okButtonProps={{ loading');
  });

  it('uses the old Ant Design modal shell for the notice editor', () => {
    const modalStart = source.indexOf('<LegacyModal');
    const modalEnd = source.indexOf('</LegacyModal>', modalStart);
    const modalBlock = source.slice(modalStart, modalEnd);

    expect(source).toContain("import { LegacyModal } from '@/components/legacy-modal';");
    expect(modalStart).toBeGreaterThan(-1);
    expect(modalBlock).toContain("title={`${submit.id ? '编辑公告' : '新建公告'}`}");
    expect(modalBlock).toContain('visible={visible}');
    expect(modalBlock).toContain("okText={saveLoading ? <LegacyLoadingIcon /> : '提交'}");
    expect(modalBlock).toContain('cancelText="取消"');
    expect(source).not.toContain('<Modal');
    expect(source).not.toContain('open={visible}');
  });

  it('uses the legacy tags select for notice labels', () => {
    expect(source).toContain("import { LegacySelect } from '@/components/legacy-select';");
    expect(source).toContain('<LegacySelect');
    expect(source).toContain('mode="tags"');
    expect(source).toContain('value={submit.tags || []}');
    expect(source).toContain('options={[]}');
    expect(source).toContain("tags: (tags.length > 0 ? tags : null) as Notice['tags']");
    expect(source).not.toContain('tags.map(String)');
    expect(source).not.toContain('<Select');
    expect(source).not.toContain("import { Input, Select } from 'antd';");
  });

  it('uses the legacy notice editor input shells', () => {
    expect(source).toContain(
      "import { LegacyInput, LegacyTextArea } from '@/components/legacy-input';",
    );
    expect(source).toContain('<LegacyInput');
    expect(source).toContain('<LegacyTextArea');
    expect(source).toContain('className="ant-input"');
    expect(source).toContain('placeholder="请输入公告标题"');
    expect(source).toContain('placeholder="请输入公告内容"');
    expect(source).toContain('placeholder="请输入图片URL"');
    expect(source).toContain('rows={12}');
    expect(source).not.toContain("import { Input } from 'antd';");
    expect(source).not.toContain('<Input');
    expect(source).not.toContain('Input.TextArea');
  });

  it('uses the original fetchLoading-style page spinner for notice refetches', () => {
    expect(source).toContain(
      '<LegacySpin loading={legacyFetchLoading(notices.isFetching, notices.error)}>',
    );
    expect(source).not.toContain('loading={notices.isLoading}');
  });

  it('keeps the original direct notice show and image value bindings', () => {
    expect(source).toContain("import { LegacySwitch } from '@/components/legacy-switch';");
    expect(source).toContain('<LegacySwitch');
    expect(source).toContain('checked={value as unknown as boolean}');
    expect(source).toContain('show.mutate(row.id, {');
    expect(source).toContain('void notices.refetch();');
    expect(source).toContain('value={submit.img_url as string | undefined}');
    expect(source).not.toContain('checked={Boolean(value)}');
    expect(source).not.toContain('<Switch');
    expect(source).not.toContain("Switch } from 'antd'");
    expect(source).not.toContain('submit.img_url ?? undefined');
  });

  it('keeps notice drop and show mutations fetching from the page after success', () => {
    const dropBlock = queriesSource.slice(
      queriesSource.indexOf('export function useDropNoticeMutation()'),
      queriesSource.indexOf('export function useShowNoticeMutation()'),
    );
    const showBlock = queriesSource.slice(
      queriesSource.indexOf('export function useShowNoticeMutation()'),
      queriesSource.indexOf('export function useSaveConfigMutation()'),
    );
    const showStart = source.indexOf('show.mutate(row.id, {');
    const showRefetch = source.indexOf('void notices.refetch();', showStart);
    const dropStart = source.indexOf('drop.mutate(row.id, {');
    const dropRefetch = source.indexOf('void notices.refetch();', dropStart);

    expect(showStart).toBeGreaterThan(-1);
    expect(showRefetch).toBeGreaterThan(showStart);
    expect(dropStart).toBeGreaterThan(-1);
    expect(dropRefetch).toBeGreaterThan(dropStart);
    expect(dropBlock).not.toContain('onSuccess');
    expect(dropBlock).not.toContain(
      "queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] })",
    );
    expect(showBlock).not.toContain('onSuccess');
    expect(showBlock).not.toContain(
      "queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] })",
    );
  });

  it('keeps the legacy notice table without an explicit rowKey', () => {
    expect(source).toContain('<LegacyStandaloneTable');
    expect(source).toContain('headers={headers}');
    expect(source).toContain('isEmpty={dataSource.length === 0}');
    expect(source).toContain('scrollX={950}');
    expect(source).toContain('fixedRightChildren={dataSource.map((row, index) => (');
    expect(source).toContain('{...legacyTableRowKey(index)}');
    expect(source).toContain(
      '<td className="ant-table-align-right" style={{ textAlign: \'right\' }}>',
    );
    expect(source).toContain(
      'className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"',
    );
    expect(source).not.toContain('<Table<Notice>');
    expect(source).not.toContain('tableLayout="auto"');
    expect(source).not.toContain('dataSource={dataSource}');
    expect(source).not.toContain('pagination={false}');
    expect(source).not.toContain('rowKey="id"');
  });

  it('uses the bundled table index when opening the notice editor', () => {
    expect(source).toContain('const renderNoticeActions = (row: Notice, index: number) =>');
    expect(source).toContain('setSubmit(dataSource[index] as Partial<Notice>);');
    expect(source).toContain('modalVisible();');
    expect(source).not.toContain('setSubmit(dataSource[index] ?? {});');
    expect(source).not.toContain('setSubmit(row);');
    expect(source).not.toContain('setVisible(true);');
  });

  it('clears the notice submit payload only after the modal has been hidden', () => {
    expect(source).toContain('useEffect(() => {');
    expect(source).toContain('if (!visible) setSubmit({});');
    expect(source).toContain('}, [visible]);');
    expect(source).toContain('setVisible((current) => !current);');
    expect(source).not.toContain('if (current) setSubmit({});');
  });

  it('keeps the original vertical divider markup in the notice action column', () => {
    expect(source).toContain("import { LegacyDivider } from '@/components/legacy-divider';");
    expect(source).toContain('<LegacyDivider type="vertical" />');
    expect(source).not.toContain(
      '<div className="ant-divider ant-divider-vertical" role="separator" />',
    );
    expect(source).not.toContain('<span className="ant-divider ant-divider-vertical"');
  });

  it('uses the bundled save endpoint for both creating and editing notices', () => {
    expect(source).toContain('useSaveNoticeMutation');
    expect(source).not.toContain('useUpdateNoticeMutation');
    expect(source).not.toContain('notice/update');
  });
});
