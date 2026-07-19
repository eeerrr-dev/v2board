import type { FormCtx } from '../schema';
import { Section, SelectRow, TextRow } from '../rows';

export function FrontendSection({ ctx }: { ctx: FormCtx }) {
  // docs/api-dialect.md §10.6: the typed chat-widget configuration is the only
  // supported chat integration path (custom_html is removed). A configured
  // provider with a missing/malformed identifier is rejected by the backend
  // config save, so the identifier fields surface per selected provider.
  const chatProvider = String(ctx.get('frontend', 'chat_widget_provider') ?? '')
    .trim()
    .toLowerCase();
  return (
    <Section title="个性化">
      <SelectRow
        ctx={ctx}
        group="frontend"
        field="frontend_theme_color"
        title="主题色"
        fallback="default"
        options={[
          { value: 'default', label: '默认' },
          { value: 'black', label: '黑色' },
          { value: 'darkblue', label: '暗蓝色' },
          { value: 'green', label: '奶绿色' },
        ]}
      />
      <TextRow
        ctx={ctx}
        group="frontend"
        field="frontend_background_url"
        title="背景"
        description="将会在后台登录页面进行展示。"
        placeholder="https://xxxxx.com/wallpaper.png"
      />
      <SelectRow
        ctx={ctx}
        group="frontend"
        field="chat_widget_provider"
        title="在线聊天挂件"
        description="为用户端加载官方聊天 SDK；需要完整填写所选提供商的标识后才会生效。"
        placeholder="请选择"
        fallback="off"
        options={[
          { value: 'off', label: '关闭' },
          { value: 'crisp', label: 'Crisp' },
          { value: 'tawk', label: 'Tawk.to' },
        ]}
        serialize={(value) => (value === 'off' ? '' : value)}
      />
      {chatProvider === 'crisp' ? (
        <TextRow
          ctx={ctx}
          group="frontend"
          field="chat_widget_crisp_website_id"
          title="Crisp Website ID"
          description="Crisp 后台的 Website ID（UUID 格式）。"
          placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
          indent
        />
      ) : null}
      {chatProvider === 'tawk' ? (
        <>
          <TextRow
            ctx={ctx}
            group="frontend"
            field="chat_widget_tawk_property_id"
            title="Tawk Property ID"
            description="Tawk 后台的 Property ID（24 位十六进制）。"
            placeholder="请输入"
            indent
          />
          <TextRow
            ctx={ctx}
            group="frontend"
            field="chat_widget_tawk_widget_id"
            title="Tawk Widget ID"
            description="Tawk 后台的 Widget ID。"
            placeholder="default"
            indent
          />
        </>
      ) : null}
    </Section>
  );
}
