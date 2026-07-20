import { useTranslation } from 'react-i18next';
import type { FormCtx } from '../schema';
import { Section, TextareaRow } from '../rows';

export function DepositSection({ ctx }: { ctx: FormCtx }) {
  const { t } = useTranslation();
  return (
    <Section title={t(($) => $.admin.config.sections.deposit)}>
      <TextareaRow
        ctx={ctx}
        group="deposit"
        field="deposit_bounus"
        title={t(($) => $.admin.config.deposit.bonus_title)}
        description={t(($) => $.admin.config.deposit.bonus_desc)}
        placeholder={t(($) => $.admin.config.deposit.bonus_placeholder)}
        rows={2}
      />
    </Section>
  );
}
