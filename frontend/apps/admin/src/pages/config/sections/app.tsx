import { Controller } from 'react-hook-form';
import { useTranslation } from 'react-i18next';
import { Field, FieldError } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import type { FormCtx } from '../schema';
import { Section, SettingRow, WarningAlert } from '../rows';
import { toText } from '../values';

export function AppSection({ ctx }: { ctx: FormCtx }) {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <WarningAlert>{t(($) => $.admin.config.app.notice)}</WarningAlert>
      <Section title={t(($) => $.admin.config.sections.app)}>
        <AppEntryRow
          ctx={ctx}
          title="Windows"
          description={t(($) => $.admin.config.app.windows_desc)}
          versionField="windows_version"
          urlField="windows_download_url"
          urlPlaceholder="https://xxxx.com/xxx.exe"
        />
        <AppEntryRow
          ctx={ctx}
          title="macOS"
          description={t(($) => $.admin.config.app.macos_desc)}
          versionField="macos_version"
          urlField="macos_download_url"
          urlPlaceholder="https://xxxx.com/xxx.dmg"
        />
        <AppEntryRow
          ctx={ctx}
          title="Android"
          description={t(($) => $.admin.config.app.android_desc)}
          versionField="android_version"
          urlField="android_download_url"
          urlPlaceholder="https://xxxx.com/xxx.apk"
        />
      </Section>
    </div>
  );
}

function AppEntryRow({
  ctx,
  title,
  description,
  versionField,
  urlField,
  urlPlaceholder,
}: {
  ctx: FormCtx;
  title: string;
  description: string;
  versionField: string;
  urlField: string;
  urlPlaceholder: string;
}) {
  const { t } = useTranslation();
  return (
    <SettingRow title={title} description={description}>
      <div className="space-y-2">
        <Controller
          control={ctx.control}
          name={versionField}
          render={({ field, fieldState }) => (
            <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(versionField)}>
              <Input
                ref={field.ref}
                name={field.name}
                placeholder="1.0.0"
                aria-label={t(($) => $.admin.config.app.version_label, { title })}
                aria-invalid={fieldState.invalid}
                data-testid={`config-${versionField}`}
                value={toText(field.value)}
                onChange={(event) => field.onChange(event.target.value)}
                onBlur={(event) => {
                  field.onBlur();
                  void ctx.save('app', versionField, event.target.value);
                }}
              />
              <FieldError errors={[fieldState.error]} />
            </Field>
          )}
        />
        <Controller
          control={ctx.control}
          name={urlField}
          render={({ field, fieldState }) => (
            <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(urlField)}>
              <Input
                ref={field.ref}
                name={field.name}
                placeholder={urlPlaceholder}
                aria-label={t(($) => $.admin.config.app.url_label, { title })}
                aria-invalid={fieldState.invalid}
                data-testid={`config-${urlField}`}
                value={toText(field.value)}
                onChange={(event) => field.onChange(event.target.value)}
                onBlur={(event) => {
                  field.onBlur();
                  void ctx.save('app', urlField, event.target.value);
                }}
              />
              <FieldError errors={[fieldState.error]} />
            </Field>
          )}
        />
      </div>
    </SettingRow>
  );
}
