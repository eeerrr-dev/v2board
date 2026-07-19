import type { FormCtx } from '../schema';
import { Section, SelectRow } from '../rows';
import { selectInteger } from '../values';

export function TicketSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="工单">
      <SelectRow
        ctx={ctx}
        group="ticket"
        field="ticket_status"
        title="工单设置"
        description="请选择工单的状态。"
        fallback="0"
        options={[
          { value: '0', label: '完全开放工单' },
          { value: '1', label: '仅限有付费订单用户' },
          { value: '2', label: '完全禁止工单' },
        ]}
        serialize={selectInteger}
      />
    </Section>
  );
}
