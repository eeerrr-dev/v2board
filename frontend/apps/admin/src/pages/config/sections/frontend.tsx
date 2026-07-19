import { useTranslation } from 'react-i18next';
import type { FormCtx } from '../schema';
import { Section, SelectRow, TextRow } from '../rows';

export function FrontendSection({ ctx }: { ctx: FormCtx }) {
  const { t } = useTranslation();
  // docs/api-dialect.md §10.6: the typed chat-widget configuration is the only
  // supported chat integration path (custom_html is removed). A configured
  // provider with a missing/malformed identifier is rejected by the backend
  // config save, so the identifier fields surface per selected provider.
  const chatProvider = String(ctx.get('frontend', 'chat_widget_provider') ?? '')
    .trim()
    .toLowerCase();
  return (
    <Section title={t(($) => $.admin.config.sections.frontend)}>
      <SelectRow
        ctx={ctx}
        group="frontend"
        field="frontend_theme_color"
        title={t(($) => $.admin.config.frontend.theme_color_title)}
        fallback="default"
        options={[
          { value: 'default', label: t(($) => $.admin.config.frontend.theme_default) },
          { value: 'black', label: t(($) => $.admin.config.frontend.theme_black) },
          { value: 'darkblue', label: t(($) => $.admin.config.frontend.theme_darkblue) },
          { value: 'green', label: t(($) => $.admin.config.frontend.theme_green) },
        ]}
      />
      <TextRow
        ctx={ctx}
        group="frontend"
        field="frontend_background_url"
        title={t(($) => $.admin.config.frontend.background_title)}
        description={t(($) => $.admin.config.frontend.background_desc)}
        placeholder="https://xxxxx.com/wallpaper.png"
      />
      <SelectRow
        ctx={ctx}
        group="frontend"
        field="chat_widget_provider"
        title={t(($) => $.admin.config.frontend.chat_widget_title)}
        description={t(($) => $.admin.config.frontend.chat_widget_desc)}
        placeholder={t(($) => $.admin.config.select_placeholder)}
        fallback="off"
        options={[
          { value: 'off', label: t(($) => $.common.close) },
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
          description={t(($) => $.admin.config.frontend.crisp_website_id_desc)}
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
            description={t(($) => $.admin.config.frontend.tawk_property_id_desc)}
            placeholder={t(($) => $.admin.config.input_placeholder)}
            indent
          />
          <TextRow
            ctx={ctx}
            group="frontend"
            field="chat_widget_tawk_widget_id"
            title="Tawk Widget ID"
            description={t(($) => $.admin.config.frontend.tawk_widget_id_desc)}
            placeholder="default"
            indent
          />
        </>
      ) : null}
    </Section>
  );
}
