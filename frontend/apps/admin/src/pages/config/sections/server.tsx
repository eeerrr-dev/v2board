import { useTranslation } from 'react-i18next';
import type { FormCtx } from '../schema';
import { Section, SwitchRow, TextRow } from '../rows';

export function ServerSection({ ctx }: { ctx: FormCtx }) {
  const { t } = useTranslation();
  return (
    <Section title={t(($) => $.admin.config.sections.server)}>
      <TextRow
        ctx={ctx}
        group="server"
        field="server_api_url"
        title={t(($) => $.admin.config.server.api_url_title)}
        description={t(($) => $.admin.config.server.api_url_desc)}
        placeholder={t(($) => $.admin.config.input_placeholder)}
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_token"
        title={t(($) => $.admin.config.server.token_title)}
        description={t(($) => $.admin.config.server.token_desc)}
        placeholder={t(($) => $.admin.config.input_placeholder)}
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_pull_interval"
        title={t(($) => $.admin.config.server.pull_interval_title)}
        description={t(($) => $.admin.config.server.pull_interval_desc)}
        placeholder={t(($) => $.admin.config.input_placeholder)}
        type="number"
        suffix={t(($) => $.admin.config.server.seconds_suffix)}
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_push_interval"
        title={t(($) => $.admin.config.server.push_interval_title)}
        description={t(($) => $.admin.config.server.push_interval_desc)}
        placeholder={t(($) => $.admin.config.input_placeholder)}
        type="number"
        suffix={t(($) => $.admin.config.server.seconds_suffix)}
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_node_report_min_traffic"
        title={t(($) => $.admin.config.server.report_min_traffic_title)}
        description={t(($) => $.admin.config.server.report_min_traffic_desc)}
        placeholder={t(($) => $.admin.config.input_placeholder)}
        type="number"
        suffix="Kb"
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_device_online_min_traffic"
        title={t(($) => $.admin.config.server.online_min_traffic_title)}
        description={t(($) => $.admin.config.server.online_min_traffic_desc)}
        placeholder={t(($) => $.admin.config.input_placeholder)}
        type="number"
        suffix="Kb"
      />
      <SwitchRow
        ctx={ctx}
        group="server"
        field="device_limit_mode"
        title={t(($) => $.admin.config.server.device_limit_mode_title)}
        description={t(($) => $.admin.config.server.device_limit_mode_desc)}
      />
    </Section>
  );
}
