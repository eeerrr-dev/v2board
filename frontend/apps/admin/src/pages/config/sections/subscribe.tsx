import type { FormCtx } from '../schema';
import { ORDER_EVENT_OPTIONS, Section, SelectRow, SwitchRow, TextRow } from '../rows';
import { parseBackendInteger, selectBoolean, selectInteger } from '../values';

export function SubscribeSection({ ctx }: { ctx: FormCtx }) {
  const timedExpire = String(ctx.get('subscribe', 'show_subscribe_method') ?? 0) === '2';
  return (
    <Section title="订阅">
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="plan_change_enable"
        title="允许用户更改订阅"
        description="开启后用户将会可以对订阅计划进行变更。"
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="reset_traffic_method"
        title="月流量重置方式"
        description="全局流量重置方式，默认每月1号。可以在订阅管理为订阅单独设置。"
        placeholder="请选择订阅重置方式"
        fallback="0"
        options={[
          { value: '0', label: '每月1号' },
          { value: '1', label: '按月重置' },
          { value: '2', label: '不重置' },
          { value: '3', label: '每年1月1日' },
          { value: '4', label: '按年重置' },
        ]}
        serialize={selectInteger}
      />
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="surplus_enable"
        title="开启折抵方案"
        description="开启后用户更换订阅将会由系统对原有订阅进行折抵，方案参考文档。"
      />
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="allow_new_period"
        title="允许提前开启流量周期"
        description="开启后用户流量用尽时可以选择扣除订阅时长为代价重置流量，按月重置时扣除本周期剩余订阅时长，每月1号重置时扣除整月时间30天。"
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="new_order_event_id"
        title="当订阅新购时触发事件"
        description="新购订阅完成时将触发该任务。"
        placeholder="请选择事件"
        fallback="0"
        options={ORDER_EVENT_OPTIONS}
        serialize={selectBoolean}
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="renew_order_event_id"
        title="当订阅续费时触发事件"
        description="续费订阅完成时将触发该任务。"
        placeholder="请选择事件"
        fallback="0"
        options={ORDER_EVENT_OPTIONS}
        serialize={selectBoolean}
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="change_order_event_id"
        title="当订阅变更时触发事件"
        description="变更订阅完成时将触发该任务。"
        placeholder="请选择事件"
        fallback="0"
        options={ORDER_EVENT_OPTIONS}
        serialize={selectBoolean}
      />
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="show_info_to_server_enable"
        title="在订阅中展示订阅信息"
        description="开启后将会在用户订阅节点时输出订阅信息。"
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="show_subscribe_method"
        title="订阅链接生效模式"
        description="用户获取订阅链接后的有效期。"
        placeholder="请选择"
        fallback="0"
        options={[
          { value: '0', label: '永久有效' },
          { value: '1', label: '一次性有效' },
          { value: '2', label: '限时有效' },
        ]}
        serialize={selectInteger}
      />
      {timedExpire ? (
        <TextRow
          ctx={ctx}
          group="subscribe"
          field="show_subscribe_expire"
          title="订阅链接有效时间(分钟)"
          description="订阅链接获取后经过该时间将失效。"
          placeholder="请输入"
          indent
          coerce={parseBackendInteger}
        />
      ) : null}
    </Section>
  );
}
