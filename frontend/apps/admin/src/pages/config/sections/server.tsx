import type { FormCtx } from '../schema';
import { Section, SwitchRow, TextRow } from '../rows';
import { parseBackendInteger } from '../values';

export function ServerSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="节点">
      <TextRow
        ctx={ctx}
        group="server"
        field="server_api_url"
        title="节点对接API地址"
        description="v2node节点一键对接专用地址。"
        placeholder="请输入"
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_token"
        title="通讯密钥"
        description="V2board与节点通讯的密钥，以便数据不会被他人获取。"
        placeholder="请输入"
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_pull_interval"
        title="节点拉取动作轮询间隔"
        description="节点从面板获取数据的间隔频率。"
        placeholder="请输入"
        type="number"
        suffix="秒"
        coerce={parseBackendInteger}
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_push_interval"
        title="节点推送动作轮询间隔"
        description="节点推送数据到面板的间隔频率。"
        placeholder="请输入"
        type="number"
        suffix="秒"
        coerce={parseBackendInteger}
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_node_report_min_traffic"
        title="节点用户流量上报最低阈值"
        description="每次推送动作仅累计使用流量高于阈值的用户信息会被上报，未上报流量会累计"
        placeholder="请输入"
        type="number"
        suffix="Kb"
        coerce={parseBackendInteger}
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_device_online_min_traffic"
        title="节点用户设备数统计最低阈值"
        description="每次推送动作仅上报流量高于阈值的在线设备IP地址会被节点统计"
        placeholder="请输入"
        type="number"
        suffix="Kb"
        coerce={parseBackendInteger}
      />
      <SwitchRow
        ctx={ctx}
        group="server"
        field="device_limit_mode"
        title="全局设备数限制采用宽松模式"
        description="开启后同一IP地址使用多个节点只统计为一个设备"
      />
    </Section>
  );
}
