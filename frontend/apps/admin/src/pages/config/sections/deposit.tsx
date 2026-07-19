import type { FormCtx } from '../schema';
import { Section, TextareaRow } from '../rows';
import { splitComma } from '../values';

export function DepositSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="充值">
      <TextareaRow
        ctx={ctx}
        group="deposit"
        field="deposit_bounus"
        title="充值奖励"
        description="充值一定金额可以获得的奖励。"
        placeholder={'请输入 充值金额:奖励金额,逗号分割\n如 50:18,100:38,200:88'}
        rows={2}
        coerce={splitComma}
      />
    </Section>
  );
}
