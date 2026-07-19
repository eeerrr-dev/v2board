import { Controller } from 'react-hook-form';
import { Field, FieldError } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import type { FormCtx } from '../schema';
import { Section, SettingRow, WarningAlert } from '../rows';
import { toText } from '../values';

export function AppSection({ ctx }: { ctx: FormCtx }) {
  return (
    <div className="space-y-4">
      <WarningAlert>用于自有客户端(APP)的版本管理及更新</WarningAlert>
      <Section title="APP">
        <AppEntryRow
          ctx={ctx}
          title="Windows"
          description="Windows端版本号及下载地址"
          versionField="windows_version"
          urlField="windows_download_url"
          urlPlaceholder="https://xxxx.com/xxx.exe"
        />
        <AppEntryRow
          ctx={ctx}
          title="macOS"
          description="macOS端版本号及下载地址"
          versionField="macos_version"
          urlField="macos_download_url"
          urlPlaceholder="https://xxxx.com/xxx.dmg"
        />
        <AppEntryRow
          ctx={ctx}
          title="Android"
          description="Android端版本号及下载地址"
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
                aria-label={`${title}版本号`}
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
                aria-label={`${title}下载地址`}
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
