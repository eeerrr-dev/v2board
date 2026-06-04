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
    expect(html).toContain('添加公告');
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
  });

  it('keeps the original notice save callback order after the page fetch', () => {
    const saveBlock = source.slice(
      source.indexOf('const saveNotice = async () => {'),
      source.indexOf('const columns: TableProps<Notice>'),
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
    expect(mutationBlock).not.toContain("queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] })");
    expect(source).toContain("import { LoadingOutlined, PlusOutlined } from '@ant-design/icons';");
    expect(source).toContain('if (!save.isPending) void saveNotice();');
    expect(source).toContain("okText={save.isPending ? <LoadingOutlined /> : '提交'}");
    expect(source).not.toContain('onOk={() => void saveNotice()}');
    expect(source).not.toContain('okText="提交"');
    expect(source).not.toContain('if (save.isPending) return;');
    expect(source).not.toContain('await save.mutateAsync(submit);');
    expect(source).not.toContain('okButtonProps={{ loading');
  });

  it('uses the original fetchLoading-style page spinner for notice refetches', () => {
    expect(source).toContain('<LegacySpin loading={notices.isFetching}>');
    expect(source).not.toContain('loading={notices.isLoading}');
  });

  it('keeps the original direct notice show and image value bindings', () => {
    expect(source).toContain('checked={value as unknown as boolean}');
    expect(source).toContain('show.mutate(row.id, {');
    expect(source).toContain('void notices.refetch();');
    expect(source).toContain('value={submit.img_url as string | undefined}');
    expect(source).not.toContain('checked={Boolean(value)}');
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
    expect(dropBlock).not.toContain("queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] })");
    expect(showBlock).not.toContain('onSuccess');
    expect(showBlock).not.toContain("queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] })");
  });

  it('keeps the legacy notice table without an explicit rowKey', () => {
    expect(source).toContain('tableLayout="auto"');
    expect(source).toContain('dataSource={dataSource}');
    expect(source).toContain('pagination={false}');
    expect(source).not.toContain('rowKey="id"');
  });

  it('uses the bundled table index when opening the notice editor', () => {
    expect(source).toContain('render: (_value, row, index) =>');
    expect(source).toContain('setSubmit(dataSource[index] as Partial<Notice>);');
    expect(source).not.toContain('setSubmit(dataSource[index] ?? {});');
    expect(source).not.toContain('setSubmit(row);');
  });

  it('clears the notice submit payload only after the modal has been hidden', () => {
    expect(source).toContain('useEffect(() => {');
    expect(source).toContain('if (!visible) setSubmit({});');
    expect(source).toContain('}, [visible]);');
    expect(source).toContain('setVisible((current) => !current);');
    expect(source).not.toContain('if (current) setSubmit({});');
  });

  it('keeps the original vertical divider markup in the notice action column', () => {
    expect(source).toContain('<div className="ant-divider ant-divider-vertical" />');
    expect(source).not.toContain('<span className="ant-divider ant-divider-vertical"');
    expect(source).not.toContain('role="separator"');
  });

  it('uses the bundled save endpoint for both creating and editing notices', () => {
    expect(source).toContain('useSaveNoticeMutation');
    expect(source).not.toContain('useUpdateNoticeMutation');
    expect(source).not.toContain('notice/update');
  });
});
