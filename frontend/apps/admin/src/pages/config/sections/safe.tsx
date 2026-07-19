import type { FormCtx } from '../schema';
import { Section, SwitchRow, TextRow, TextareaRow } from '../rows';
import { isBackendEnabled, parseBackendInteger, splitComma } from '../values';

export function SafeSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="安全">
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="email_verify"
        title="邮箱验证"
        description="开启后将会强制要求用户进行邮箱验证。"
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="email_gmail_limit_enable"
        title="禁止使用Gmail多别名"
        description="开启后Gmail多别名将无法注册。"
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="safe_mode_enable"
        title="安全模式"
        description="开启后除了站点URL以外的绑定本站点的域名访问都将会被403。"
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="admin_mfa_force"
        title="强制两步验证"
        description="开启后未启用两步验证的管理员/员工只能访问自己的两步验证设置，其余后台功能将被拒绝，直到完成绑定。"
      />
      <TextRow
        ctx={ctx}
        group="safe"
        field="secure_path"
        title="后台路径"
        description="后台管理路径，修改后将会改变原有的admin路径"
        placeholder="admin"
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="email_whitelist_enable"
        title="邮箱后缀白名单"
        description="开启后在名单中的邮箱后缀才允许进行注册。"
      />
      {isBackendEnabled(ctx.get('safe', 'email_whitelist_enable')) ? (
        <TextareaRow
          ctx={ctx}
          group="safe"
          field="email_whitelist_suffix"
          title="白名单后缀"
          description="请使用逗号进行分割，如：qq.com,gmail.com。"
          placeholder="请输入后缀域名，逗号分割 如：qq.com,gmail.com"
          rows={4}
          indent
          coerce={splitComma}
        />
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="recaptcha_enable"
        title="防机器人"
        description="开启后将会使用Google reCAPTCHA防止机器人。"
      />
      {isBackendEnabled(ctx.get('safe', 'recaptcha_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="safe"
            field="recaptcha_key"
            title="密钥"
            description="在Google reCAPTCHA申请的密钥。"
            placeholder="请输入"
            indent
          />
          <TextRow
            ctx={ctx}
            group="safe"
            field="recaptcha_site_key"
            title="网站密钥"
            description="在Google reCAPTCH申请的网站密钥。"
            placeholder="请输入"
            indent
          />
        </>
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="register_limit_by_ip_enable"
        title="IP注册限制"
        description="开启后如果IP注册账户达到规则要求将会被限制注册，请注意IP判断可能因为CDN或前置代理导致问题。"
      />
      {isBackendEnabled(ctx.get('safe', 'register_limit_by_ip_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="safe"
            field="register_limit_count"
            title="次数"
            description="达到注册次数后开启惩罚。"
            placeholder="请输入"
            indent
            coerce={parseBackendInteger}
          />
          <TextRow
            ctx={ctx}
            group="safe"
            field="register_limit_expire"
            title="惩罚时间(分钟)"
            description="需要等待惩罚时间过后才可以再次注册。"
            placeholder="请输入"
            indent
            coerce={parseBackendInteger}
          />
        </>
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="password_limit_enable"
        title="防爆破限制"
        description="开启后如果该账户尝试登陆失败次数过多将会被限制。"
      />
      {isBackendEnabled(ctx.get('safe', 'password_limit_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="safe"
            field="password_limit_count"
            title="次数"
            description="达到失败次数后开启惩罚。"
            placeholder="请输入"
            indent
            coerce={parseBackendInteger}
          />
          <TextRow
            ctx={ctx}
            group="safe"
            field="password_limit_expire"
            title="惩罚时间(分钟)"
            description="需要等待惩罚时间过后才可以再次登陆。"
            placeholder="请输入"
            indent
            coerce={parseBackendInteger}
          />
        </>
      ) : null}
    </Section>
  );
}
