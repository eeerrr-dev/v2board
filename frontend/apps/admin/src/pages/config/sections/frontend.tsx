import { useTranslation } from 'react-i18next';
import type { FormCtx } from '../schema';
import { Section, SelectRow, TextRow } from '../rows';

export function FrontendSection({ ctx }: { ctx: FormCtx }) {
  const { t } = useTranslation();
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
    </Section>
  );
}
