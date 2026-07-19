// Values stay Chinese until product translations are supplied (see ./index.ts).
// Keys follow the source order of apps/admin/src/pages/knowledge.tsx, then
// knowledge-form-schema.ts. `locale_labels` is keyed by the canonical backend
// locale codes (KNOWLEDGE_LOCALES); its values are language autonyms, so they
// stay identical in every translation.
export const adminKnowledge = {
  locale_labels: {
    'zh-CN': '简体中文',
    'zh-TW': '繁體中文',
    'en-US': 'English',
    'ja-JP': '日本語',
    'vi-VN': 'Tiếng Việt',
    'ko-KR': '한국어',
  },
  title_placeholder: '请输入知识标题',
  category: '分类',
  category_placeholder: '请输入分类，分类将会自动归集',
  language_placeholder: '请选择知识语言',
  body_label: '内容',
  body_placeholder: '请输入知识内容，支持 Markdown',
  save_success: '保存成功',
  edit_title: '编辑知识',
  create_title: '新增知识',
  sheet_description: '编辑文章分类、语言、标题和 Markdown 内容。',
  detail_error: '知识详情加载失败',
  delete_confirm_title: '警告',
  delete_confirm_description: '确定要删除该条项目吗？',
  id_column: '文章ID',
  show: '显示',
  toggle_show: '切换「{{title}}」显示',
  updated_at: '更新时间',
  move_up: '上移',
  move_down: '下移',
  list_error: '知识库加载失败',
  categories_error: '知识分类加载失败',
  page_title: '知识库管理',
  empty: '暂无知识',
  category_required: '分类不能为空',
  language_required: '语言不能为空',
  title_required: '标题不能为空',
  body_required: '内容不能为空',
};
