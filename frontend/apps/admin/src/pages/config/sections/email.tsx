import { Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { FormCtx } from '../schema';
import { Section, SelectRow, SettingRow, TextRow, WarningAlert } from '../rows';
import { parseBackendInteger } from '../values';

export function EmailSection({
  ctx,
  templates,
  onTest,
  testing,
}: {
  ctx: FormCtx;
  templates: string[];
  onTest: () => void;
  testing: boolean;
}) {
  return (
    <div className="space-y-4">
      <WarningAlert>
        保存后 API 与后台任务会自动应用最新邮件配置；本页配置优先级高于环境变量中的邮件配置。
      </WarningAlert>
      <Section title="邮件">
        <TextRow
          ctx={ctx}
          group="email"
          field="email_host"
          title="SMTP服务器地址"
          description="由邮件服务商提供的服务地址"
          placeholder="请输入"
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_port"
          title="SMTP服务端口"
          description="常见的端口有25, 465, 587"
          placeholder="请输入"
          coerce={parseBackendInteger}
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_encryption"
          title="SMTP加密方式"
          description="465端口加密方式一般为SSL，587端口加密方式一般为TLS"
          placeholder="请输入"
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_username"
          title="SMTP账号"
          description="由邮件服务商提供的账号"
          placeholder="请输入"
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_password"
          title="SMTP密码"
          description="由邮件服务商提供的密码"
          placeholder="请输入"
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_from_address"
          title="发件地址"
          description="由邮件服务商提供的发件地址"
          placeholder="请输入"
        />
        <SelectRow
          ctx={ctx}
          group="email"
          field="email_template"
          title="邮件模板"
          description="选择当前原生运行时提供的邮件模板"
          options={templates.map((template) => ({ value: template, label: template }))}
        />
        <SettingRow title="发送测试邮件" description="邮件将会发送到当前登陆用户邮箱">
          <Button onClick={onTest} disabled={testing} data-testid="config-test-mail">
            {testing ? (
              <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            发送测试邮件
          </Button>
        </SettingRow>
      </Section>
    </div>
  );
}
