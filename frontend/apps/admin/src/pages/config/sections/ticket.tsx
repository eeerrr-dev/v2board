import { useTranslation } from 'react-i18next';
import type { FormCtx } from '../schema';
import { Section, SelectRow } from '../rows';
import { selectInteger } from '../values';

export function TicketSection({ ctx }: { ctx: FormCtx }) {
  const { t } = useTranslation();
  return (
    <Section title={t(($) => $.admin.config.sections.ticket)}>
      <SelectRow
        ctx={ctx}
        group="ticket"
        field="ticket_status"
        title={t(($) => $.admin.config.ticket.status_title)}
        description={t(($) => $.admin.config.ticket.status_desc)}
        fallback="0"
        options={[
          { value: '0', label: t(($) => $.admin.config.ticket.status_open) },
          { value: '1', label: t(($) => $.admin.config.ticket.status_paid_only) },
          { value: '2', label: t(($) => $.admin.config.ticket.status_closed) },
        ]}
        serialize={selectInteger}
      />
    </Section>
  );
}
