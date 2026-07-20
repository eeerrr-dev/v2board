import { useMemo } from 'react';
import { type FieldPath } from 'react-hook-form';
import { useTranslation } from 'react-i18next';
import { ExternalLink } from 'lucide-react';
import type { admin } from '@v2board/api-client';
import { HeaderTooltip } from '@v2board/ui/header-tooltip';
import { Input } from '@v2board/ui/input';
import { Label } from '@v2board/ui/label';
import { Switch } from '@v2board/ui/switch';
import { Textarea } from '@v2board/ui/textarea';
import {
  ANYTLS_PADDING_SCHEME_PLACEHOLDER,
  ENCRYPTION_MODE_OPTIONS,
  ENCRYPTION_RTT_OPTIONS,
  ENCRYPTION_SETTINGS_DEFAULTS,
  HYSTERIA_VERSION_OPTIONS,
  PROXY_PROTOCOL_OPTIONS,
  SECURITY_REALITY_OPTION,
  SECURITY_TLS_OPTION,
  SHADOWSOCKS_CIPHER_OPTIONS,
  STREAM_NETWORK_OPTIONS,
  TLS_FINGERPRINT_OPTIONS,
  TLS_SETTINGS_DEFAULTS,
  TROJAN_NETWORK_OPTIONS,
  TUIC_CONGESTION_CONTROL_OPTIONS,
  TUIC_RELAY_MODE_OPTIONS,
  V2NODE_PROTOCOLS,
  V2NODE_PROTOCOL_OPTIONS,
  getBinarySelectOptions,
  getBinarySelectValue,
  getEchModeOptions,
  getHysteria2ObfsOptions,
  getHysteriaV1ObfsOptions,
  getNetworkSettingsPlaceholder,
  getNumericSelectValue,
  getSecurityNoneOption,
  getShadowsocksObfsOptions,
  getTlsCertModeOptions,
  getTlsSupportOptions,
  getV2nodeSecurityOptions,
  getV2nodeSecurityValue,
  getV2nodeShadowsocksNetworkOptions,
  getV2nodeTransportOptions,
  getVlessEncryptionOptions,
  getVlessFlowOptions,
  getVlessFlowOptionsForNetwork,
  normalizeSettings,
  settingValue,
  settingsObject,
  withSetting,
} from './domain';
import { NodeFieldError, NodeSelect, type NodeAdvancedField, type NodeForm } from './form-controls';
import { switchV2nodeProtocol, type ServerNodeEditorValues } from './form-schema';
import {
  binaryValue,
  displayText,
  hysteriaVersion,
  inputValue,
  nullableText,
  securityValue,
  selectValue,
  shadowsocksCipher,
  shadowsocksObfs,
  toBoolean,
  trojanNetwork,
  v2nodeNetwork,
  vlessFlow,
  vmessNetwork,
} from './node-values';

export function NodeAddressFields({
  form,
  showChildDrawer,
}: {
  form: NodeForm;
  showChildDrawer: (title?: string, field?: NodeAdvancedField) => void;
}) {
  const { t } = useTranslation();
  const { values, setField, setFieldOptions } = form;
  if (values.type === 'v2node') {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className="space-y-2">
          <Label htmlFor="node-host">{t(($) => $.admin.servers.connect_address)}</Label>
          <Input
            id="node-host"
            placeholder={t(($) => $.admin.servers.host_ip_placeholder)}
            value={inputValue(values.host)}
            onChange={(event) => setField('host', event.target.value, setFieldOptions)}
            data-testid="node-host"
          />
          <NodeFieldError form={form} name="host" />
        </div>
        <div className="space-y-2">
          <Label htmlFor="node-listen-ip">{t(($) => $.admin.servers.listen_address)}</Label>
          <Input
            id="node-listen-ip"
            placeholder={t(($) => $.admin.servers.listen_ip_placeholder)}
            value={inputValue(values.listen_ip)}
            onChange={(event) => setField('listen_ip', event.target.value, setFieldOptions)}
            data-testid="node-listen-ip"
          />
        </div>
      </div>
    );
  }
  if (values.type === 'vmess' || values.type === 'vless') {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <div className="space-y-2 sm:col-span-2">
          <Label htmlFor="node-host">{t(($) => $.admin.servers.node_address)}</Label>
          <Input
            id="node-host"
            placeholder={t(($) => $.admin.servers.connect_address_placeholder)}
            value={inputValue(values.host)}
            onChange={(event) => setField('host', event.target.value, setFieldOptions)}
            data-testid="node-host"
          />
          <NodeFieldError form={form} name="host" />
        </div>
        {values.type === 'vmess' ? (
          <VmessTlsField form={form} showChildDrawer={showChildDrawer} />
        ) : (
          <VlessSecurityField form={form} showChildDrawer={showChildDrawer} />
        )}
      </div>
    );
  }
  return (
    <div className="space-y-2">
      <Label htmlFor="node-host">{t(($) => $.admin.servers.node_address)}</Label>
      <Input
        id="node-host"
        placeholder={t(($) => $.admin.servers.host_ip_placeholder)}
        value={inputValue(values.host)}
        onChange={(event) => setField('host', event.target.value, setFieldOptions)}
        data-testid="node-host"
      />
      <NodeFieldError form={form} name="host" />
    </div>
  );
}

export function NodePortFields({ form }: { form: NodeForm }) {
  const { t } = useTranslation();
  const { values, setField, setFieldOptions } = form;
  const type = values.type;
  const portInput = (
    name: 'port' | 'server_port',
    label: string,
    placeholder: string,
    testId: string,
  ) => (
    <div className="space-y-2">
      <Label htmlFor={testId}>{label}</Label>
      <Input
        id={testId}
        placeholder={placeholder}
        value={inputValue(values[name])}
        onChange={(event) => setField(name, event.target.value, setFieldOptions)}
        data-testid={testId}
      />
      <NodeFieldError form={form} name={name} />
    </div>
  );

  const connectPortLabel = t(($) => $.admin.servers.connect_port);
  const connectPortPlaceholder = t(($) => $.admin.servers.connect_port_placeholder);
  const serverPortLabel = t(($) => $.admin.servers.server_port);
  const serverPortPlaceholder = t(($) => $.admin.servers.server_port_placeholder);

  if (type === 'trojan' || type === 'hysteria' || type === 'tuic' || type === 'anytls') {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        {portInput('port', connectPortLabel, connectPortPlaceholder, 'node-port')}
        {portInput('server_port', serverPortLabel, serverPortPlaceholder, 'node-server-port')}
        {type === 'trojan' ? (
          <TrojanAllowInsecureField form={form} />
        ) : (
          <ServerInsecureField form={form} />
        )}
      </div>
    );
  }
  if (type === 'v2node') {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        {portInput('port', connectPortLabel, connectPortPlaceholder, 'node-port')}
        {portInput('server_port', serverPortLabel, serverPortPlaceholder, 'node-server-port')}
      </div>
    );
  }
  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
      {portInput('port', connectPortLabel, connectPortPlaceholder, 'node-port')}
      {portInput(
        'server_port',
        serverPortLabel,
        t(($) => $.admin.servers.server_port_nat_placeholder),
        'node-server-port',
      )}
    </div>
  );
}

function ChildFieldLink({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button type="button" className="text-sm text-primary" onClick={onClick}>
      {label}
    </button>
  );
}

function TrojanAllowInsecureField({ form }: { form: NodeForm }) {
  const { t } = useTranslation();
  if (form.values.type !== 'trojan') return null;
  return (
    <div className="space-y-2">
      <Label htmlFor="node-allow-insecure">
        <HeaderTooltip title={t(($) => $.admin.servers.insecure_tip)}>
          {t(($) => $.admin.servers.allow_insecure)}
        </HeaderTooltip>
      </Label>
      <NodeSelect
        value={getBinarySelectValue(form.values.allow_insecure)}
        options={getBinarySelectOptions(t)}
        placeholder={t(($) => $.admin.servers.allow_insecure)}
        onChange={(value) =>
          form.setField('allow_insecure', binaryValue(value, 0), form.setFieldOptions)
        }
        testId="node-allow-insecure"
      />
    </div>
  );
}

function ServerInsecureField({ form }: { form: NodeForm }) {
  const { t } = useTranslation();
  if (!('insecure' in form.values)) return null;
  return (
    <div className="space-y-2">
      <Label htmlFor="node-insecure">
        <HeaderTooltip title={t(($) => $.admin.servers.insecure_tip)}>
          {t(($) => $.admin.servers.allow_insecure)}
        </HeaderTooltip>
      </Label>
      <NodeSelect
        value={getBinarySelectValue(form.values.insecure)}
        options={getBinarySelectOptions(t)}
        placeholder={t(($) => $.admin.servers.allow_insecure)}
        onChange={(value) => form.setField('insecure', binaryValue(value, 0), form.setFieldOptions)}
        testId="node-insecure"
      />
    </div>
  );
}

function VmessTlsField({
  form,
  showChildDrawer,
}: {
  form: NodeForm;
  showChildDrawer: (title?: string, field?: NodeAdvancedField) => void;
}) {
  const { t } = useTranslation();
  if (form.values.type !== 'vmess') return null;
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <Label htmlFor="node-tls">TLS</Label>
        <ChildFieldLink
          label={t(($) => $.admin.servers.edit_config)}
          onClick={() =>
            showChildDrawer(
              t(($) => $.admin.servers.edit_tls_config),
              'tlsSettings',
            )
          }
        />
      </div>
      <NodeSelect
        value={getBinarySelectValue(form.values.tls)}
        options={getTlsSupportOptions(t)}
        placeholder={t(($) => $.admin.servers.tls_support_placeholder)}
        onChange={(value) => form.setField('tls', binaryValue(value, 0), form.setFieldOptions)}
        testId="node-tls"
      />
    </div>
  );
}

function VlessSecurityField({
  form,
  showChildDrawer,
}: {
  form: NodeForm;
  showChildDrawer: (title?: string, field?: NodeAdvancedField) => void;
}) {
  const { t } = useTranslation();
  if (form.values.type !== 'vless') return null;
  const security = form.values.tls;
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <Label htmlFor="node-vless-security">{t(($) => $.admin.servers.security)}</Label>
        {parseInt(String(security ?? 0), 10) !== 0 ? (
          <ChildFieldLink
            label={t(($) => $.admin.servers.edit_config)}
            onClick={() =>
              showChildDrawer(
                t(($) => $.admin.servers.edit_security_config),
                'tls_settings',
              )
            }
          />
        ) : null}
      </div>
      <NodeSelect
        value={getNumericSelectValue(form.values.tls)}
        options={[getSecurityNoneOption(t), SECURITY_TLS_OPTION, SECURITY_REALITY_OPTION]}
        onChange={(value) => form.setField('tls', securityValue(value, 0), form.setFieldOptions)}
        testId="node-vless-security"
      />
    </div>
  );
}

function V2nodeFields({
  form,
  showChildDrawer,
}: {
  form: NodeForm;
  showChildDrawer: (title?: string, field?: NodeAdvancedField) => void;
}) {
  const { t } = useTranslation();
  const { values, setField, setFieldOptions, replaceValues } = form;
  if (values.type !== 'v2node') return null;
  const config = values.config;
  const protocolValue = config.protocol || null;
  const selectedSecurity = getV2nodeSecurityValue(protocolValue, config.tls);

  const changeProtocol = (value: string | number | null) => {
    const protocol = V2NODE_PROTOCOLS.find((candidate) => candidate === value);
    if (protocol) replaceValues(switchV2nodeProtocol(values, protocol));
  };

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className="space-y-2">
          <Label htmlFor="node-protocol">{t(($) => $.admin.servers.node_protocol)}</Label>
          <NodeSelect
            value={protocolValue}
            options={V2NODE_PROTOCOL_OPTIONS}
            onChange={changeProtocol}
            testId="node-protocol"
          />
          <NodeFieldError form={form} name="config.protocol" />
        </div>
        {config.protocol !== '' && config.protocol !== 'shadowsocks' ? (
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <Label htmlFor="node-v2node-security">{t(($) => $.admin.servers.security)}</Label>
              {selectedSecurity ? (
                <ChildFieldLink
                  label={t(($) => $.admin.servers.edit_config)}
                  onClick={() =>
                    showChildDrawer(
                      t(($) => $.admin.servers.edit_security_config),
                      'tls_settings',
                    )
                  }
                />
              ) : null}
            </div>
            <NodeSelect
              value={getV2nodeSecurityValue(protocolValue, config.tls)}
              options={getV2nodeSecurityOptions(t, protocolValue)}
              onChange={(value) => setField('config.tls', securityValue(value, 0), setFieldOptions)}
              testId="node-v2node-security"
            />
          </div>
        ) : null}
      </div>

      {config.protocol === 'shadowsocks' ? (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Label htmlFor="node-v2node-network">{t(($) => $.admin.servers.transport)}</Label>
            <ChildFieldLink
              label={t(($) => $.admin.servers.edit_config)}
              onClick={() =>
                showChildDrawer(
                  t(($) => $.admin.servers.edit_protocol_config),
                  'network_settings',
                )
              }
            />
          </div>
          <NodeSelect
            value={config.network}
            options={getV2nodeShadowsocksNetworkOptions(t)}
            placeholder={t(($) => $.admin.servers.transport_placeholder)}
            onChange={(value) => setField('config.network', v2nodeNetwork(value), setFieldOptions)}
            testId="node-v2node-network"
          />
          <NodeFieldError form={form} name="config.network" />
        </div>
      ) : null}

      {config.protocol !== '' &&
      config.protocol !== 'hysteria2' &&
      config.protocol !== 'shadowsocks' &&
      config.protocol !== 'tuic' ? (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Label htmlFor="node-v2node-network">{t(($) => $.admin.servers.transport)}</Label>
            <ChildFieldLink
              label={t(($) => $.admin.servers.edit_config)}
              onClick={() =>
                showChildDrawer(
                  t(($) => $.admin.servers.edit_protocol_config),
                  'network_settings',
                )
              }
            />
          </div>
          <NodeSelect
            value={config.network}
            options={getV2nodeTransportOptions(t, protocolValue)}
            placeholder={t(($) => $.admin.servers.transport_placeholder)}
            onChange={(value) => setField('config.network', v2nodeNetwork(value), setFieldOptions)}
            testId="node-v2node-network"
          />
          <NodeFieldError form={form} name="config.network" />
        </div>
      ) : null}

      {config.protocol === 'anytls' ? (
        <div>
          <ChildFieldLink
            label={t(($) => $.admin.servers.edit_padding_scheme)}
            onClick={() =>
              showChildDrawer(
                t(($) => $.admin.servers.edit_padding_scheme),
                'padding_scheme',
              )
            }
          />
        </div>
      ) : null}

      {config.protocol === 'hysteria2' ? (
        <>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="node-obfs">{t(($) => $.admin.servers.obfs_mode)}</Label>
              <NodeSelect
                value={config.obfs ?? null}
                options={getHysteria2ObfsOptions(t)}
                onChange={(value) => setField('config.obfs', nullableText(value), setFieldOptions)}
                testId="node-obfs"
              />
            </div>
            {config.obfs === 'salamander' ? (
              <div className="space-y-2">
                <Label htmlFor="node-obfs-password">
                  {t(($) => $.admin.servers.obfs_password)}
                </Label>
                <Input
                  id="node-obfs-password"
                  placeholder={t(($) => $.admin.servers.auto_generate_placeholder)}
                  value={inputValue(config.obfs_password)}
                  onChange={(event) =>
                    setField('config.obfs_password', event.target.value, setFieldOptions)
                  }
                />
              </div>
            ) : null}
          </div>
          <BandwidthFields form={form} />
        </>
      ) : null}

      {config.protocol === 'tuic' ? (
        <>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="node-disable-sni">{t(($) => $.admin.servers.disable_sni)}</Label>
              <NodeSelect
                value={getBinarySelectValue(config.disable_sni)}
                options={getBinarySelectOptions(t)}
                onChange={(value) =>
                  setField('config.disable_sni', binaryValue(value, 0), setFieldOptions)
                }
                testId="node-disable-sni"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="node-udp-relay-mode">
                {t(($) => $.admin.servers.udp_relay_mode)}
              </Label>
              <NodeSelect
                value={config.udp_relay_mode ?? 'native'}
                options={TUIC_RELAY_MODE_OPTIONS}
                onChange={(value) =>
                  setField('config.udp_relay_mode', nullableText(value), setFieldOptions)
                }
                testId="node-udp-relay-mode"
              />
            </div>
          </div>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="node-congestion-control">
                {t(($) => $.admin.servers.congestion_control)}
              </Label>
              <NodeSelect
                value={config.congestion_control ?? 'cubic'}
                options={TUIC_CONGESTION_CONTROL_OPTIONS}
                onChange={(value) =>
                  setField('config.congestion_control', nullableText(value), setFieldOptions)
                }
                testId="node-congestion-control"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="node-zero-rtt">{t(($) => $.admin.servers.zero_rtt)}</Label>
              <NodeSelect
                value={getBinarySelectValue(config.zero_rtt_handshake)}
                options={getBinarySelectOptions(t)}
                onChange={(value) =>
                  setField('config.zero_rtt_handshake', binaryValue(value, 0), setFieldOptions)
                }
                testId="node-zero-rtt"
              />
            </div>
          </div>
        </>
      ) : null}

      {config.protocol === 'shadowsocks' ? (
        <div className="space-y-2">
          <Label htmlFor="node-cipher">{t(($) => $.admin.servers.cipher)}</Label>
          <NodeSelect
            value={config.cipher ?? 'aes-128-gcm'}
            options={SHADOWSOCKS_CIPHER_OPTIONS}
            onChange={(value) => setField('config.cipher', nullableText(value), setFieldOptions)}
            testId="node-cipher"
          />
          <NodeFieldError form={form} name="config.cipher" />
        </div>
      ) : null}

      {config.protocol === 'vless' ? (
        <>
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <Label htmlFor="node-encryption">{t(($) => $.admin.servers.encryption)}</Label>
              {config.encryption ? (
                <ChildFieldLink
                  label={t(($) => $.admin.servers.edit_config)}
                  onClick={() =>
                    showChildDrawer(
                      t(($) => $.admin.servers.edit_encryption_config),
                      'encryption_settings',
                    )
                  }
                />
              ) : null}
            </div>
            <NodeSelect
              value={config.encryption ?? null}
              options={getVlessEncryptionOptions(t)}
              placeholder={t(($) => $.admin.servers.encryption_placeholder)}
              onChange={(value) =>
                setField('config.encryption', nullableText(value), setFieldOptions)
              }
              testId="node-encryption"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="node-flow">{t(($) => $.admin.servers.flow)}</Label>
            <NodeSelect
              value={config.flow ?? null}
              options={getVlessFlowOptions(t)}
              placeholder={t(($) => $.admin.servers.flow_placeholder)}
              onChange={(value) => setField('config.flow', vlessFlow(value), setFieldOptions)}
              testId="node-flow"
            />
            <NodeFieldError form={form} name="config.flow" />
          </div>
        </>
      ) : null}
    </div>
  );
}

function BandwidthFields({ form }: { form: NodeForm }) {
  const { t } = useTranslation();
  const { values, setField, setFieldOptions } = form;
  const field = (name: 'up_mbps' | 'down_mbps', label: string, placeholder: string) => {
    const v2nodeConfig = values.type === 'v2node' ? values.config : undefined;
    const value =
      v2nodeConfig?.protocol === 'hysteria2'
        ? v2nodeConfig[name]
        : values.type === 'hysteria'
          ? values[name]
          : undefined;
    const change = (next: string) => {
      if (values.type === 'v2node' && values.config.protocol === 'hysteria2') {
        if (name === 'up_mbps') setField('config.up_mbps', next, setFieldOptions);
        else setField('config.down_mbps', next, setFieldOptions);
      } else if (values.type === 'hysteria') {
        if (name === 'up_mbps') setField('up_mbps', next, setFieldOptions);
        else setField('down_mbps', next, setFieldOptions);
      }
    };
    return (
      <div className="space-y-2">
        <Label htmlFor={`node-${name}`}>{label}</Label>
        <div className="relative">
          <Input
            id={`node-${name}`}
            className="pr-16"
            placeholder={placeholder}
            value={inputValue(value)}
            onChange={(event) => change(event.target.value)}
          />
          <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
            Mbps
          </span>
        </div>
        {values.type === 'v2node' ? (
          <NodeFieldError
            form={form}
            name={name === 'up_mbps' ? 'config.up_mbps' : 'config.down_mbps'}
          />
        ) : (
          <NodeFieldError form={form} name={name} />
        )}
      </div>
    );
  };
  return (
    <>
      {field(
        'up_mbps',
        t(($) => $.admin.servers.up_mbps),
        t(($) => $.admin.servers.up_mbps_placeholder),
      )}
      {field(
        'down_mbps',
        t(($) => $.admin.servers.down_mbps),
        t(($) => $.admin.servers.down_mbps_placeholder),
      )}
    </>
  );
}

export function ServerTypeFields({
  editing,
  form,
  showChildDrawer,
}: {
  editing: boolean;
  form: NodeForm;
  showChildDrawer: (title?: string, field?: NodeAdvancedField) => void;
}) {
  const { t } = useTranslation();
  const { values, setField, setFieldOptions } = form;

  if (values.type === 'v2node') {
    return <V2nodeFields form={form} showChildDrawer={showChildDrawer} />;
  }

  if (values.type === 'shadowsocks') {
    const selectedObfs = values.obfs;
    const obfsSettings = settingsObject(values.obfs_settings) ?? {};
    return (
      <div className="space-y-5">
        <div className="space-y-2">
          <Label htmlFor="node-cipher">{t(($) => $.admin.servers.cipher)}</Label>
          <NodeSelect
            value={values.cipher ?? (editing ? undefined : 'chacha20-ietf-poly1305')}
            options={SHADOWSOCKS_CIPHER_OPTIONS}
            onChange={(value) => setField('cipher', shadowsocksCipher(value), setFieldOptions)}
            testId="node-cipher"
          />
          <NodeFieldError form={form} name="cipher" />
        </div>
        <div className="space-y-2">
          <Label htmlFor="node-obfs">{t(($) => $.admin.servers.obfs)}</Label>
          <NodeSelect
            value={values.obfs ?? ''}
            options={getShadowsocksObfsOptions(t)}
            onChange={(value) => setField('obfs', shadowsocksObfs(value), setFieldOptions)}
            testId="node-obfs"
          />
          {selectedObfs === 'http' ? (
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              <div className="space-y-2">
                <Label htmlFor="node-obfs-path">{t(($) => $.admin.servers.path)}</Label>
                <Input
                  id="node-obfs-path"
                  placeholder={t(($) => $.admin.servers.path)}
                  value={inputValue(settingValue(obfsSettings, 'path'))}
                  onChange={(event) =>
                    setField(
                      'obfs_settings',
                      withSetting(obfsSettings, 'path', event.target.value),
                      setFieldOptions,
                    )
                  }
                  data-testid="node-obfs-path"
                />
              </div>
              <div className="space-y-2 sm:col-span-2">
                <Label htmlFor="node-obfs-host">Host</Label>
                <Input
                  id="node-obfs-host"
                  placeholder="Host"
                  value={inputValue(settingValue(obfsSettings, 'host'))}
                  onChange={(event) =>
                    setField(
                      'obfs_settings',
                      withSetting(obfsSettings, 'host', event.target.value),
                      setFieldOptions,
                    )
                  }
                  data-testid="node-obfs-host"
                />
              </div>
            </div>
          ) : null}
          <NodeFieldError form={form} name="obfs" />
        </div>
      </div>
    );
  }

  if (values.type === 'vmess') {
    return (
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Label htmlFor="node-network">{t(($) => $.admin.servers.transport)}</Label>
          <ChildFieldLink
            label={t(($) => $.admin.servers.edit_config)}
            onClick={() =>
              showChildDrawer(
                t(($) => $.admin.servers.edit_protocol_config),
                'networkSettings',
              )
            }
          />
        </div>
        <NodeSelect
          value={values.network}
          options={STREAM_NETWORK_OPTIONS}
          placeholder={t(($) => $.admin.servers.transport_placeholder)}
          onChange={(value) => setField('network', vmessNetwork(value), setFieldOptions)}
          testId="node-network"
        />
        <NodeFieldError form={form} name="network" />
      </div>
    );
  }

  if (values.type === 'trojan') {
    return (
      <div className="space-y-5">
        <div className="space-y-2">
          <Label htmlFor="node-server-name">{t(($) => $.admin.servers.sni)}</Label>
          <Input
            id="node-server-name"
            placeholder={t(($) => $.admin.servers.sni_placeholder)}
            value={inputValue(values.server_name)}
            onChange={(event) => setField('server_name', event.target.value, setFieldOptions)}
            data-testid="node-server-name"
          />
        </div>
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Label htmlFor="node-network">{t(($) => $.admin.servers.transport)}</Label>
            <ChildFieldLink
              label={t(($) => $.admin.servers.edit_config)}
              onClick={() =>
                showChildDrawer(
                  t(($) => $.admin.servers.edit_protocol_config),
                  'network_settings',
                )
              }
            />
          </div>
          <NodeSelect
            value={values.network}
            options={TROJAN_NETWORK_OPTIONS}
            placeholder={t(($) => $.admin.servers.transport_placeholder)}
            onChange={(value) => setField('network', trojanNetwork(value), setFieldOptions)}
            testId="node-network"
          />
          <NodeFieldError form={form} name="network" />
        </div>
      </div>
    );
  }

  if (values.type === 'tuic') {
    const tuicDisableSni = values.disable_sni;
    return (
      <div className="space-y-5">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <Label htmlFor="node-disable-sni">{t(($) => $.admin.servers.disable_sni)}</Label>
            <NodeSelect
              value={getBinarySelectValue(values.disable_sni)}
              options={getBinarySelectOptions(t)}
              onChange={(value) => setField('disable_sni', binaryValue(value, 0), setFieldOptions)}
              testId="node-disable-sni"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="node-udp-relay-mode">{t(($) => $.admin.servers.udp_relay_mode)}</Label>
            <NodeSelect
              value={values.udp_relay_mode ?? 'native'}
              options={TUIC_RELAY_MODE_OPTIONS}
              onChange={(value) => setField('udp_relay_mode', nullableText(value), setFieldOptions)}
              testId="node-udp-relay-mode"
            />
          </div>
        </div>
        {parseInt(String(tuicDisableSni ?? 0), 10) ? null : (
          <div className="space-y-2">
            <Label htmlFor="node-server-name">{t(($) => $.admin.servers.sni)}</Label>
            <Input
              id="node-server-name"
              placeholder={t(($) => $.admin.servers.sni_placeholder)}
              value={inputValue(values.server_name)}
              onChange={(event) => setField('server_name', event.target.value, setFieldOptions)}
              data-testid="node-server-name"
            />
          </div>
        )}
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <Label htmlFor="node-congestion-control">
              {t(($) => $.admin.servers.congestion_control)}
            </Label>
            <NodeSelect
              value={values.congestion_control ?? 'cubic'}
              options={TUIC_CONGESTION_CONTROL_OPTIONS}
              onChange={(value) =>
                setField('congestion_control', nullableText(value), setFieldOptions)
              }
              testId="node-congestion-control"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="node-zero-rtt">{t(($) => $.admin.servers.zero_rtt)}</Label>
            <NodeSelect
              value={getBinarySelectValue(values.zero_rtt_handshake)}
              options={getBinarySelectOptions(t)}
              onChange={(value) =>
                setField('zero_rtt_handshake', binaryValue(value, 0), setFieldOptions)
              }
              testId="node-zero-rtt"
            />
          </div>
        </div>
      </div>
    );
  }

  if (values.type === 'vless') {
    const vlessNetwork = values.network;
    const vlessEncryption = values.encryption;
    return (
      <div className="space-y-5">
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Label htmlFor="node-network">{t(($) => $.admin.servers.transport)}</Label>
            <ChildFieldLink
              label={t(($) => $.admin.servers.edit_config)}
              onClick={() =>
                showChildDrawer(
                  t(($) => $.admin.servers.edit_protocol_config),
                  'network_settings',
                )
              }
            />
          </div>
          <NodeSelect
            value={values.network}
            options={STREAM_NETWORK_OPTIONS}
            placeholder={t(($) => $.admin.servers.transport_placeholder)}
            onChange={(value) =>
              setField('network', typeof value === 'string' ? value : '', setFieldOptions)
            }
            testId="node-network"
          />
          <NodeFieldError form={form} name="network" />
        </div>
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Label htmlFor="node-encryption">{t(($) => $.admin.servers.encryption)}</Label>
            {vlessEncryption ? (
              <ChildFieldLink
                label={t(($) => $.admin.servers.edit_config)}
                onClick={() =>
                  showChildDrawer(
                    t(($) => $.admin.servers.edit_encryption_config),
                    'encryption_settings',
                  )
                }
              />
            ) : null}
          </div>
          <NodeSelect
            value={values.encryption ?? null}
            options={getVlessEncryptionOptions(t)}
            placeholder={t(($) => $.admin.servers.encryption_placeholder)}
            onChange={(value) => setField('encryption', nullableText(value), setFieldOptions)}
            testId="node-encryption"
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="node-flow">{t(($) => $.admin.servers.flow)}</Label>
          <NodeSelect
            value={values.flow ?? null}
            options={getVlessFlowOptionsForNetwork(t, vlessNetwork)}
            placeholder={t(($) => $.admin.servers.flow_placeholder)}
            onChange={(value) => setField('flow', vlessFlow(value), setFieldOptions)}
            testId="node-flow"
          />
          <NodeFieldError form={form} name="flow" />
        </div>
      </div>
    );
  }

  if (values.type === 'hysteria') {
    const version = parseInt(String(values.version ?? 1), 10);
    const obfs = values.obfs == null ? null : String(values.obfs);
    return (
      <div className="space-y-5">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-4">
          <div className="space-y-2">
            <Label htmlFor="node-version">{t(($) => $.admin.servers.hysteria_version)}</Label>
            <NodeSelect
              value={getNumericSelectValue(values.version, 1)}
              options={HYSTERIA_VERSION_OPTIONS}
              onChange={(value) => setField('version', hysteriaVersion(value), setFieldOptions)}
              testId="node-version"
            />
          </div>
        </div>
        <div className="space-y-2">
          <Label htmlFor="node-server-name">{t(($) => $.admin.servers.sni)}</Label>
          <Input
            id="node-server-name"
            placeholder={t(($) => $.admin.servers.sni_placeholder)}
            value={inputValue(values.server_name)}
            onChange={(event) => setField('server_name', event.target.value, setFieldOptions)}
            data-testid="node-server-name"
          />
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          {version === 1 ? (
            <div className="space-y-2">
              <Label htmlFor="node-obfs">{t(($) => $.admin.servers.obfs_mode)}</Label>
              <NodeSelect
                value={values.obfs ?? null}
                options={getHysteriaV1ObfsOptions(t)}
                onChange={(value) => setField('obfs', nullableText(value), setFieldOptions)}
                testId="node-obfs"
              />
            </div>
          ) : null}
          {version === 1 && obfs === 'xplus' ? (
            <div className="space-y-2">
              <Label htmlFor="node-obfs-password">{t(($) => $.admin.servers.obfs_param)}</Label>
              <Input
                id="node-obfs-password"
                placeholder={t(($) => $.admin.servers.auto_generate_placeholder)}
                value={inputValue(values.obfs_password)}
                onChange={(event) => setField('obfs_password', event.target.value, setFieldOptions)}
              />
            </div>
          ) : null}
          {version === 2 ? (
            <div className="space-y-2">
              <Label htmlFor="node-obfs">{t(($) => $.admin.servers.obfs_mode)}</Label>
              <NodeSelect
                value={values.obfs ?? null}
                options={getHysteria2ObfsOptions(t)}
                onChange={(value) => setField('obfs', nullableText(value), setFieldOptions)}
                testId="node-obfs"
              />
            </div>
          ) : null}
          {version === 2 && obfs === 'salamander' ? (
            <div className="space-y-2">
              <Label htmlFor="node-obfs-password">{t(($) => $.admin.servers.obfs_password)}</Label>
              <Input
                id="node-obfs-password"
                placeholder={t(($) => $.admin.servers.auto_generate_placeholder)}
                value={inputValue(values.obfs_password)}
                onChange={(event) => setField('obfs_password', event.target.value, setFieldOptions)}
              />
            </div>
          ) : null}
        </div>
        <BandwidthFields form={form} />
      </div>
    );
  }

  if (values.type === 'anytls') {
    return (
      <div className="space-y-5">
        <div className="space-y-2">
          <Label htmlFor="node-server-name">{t(($) => $.admin.servers.sni)}</Label>
          <Input
            id="node-server-name"
            placeholder={t(($) => $.admin.servers.sni_placeholder)}
            value={inputValue(values.server_name)}
            onChange={(event) => setField('server_name', event.target.value, setFieldOptions)}
            data-testid="node-server-name"
          />
        </div>
        <div>
          <ChildFieldLink
            label={t(($) => $.admin.servers.edit_padding_scheme)}
            onClick={() =>
              showChildDrawer(
                t(($) => $.admin.servers.edit_padding_scheme),
                'padding_scheme',
              )
            }
          />
        </div>
      </div>
    );
  }

  return null;
}

// ---------------------------------------------------------------------------
// Child config drawers.
// ---------------------------------------------------------------------------

export function NodeChildField({ field, form }: { field: NodeAdvancedField; form: NodeForm }) {
  const { t } = useTranslation();
  const { values, setField, setFieldOptions } = form;

  const networkSettingsEditor = (
    type: admin.ServerTypeName,
    network: unknown,
    value: unknown,
    onChange: (value: string) => void,
    errorName: FieldPath<ServerNodeEditorValues>,
  ) => (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <Label htmlFor="node-network-settings">{t(($) => $.admin.servers.protocol_settings)}</Label>
        <a
          className="inline-flex items-center gap-1 text-sm text-primary"
          href="https://www.v2ray.com/chapter_02/05_transport.html"
          target="_blank"
          rel="noopener noreferrer"
        >
          <ExternalLink className="size-3.5" />
          {t(($) => $.admin.servers.reference)}
        </a>
      </div>
      <Textarea
        id="node-network-settings"
        rows={12}
        className="font-mono text-xs"
        placeholder={getNetworkSettingsPlaceholder(type, network)}
        value={inputValue(value)}
        onChange={(event) => onChange(event.target.value)}
        data-testid="node-network-settings"
      />
      <NodeFieldError form={form} name={errorName} />
    </div>
  );

  if (field === 'network_settings' || field === 'networkSettings') {
    if (values.type === 'v2node') {
      return networkSettingsEditor(
        values.type,
        values.config.network,
        values.config.network_settings,
        (value) => setField('config.network_settings', value, setFieldOptions),
        'config.network_settings',
      );
    }
    if (values.type === 'vmess' && field === 'networkSettings') {
      return networkSettingsEditor(
        values.type,
        values.network,
        values.networkSettings,
        (value) => setField('networkSettings', value, setFieldOptions),
        'networkSettings',
      );
    }
    if ((values.type === 'trojan' || values.type === 'vless') && field === 'network_settings') {
      return networkSettingsEditor(
        values.type,
        values.network,
        values.network_settings,
        (value) => setField('network_settings', value, setFieldOptions),
        'network_settings',
      );
    }
    return null;
  }

  if (field === 'padding_scheme') {
    const paddingScheme =
      values.type === 'v2node' && values.config.protocol === 'anytls'
        ? values.config.padding_scheme
        : values.type === 'anytls'
          ? values.padding_scheme
          : undefined;
    return (
      <div className="space-y-2">
        <Label htmlFor="node-padding-scheme">{t(($) => $.admin.servers.padding_scheme)}</Label>
        <Textarea
          id="node-padding-scheme"
          rows={12}
          className="font-mono text-xs"
          placeholder={ANYTLS_PADDING_SCHEME_PLACEHOLDER}
          value={inputValue(paddingScheme)}
          onChange={(event) => {
            if (values.type === 'v2node' && values.config.protocol === 'anytls') {
              setField('config.padding_scheme', event.target.value, setFieldOptions);
            } else if (values.type === 'anytls') {
              setField('padding_scheme', event.target.value, setFieldOptions);
            }
          }}
          data-testid="node-padding-scheme"
        />
        <NodeFieldError
          form={form}
          name={values.type === 'v2node' ? 'config.padding_scheme' : 'padding_scheme'}
        />
      </div>
    );
  }

  if (field === 'tls_settings' || field === 'tlsSettings') {
    if (values.type === 'v2node') {
      const settings = 'tls_settings' in values.config ? values.config.tls_settings : undefined;
      return (
        <TlsSettingsField
          settings={settings}
          tls={values.config.tls}
          certApply
          onChange={(next) => setField('config.tls_settings', next, setFieldOptions)}
        />
      );
    }
    if (values.type === 'vmess' && field === 'tlsSettings') {
      return (
        <TlsSettingsField
          settings={values.tlsSettings}
          tls={values.tls}
          certApply={false}
          onChange={(next) => setField('tlsSettings', next, setFieldOptions)}
        />
      );
    }
    if (values.type === 'vless' && field === 'tls_settings') {
      return (
        <TlsSettingsField
          settings={values.tls_settings}
          tls={values.tls}
          certApply
          onChange={(next) => setField('tls_settings', next, setFieldOptions)}
        />
      );
    }
    return null;
  }

  if (field === 'encryption_settings') {
    if (values.type === 'vless') {
      return (
        <EncryptionSettingsField
          settings={values.encryption_settings}
          onChange={(next) => setField('encryption_settings', next, setFieldOptions)}
        />
      );
    }
    if (values.type === 'v2node' && values.config.protocol === 'vless') {
      return (
        <EncryptionSettingsField
          settings={values.config.encryption_settings}
          onChange={(next) => setField('config.encryption_settings', next, setFieldOptions)}
        />
      );
    }
    return null;
  }

  return null;
}

function TlsSettingsField({
  settings,
  tls,
  certApply,
  onChange,
}: {
  settings: unknown;
  tls: unknown;
  certApply: boolean;
  onChange: (value: object) => void;
}) {
  const { t } = useTranslation();
  const value = normalizeSettings(settings, TLS_SETTINGS_DEFAULTS);
  const tlsValue = parseInt(String(tls ?? 0), 10);
  const certMode = settingValue(value, 'cert_mode');
  const ech = settingValue(value, 'ech');
  const change = (key: string, next: unknown) => {
    onChange(withSetting(value, key, next));
  };

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="node-tls-server-name">Server Name(SNI)</Label>
        <Input
          id="node-tls-server-name"
          value={displayText(settingValue(value, 'server_name'))}
          onChange={(event) => change('server_name', event.target.value)}
          placeholder={
            tlsValue === 2 ? t(($) => $.admin.servers.reality_server_name_placeholder) : ''
          }
        />
      </div>
      {tlsValue === 1 && certApply ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-cert-mode">{t(($) => $.admin.servers.cert_mode)}</Label>
          <NodeSelect
            id="node-tls-cert-mode"
            value={selectValue(certMode) ?? 'self'}
            options={getTlsCertModeOptions(t)}
            onChange={(next) => change('cert_mode', next)}
          />
        </div>
      ) : null}
      {certMode === 'dns' && certApply ? (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Label htmlFor="node-tls-provider">{t(($) => $.admin.servers.dns_provider)}</Label>
            <a
              className="text-sm text-primary"
              target="_blank"
              href="https://go-acme.github.io/lego/dns/index.html"
              rel="noopener noreferrer"
            >
              {t(($) => $.admin.servers.fill_reference)}
            </a>
          </div>
          <Input
            id="node-tls-provider"
            value={displayText(settingValue(value, 'provider'))}
            onChange={(event) => change('provider', event.target.value)}
            placeholder={t(($) => $.admin.servers.dns_provider_placeholder)}
          />
        </div>
      ) : null}
      {certMode === 'dns' && certApply ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-dns-env">DNS env</Label>
          <Input
            id="node-tls-dns-env"
            value={displayText(settingValue(value, 'dns_env'))}
            onChange={(event) => change('dns_env', event.target.value)}
            placeholder={t(($) => $.admin.servers.dns_env_placeholder)}
          />
        </div>
      ) : null}
      {tlsValue === 1 && certMode !== 'none' && certApply ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-cert-file">{t(($) => $.admin.servers.cert_file)}</Label>
          <Input
            id="node-tls-cert-file"
            value={displayText(settingValue(value, 'cert_file'))}
            onChange={(event) => change('cert_file', event.target.value)}
            placeholder={t(($) => $.admin.servers.cert_file_placeholder)}
          />
        </div>
      ) : null}
      {tlsValue === 1 && certMode !== 'none' && certApply ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-key-file">{t(($) => $.admin.servers.key_file)}</Label>
          <Input
            id="node-tls-key-file"
            value={displayText(settingValue(value, 'key_file'))}
            onChange={(event) => change('key_file', event.target.value)}
            placeholder={t(($) => $.admin.servers.cert_file_placeholder)}
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-destination">Server Address</Label>
          <Input
            id="node-tls-destination"
            value={displayText(settingValue(value, 'dest'))}
            onChange={(event) => change('dest', event.target.value)}
            placeholder={t(($) => $.admin.servers.reality_dest_placeholder)}
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-server-port">Server Port</Label>
          <Input
            id="node-tls-server-port"
            value={displayText(settingValue(value, 'server_port'))}
            onChange={(event) => change('server_port', event.target.value)}
            placeholder={t(($) => $.admin.servers.reality_port_placeholder)}
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-proxy-protocol">Proxy Protocol</Label>
          <NodeSelect
            id="node-tls-proxy-protocol"
            value={parseInt(String(settingValue(value, 'xver') ?? 0), 10) || 0}
            options={PROXY_PROTOCOL_OPTIONS}
            onChange={(next) => change('xver', next)}
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-private-key">Private Key</Label>
          <Input
            id="node-tls-private-key"
            value={displayText(settingValue(value, 'private_key'))}
            onChange={(event) => change('private_key', event.target.value)}
            placeholder={t(($) => $.admin.servers.auto_generate_placeholder)}
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-public-key">Public Key</Label>
          <Input
            id="node-tls-public-key"
            value={displayText(settingValue(value, 'public_key'))}
            onChange={(event) => change('public_key', event.target.value)}
            placeholder={t(($) => $.admin.servers.auto_generate_placeholder)}
          />
        </div>
      ) : null}
      {tlsValue === 2 ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-short-id">ShortId</Label>
          <Input
            id="node-tls-short-id"
            value={displayText(settingValue(value, 'short_id'))}
            onChange={(event) => change('short_id', event.target.value)}
            placeholder={t(($) => $.admin.servers.auto_generate_placeholder)}
          />
        </div>
      ) : null}
      <div className="space-y-2">
        <Label htmlFor="node-tls-fingerprint">FingerPrint</Label>
        <NodeSelect
          id="node-tls-fingerprint"
          value={selectValue(settingValue(value, 'fingerprint'))}
          options={TLS_FINGERPRINT_OPTIONS}
          onChange={(next) => change('fingerprint', next)}
          placeholder={t(($) => $.admin.servers.fingerprint_placeholder)}
        />
      </div>
      {tlsValue === 1 && certApply ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-reject-unknown-sni">Reject unknown sni</Label>
          <div>
            <Switch
              id="node-tls-reject-unknown-sni"
              checked={toBoolean(settingValue(value, 'reject_unknown_sni'))}
              onCheckedChange={(checked) => change('reject_unknown_sni', checked ? '1' : '0')}
            />
          </div>
        </div>
      ) : null}
      <div className="space-y-2">
        <Label htmlFor="node-tls-allow-insecure">Allow Insecure</Label>
        <div>
          <Switch
            id="node-tls-allow-insecure"
            checked={toBoolean(settingValue(value, 'allow_insecure'))}
            onCheckedChange={(checked) => change('allow_insecure', checked ? '1' : '0')}
          />
        </div>
      </div>
      <div className="space-y-2">
        <Label htmlFor="node-tls-ech">ECH (Encrypted Client Hello)</Label>
        <NodeSelect
          id="node-tls-ech"
          value={displayText(ech)}
          options={getEchModeOptions(t)}
          onChange={(next) => change('ech', next)}
          placeholder={t(($) => $.admin.servers.ech_mode_placeholder)}
        />
      </div>
      {ech === 'cloudflare' ? (
        <div className="rounded-md border border-success/30 bg-success/10 px-3 py-2 text-sm text-success">
          {t(($) => $.admin.servers.ech_cloudflare_note)}
        </div>
      ) : null}
      {ech === 'custom' ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-ech-server-name">
            {t(($) => $.admin.servers.ech_server_name)}
          </Label>
          <Input
            id="node-tls-ech-server-name"
            value={displayText(settingValue(value, 'ech_server_name'))}
            onChange={(event) => change('ech_server_name', event.target.value)}
            placeholder={t(($) => $.admin.servers.required_placeholder)}
          />
        </div>
      ) : null}
      {ech === 'custom' ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-ech-key">{t(($) => $.admin.servers.ech_key)}</Label>
          <Input
            id="node-tls-ech-key"
            value={displayText(settingValue(value, 'ech_key'))}
            onChange={(event) => change('ech_key', event.target.value)}
            placeholder={t(($) => $.admin.servers.auto_generate_placeholder)}
          />
        </div>
      ) : null}
      {ech === 'custom' ? (
        <div className="space-y-2">
          <Label htmlFor="node-tls-ech-config">{t(($) => $.admin.servers.ech_config)}</Label>
          <Input
            id="node-tls-ech-config"
            value={displayText(settingValue(value, 'ech_config'))}
            onChange={(event) => change('ech_config', event.target.value)}
            placeholder={t(($) => $.admin.servers.auto_generate_placeholder)}
          />
        </div>
      ) : null}
    </div>
  );
}

function EncryptionSettingsField({
  settings,
  onChange,
}: {
  settings: unknown;
  onChange: (value: object) => void;
}) {
  const { t } = useTranslation();
  const value = useMemo(
    () => normalizeSettings(settings, ENCRYPTION_SETTINGS_DEFAULTS),
    [settings],
  );
  const change = (key: string, next: unknown) => {
    onChange(withSetting(value, key, next));
  };
  const rtt = settingValue(value, 'rtt');

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="node-encryption-mode">Mode</Label>
        <NodeSelect
          id="node-encryption-mode"
          value={displayText(settingValue(value, 'mode')) || 'native'}
          options={ENCRYPTION_MODE_OPTIONS}
          onChange={(next) => change('mode', next)}
        />
      </div>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className="space-y-2">
          <Label htmlFor="node-encryption-rtt">RTT</Label>
          <NodeSelect
            id="node-encryption-rtt"
            value={displayText(rtt) || '0rtt'}
            options={ENCRYPTION_RTT_OPTIONS}
            onChange={(next) => change('rtt', next)}
          />
        </div>
        {rtt === '0rtt' ? (
          <div className="space-y-2">
            <Label htmlFor="node-encryption-ticket">Ticket time</Label>
            <Input
              id="node-encryption-ticket"
              value={displayText(settingValue(value, 'ticket'))}
              onChange={(event) => change('ticket', event.target.value)}
              placeholder={t(($) => $.admin.servers.ticket_placeholder)}
            />
          </div>
        ) : null}
      </div>
      <div className="space-y-2">
        <Label htmlFor="node-encryption-server-padding">Server Padding</Label>
        <Input
          id="node-encryption-server-padding"
          value={displayText(settingValue(value, 'server_padding'))}
          onChange={(event) => change('server_padding', event.target.value)}
          placeholder={t(($) => $.admin.servers.padding_placeholder)}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="node-encryption-private-key">Private Key</Label>
        <Input
          id="node-encryption-private-key"
          value={displayText(settingValue(value, 'private_key'))}
          onChange={(event) => change('private_key', event.target.value)}
          placeholder={t(($) => $.admin.servers.pq_auto_generate_placeholder)}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="node-encryption-client-padding">Client Padding</Label>
        <Input
          id="node-encryption-client-padding"
          value={displayText(settingValue(value, 'client_padding'))}
          onChange={(event) => change('client_padding', event.target.value)}
          placeholder={t(($) => $.admin.servers.padding_placeholder)}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="node-encryption-password">Password</Label>
        <Input
          id="node-encryption-password"
          value={displayText(settingValue(value, 'password'))}
          onChange={(event) => change('password', event.target.value)}
          placeholder={t(($) => $.admin.servers.pq_auto_generate_placeholder)}
        />
      </div>
    </div>
  );
}
