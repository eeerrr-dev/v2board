// Values stay Chinese until product translations are supplied (see ./index.ts).
// Keys follow the source order of apps/admin/src/pages/notices.tsx, then
// notice-form-schema.ts.
export const adminNotices = {
  delete_confirm_title: '删除公告',
  delete_confirm_description: '确定要删除公告「{{title}}」吗？',
  show: '显示',
  toggle_show: '切换公告「{{title}}」显示',
  created_at: '创建时间',
  list_error: '公告列表加载失败',
  page_title: '公告管理',
  create: '添加公告',
  empty: '暂无公告',
  edit_title: '编辑公告',
  create_title: '新建公告',
  dialog_description: '编辑公告标题、内容、标签和图片。',
  title_placeholder: '请输入公告标题',
  content_label: '公告内容',
  content_placeholder: '请输入公告内容',
  tags_label: '公告标签',
  tags_placeholder: '输入后回车添加标签',
  img_url_label: '图片URL',
  img_url_placeholder: '请输入图片URL',
  title_required: '标题不能为空',
  content_required: '内容不能为空',
  img_url_invalid: '图片URL格式不正确',
  tag_required: '标签不能为空',
};
