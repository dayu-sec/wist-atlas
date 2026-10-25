import type {
  HostProcessTopologyGraph,
  HostProcessTopologyNode,
  HostTopologyDto,
  LoadedProcessGroupsState,
} from '../types/domain';
import { getNodeStyle } from '../utils/colors';
import { formatAddressFamily, parseTopPrograms } from '../utils/presentation';
import { basename, formatCount, formatRelative, truncate } from '../utils/format';
import { ArrowRightIcon, CloseIcon, KindGlyph } from './icons';

type Props = {
  node: HostProcessTopologyNode | null;
  graph: HostProcessTopologyGraph | null;
  hostTopology: HostTopologyDto | null;
  processViewMode: 'overview' | 'expanded' | 'loading' | null;
  loadedProcessGroups: LoadedProcessGroupsState | null;
  onClose: () => void;
  onExpand: (nodeId: string) => void;
  /** 网段节点：切到「这个网段里有哪些机器」。 */
  onOpenNetwork?: (segmentObjectId: string) => void;
  /** 服务节点：切到「这个服务跑在哪些主机上」。 */
  onOpenService?: (serviceObjectId: string) => void;
  /** 主机节点：在网段视图里反着切回那台主机的聚焦图。 */
  onFocusHost?: (hostObjectId: string) => void;
};

export default function NodeDetailPanel({
  node,
  graph,
  hostTopology,
  processViewMode,
  loadedProcessGroups,
  onClose,
  onExpand,
  onOpenNetwork,
  onOpenService,
  onFocusHost,
}: Props) {
  if (!node) return null;

  const style = getNodeStyle(node.objectKind);
  const title =
    node.objectKind === 'ProcessGroup'
      ? basename(String(node.properties.executable ?? node.label))
      : node.objectKind === 'ProcessSummary'
        ? '运行时'
        : node.label;

  return (
    <aside className="absolute inset-y-0 right-0 z-30 flex w-[336px] flex-col border-l border-line bg-surface shadow-[-12px_0_32px_-24px_rgba(16,24,40,0.35)]">
      <header className="flex items-start gap-2.5 border-b border-line px-4 py-3.5">
        <span
          className="mt-0.5 grid size-7 shrink-0 place-items-center rounded-[8px]"
          style={{ backgroundColor: style.fill, color: style.stroke }}
          aria-hidden
        >
          <KindGlyph kind={node.objectKind} className="size-4" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-[9.5px] font-medium uppercase leading-none tracking-[0.08em] text-[#98a2b3]">
            {style.label}
          </div>
          <h3 className="mt-1 truncate text-[13px] font-semibold leading-4 text-ink" title={title}>
            {title}
          </h3>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="关闭"
          className="-mr-1 grid size-7 shrink-0 place-items-center rounded-[7px] text-[#98a2b3] transition-colors hover:bg-[#f2f4f7] hover:text-[#344054]"
        >
          <CloseIcon className="size-4" />
        </button>
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-4">
        {node.objectKind === 'HostInventory' && hostTopology?.host.id === node.objectId ? (
          <HostDetail topology={hostTopology} />
        ) : node.objectKind === 'NetworkSegment' ? (
          <NetworkDetail node={node} graph={graph} />
        ) : node.objectKind === 'ServiceEntity' ? (
          <ServiceDetail node={node} graph={graph} />
        ) : node.objectKind === 'ProcessSummary' ? (
          <RuntimeDetail
            node={node}
            expanded={Boolean(graph?.nodes.some((item) => item.objectKind === 'ProcessGroup'))}
            onExpand={() => onExpand(node.id)}
          />
        ) : node.objectKind === 'ProcessGroup' ? (
          <ProcessGroupDetail
            node={node}
            processViewMode={processViewMode}
            loadedProcessGroups={loadedProcessGroups}
          />
        ) : (
          <PropertiesDetail node={node} />
        )}

        {node.objectKind === 'NetworkSegment' && onOpenNetwork ? (
          <DrillButton onClick={() => onOpenNetwork(node.objectId)}>
            查看该网段里的主机
          </DrillButton>
        ) : null}
        {node.objectKind === 'ServiceEntity' && onOpenService ? (
          <DrillButton onClick={() => onOpenService(node.objectId)}>
            查看监听详情
          </DrillButton>
        ) : null}
        {node.objectKind === 'HostInventory' && onFocusHost ? (
          <DrillButton onClick={() => onFocusHost(node.objectId)}>
            聚焦这台主机
          </DrillButton>
        ) : null}
      </div>
    </aside>
  );
}

function DrillButton({ children, onClick }: { children: React.ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="mt-4 flex w-full items-center justify-between rounded-lg border border-line bg-[#fcfcfd] px-3 py-2 text-[11.5px] font-medium text-[#344054] transition-colors hover:border-brand/40 hover:bg-white hover:text-brand"
    >
      {children}
      <ArrowRightIcon className="size-3.5" />
    </button>
  );
}

function HostDetail({ topology }: { topology: HostTopologyDto }) {
  const os = [topology.host.osName, topology.host.osVersion].filter(Boolean).join(' ');
  const containers = topology.containers ?? [];

  return (
    <div className="space-y-5">
      <Section title="资产">
        <Field label="系统">{os || '未采集'}</Field>
        <Field label="架构">{topology.host.arch ?? '未采集'}</Field>
        <Field label="机器 ID">
          <span className="font-mono text-[10px] break-all">{topology.host.machineId ?? '—'}</span>
        </Field>
      </Section>

      <Section title="规模">
        <div className="grid grid-cols-3 gap-2">
          <Metric label="网段" value={topology.networkSegments.length} />
          <Metric label="进程" value={topology.processes.length} />
          <Metric label="容器" value={containers.length} />
        </div>
      </Section>

      {containers.length > 0 && (
        <Section title={`容器（${containers.length}）`}>
          <ul className="space-y-1.5">
            {containers.map((container) => (
              <li
                key={container.container_id}
                className="rounded-[8px] border border-[#f2f4f7] bg-[#f9fafb] px-2.5 py-1.5"
              >
                <div className="flex items-baseline justify-between gap-2">
                  <span
                    className="truncate text-[11px] font-medium text-[#344054]"
                    title={container.container_name ?? container.container_id}
                  >
                    {container.container_name ?? container.container_id.slice(0, 12)}
                  </span>
                  <span className="shrink-0 font-mono text-[9.5px] text-[#98a2b3]">
                    {container.runtime_type}
                  </span>
                </div>
                {container.image_ref && (
                  <div
                    className="mt-0.5 truncate font-mono text-[9.5px] leading-3 text-[#667085]"
                    title={container.image_ref}
                  >
                    {container.image_ref}
                  </div>
                )}
                {container.pod_name && (
                  <div className="mt-0.5 truncate text-[9.5px] leading-3 text-[#98a2b3]">
                    {container.k8s_namespace ? `${container.k8s_namespace} / ` : ''}
                    {container.pod_name}
                  </div>
                )}
              </li>
            ))}
          </ul>
        </Section>
      )}

      {topology.networkSegments.length > 0 && (
        <Section title={`网络接口 ${topology.networkSegments.length}`}>
          <ul className="space-y-1">
            {topology.networkSegments.map((segment, index) => {
              const name = String(segment.name ?? segment.cidr ?? '-');
              const family = formatAddressFamily(segment.address_family);
              return (
                <li
                  key={`${String(segment.network_segment_id ?? index)}`}
                  className="flex items-center justify-between gap-2 rounded-[8px] bg-[#f9fafb] px-2.5 py-1.5"
                >
                  <span className="truncate font-mono text-[10.5px] text-[#344054]" title={name}>
                    {truncate(name, 30)}
                  </span>
                  {family && (
                    <span className="shrink-0 text-[9px] font-medium uppercase tracking-[0.06em] text-[#98a2b3]">
                      {family}
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        </Section>
      )}

      {topology.processGroups.length > 0 && (
        <Section title="进程分组 Top 5">
          <ul className="space-y-1.5">
            {topology.processGroups.slice(0, 5).map((group, index) => (
              <li key={`${String(group.display_name)}-${index}`} className="flex items-baseline justify-between gap-2">
                <span className="truncate text-[11px] text-[#344054]" title={String(group.display_name)}>
                  {String(group.display_name ?? '-')}
                </span>
                <span className="tnum shrink-0 text-[10.5px] text-[#98a2b3]">
                  {String(group.process_count ?? '-')}
                </span>
              </li>
            ))}
          </ul>
        </Section>
      )}

      <Section title="标识">
        <Field label="对象 ID">
          <span className="font-mono text-[10px] break-all">{topology.host.id}</span>
        </Field>
      </Section>
    </div>
  );
}

/**
 * 网段自己只有 CIDR 和地址族；「哪个接口、哪个 IP」在 host_network_assoc 边上。
 * 详情面板要把这条边上的信息读出来，否则用户点开网段还是不知道它是怎么接进来的。
 */
function NetworkDetail({ node, graph }: { node: HostProcessTopologyNode; graph: HostProcessTopologyGraph | null }) {
  const association = graph?.edges.find(
    (edge) =>
      edge.edgeKind === 'host_network_assoc' &&
      (edge.target === node.id || edge.source === node.id),
  );
  const iface = association?.properties?.ifaceName;
  const ip = association?.properties?.ipAddr;
  const family = formatAddressFamily(node.properties.addressFamily);
  const name = String(node.properties.cidr ?? node.label);

  return (
    <div className="space-y-5">
      <Section title="接入">
        <Field label="接口">{typeof iface === 'string' ? iface : '—'}</Field>
        <Field label="地址">{typeof ip === 'string' ? ip : '—'}</Field>
        <Field label="地址族">{family || '—'}</Field>
      </Section>

      <Section title="网段">
        <Field label="CIDR">
          <span className="font-mono text-[10.5px]">{name}</span>
        </Field>
        <Field label="展示名">{node.label}</Field>
      </Section>

      <Section title="标识">
        <Field label="对象 ID">
          <span className="font-mono text-[10px] break-all">{node.objectId}</span>
        </Field>
      </Section>
    </div>
  );
}

function ServiceDetail({
  node,
  graph,
}: {
  node: HostProcessTopologyNode;
  graph: HostProcessTopologyGraph | null;
}) {
  const endpoints = endpointStrings(node.properties);
  const instanceCount = asNumber(node.properties.instanceCount);

  // 实例和主机都从图里反查，不从节点属性重建：属性只保证「这个节点自己知道什么」，
  // 而「还跑在哪些主机上」本来就是图上的事实。
  const instanceIds = new Set(
    (graph?.edges ?? [])
      .filter((edge) => edge.edgeKind === 'service_instance_assoc' && edge.target === node.id)
      .map((edge) => edge.source),
  );
  const instances = (graph?.nodes ?? []).filter((item) => instanceIds.has(item.id));
  const hostCount = (graph?.nodes ?? []).filter((item) => item.objectKind === 'HostInventory').length;

  return (
    <div className="space-y-5">
      <Section title="服务">
        <Field label="类型">{asString(node.properties.serviceType) ?? '—'}</Field>
        <Field label="边界">{asString(node.properties.boundary) ?? '—'}</Field>
        <Field label="外部引用">
          <span className="font-mono text-[10px] break-all">
            {asString(node.properties.externalRef) ?? '—'}
          </span>
        </Field>
      </Section>

      <Section title="规模">
        <div className="grid grid-cols-2 gap-2">
          <Metric label="实例" value={instanceCount ?? instances.length} />
          <Metric label="主机" value={hostCount} />
        </div>
      </Section>

      <Section title="监听端点">
        {endpoints.length === 0 ? (
          <p className="text-[11px] text-[#98a2b3]">没有采集到监听地址</p>
        ) : (
          <ul className="space-y-1">
            {endpoints.map((endpoint) => (
              <li key={endpoint} className="font-mono text-[10.5px] text-[#475467]">
                {endpoint}
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title={`运行主机（${instances.length}）`}>
        {instances.length === 0 ? (
          <p className="text-[11px] text-[#98a2b3]">没有实例，可能已停止监听</p>
        ) : (
          <ul className="space-y-1.5">
            {instances.map((instance) => {
              const hostName = asString(instance.properties.hostName);
              const instanceEndpoints = endpointStrings(instance.properties);
              const lastSeenAt = asString(instance.properties.lastSeenAt);
              return (
                <li
                  key={instance.id}
                  className="rounded-[8px] border border-[#f2f4f7] bg-[#f9fafb] px-2.5 py-1.5"
                >
                  <div className="flex items-baseline justify-between gap-2">
                    <span
                      className="truncate text-[11px] font-medium text-[#344054]"
                      title={hostName ?? undefined}
                    >
                      {hostName ?? '未知主机'}
                    </span>
                    <span className="shrink-0 text-[9.5px] text-[#98a2b3]">
                      {formatRelative(lastSeenAt)}
                    </span>
                  </div>
                  {instanceEndpoints.length > 0 && (
                    <div className="mt-0.5 font-mono text-[9.5px] leading-3 text-[#98a2b3]">
                      {instanceEndpoints.join(' · ')}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </Section>

      <Section title="标识">
        <Field label="对象 ID">
          <span className="font-mono text-[10px] break-all">{node.objectId}</span>
        </Field>
      </Section>

      <GraphId id={node.id} />
    </div>
  );
}

function RuntimeDetail({
  node,
  expanded,
  onExpand,
}: {
  node: HostProcessTopologyNode;
  expanded: boolean;
  onExpand: () => void;
}) {
  const totalProcesses = Number(node.properties.totalProcesses ?? 0);
  const totalPrograms = Number(node.properties.totalPrograms ?? 0);
  const programs = parseTopPrograms(node.properties.topPrograms);

  return (
    <div className="space-y-5">
      <Section title="规模">
        <div className="grid grid-cols-2 gap-2">
          <Metric label="进程" value={totalProcesses} />
          <Metric label="程序" value={totalPrograms} />
        </div>
      </Section>

      <Section title="高频程序">
        {programs.length > 0 ? (
          <ul className="space-y-1.5">
            {programs.map((program) => (
              <li key={program.name} className="flex items-baseline justify-between gap-2">
                <span className="truncate text-[11px] text-[#344054]" title={program.name}>
                  {program.name}
                </span>
                <span className="tnum shrink-0 text-[10.5px] text-[#98a2b3]">{program.count}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-[11px] text-[#98a2b3]">暂无程序摘要</p>
        )}
      </Section>

      <button
        type="button"
        onClick={onExpand}
        className="flex h-8 w-full items-center justify-center gap-1.5 rounded-[8px] bg-[#4f46e5] text-[11.5px] font-medium text-white transition-colors hover:bg-[#4338ca]"
      >
        {expanded ? '收起进程分组' : '在图中展开进程分组'}
        <ArrowRightIcon className="size-3.5" />
      </button>

      <Section title="标识">
        <Field label="对象 ID">
          <span className="font-mono text-[10px] break-all">{node.objectId}</span>
        </Field>
      </Section>
    </div>
  );
}

function ProcessGroupDetail({
  node,
  processViewMode,
  loadedProcessGroups,
}: {
  node: HostProcessTopologyNode;
  processViewMode: 'overview' | 'expanded' | 'loading' | null;
  loadedProcessGroups: LoadedProcessGroupsState | null;
}) {
  const processCount = asNumber(node.properties.processCount);
  const totalMemoryRssKiB = asNumber(node.properties.totalMemoryRssKiB);
  const totalMemoryRssMiB = asNumber(node.properties.totalMemoryRssMiB);
  const dominantState = asString(node.properties.dominantState);
  const executable = asString(node.properties.executable);
  const states = asString(node.properties.states);
  const hiddenGroups = asNumber(node.properties.hiddenGroups);
  const hiddenProcesses = asNumber(node.properties.hiddenProcesses);
  const displayLimit = asNumber(node.properties.displayLimit);
  const isOverflow = Boolean(node.properties.collapsed) && hiddenGroups != null;

  if (isOverflow) {
    return (
      <div className="space-y-5">
        <Section title="收起的分组">
          <div className="grid grid-cols-2 gap-2">
            <Metric label="隐藏分组" value={hiddenGroups ?? 0} />
            <Metric label="隐藏进程" value={hiddenProcesses ?? 0} />
          </div>
          <p className="mt-2 text-[11px] leading-relaxed text-[#98a2b3]">
            当前只显示进程数最多的前 {displayLimit ?? '-'} 组。点这个节点可以继续加载更多分组。
          </p>
        </Section>
        <ViewModeBadge mode={processViewMode} />
        {processViewMode === 'loading' && <Notice tone="warn">正在加载下一批分组…</Notice>}
        {loadedProcessGroups && (
          <Notice tone="info">
            已追加 {loadedProcessGroups.groups.length} 组
            {loadedProcessGroups.totalGroups > 0 ? ` / ${loadedProcessGroups.totalGroups}` : ''}
            {loadedProcessGroups.hasMore
              ? `，还有 ${loadedProcessGroups.totalGroups - loadedProcessGroups.offset} 组未加载`
              : '，已全部加载'}
          </Notice>
        )}
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <Section title="进程分组">
        <Field label="可执行文件">
          <span className="font-mono text-[10px] break-all">{executable ?? '—'}</span>
        </Field>
        <div className="grid grid-cols-2 gap-2 pt-1">
          <Metric label="进程数" value={processCount ?? 0} />
          <Metric label="RSS" value={totalMemoryRssMiB ?? 0} suffix="MiB" />
        </div>
      </Section>

      <Section title="状态">
        <Field label="主导状态">{dominantState ?? '—'}</Field>
        <Field label="分布">{states ?? '—'}</Field>
        <Field label="内存">{totalMemoryRssKiB != null ? `${formatCount(totalMemoryRssKiB)} KiB` : '—'}</Field>
      </Section>

      <ViewModeBadge mode={processViewMode} />
      <GraphId id={node.id} />
    </div>
  );
}

function PropertiesDetail({ node }: { node: HostProcessTopologyNode }) {
  const entries = Object.entries(node.properties);
  return (
    <div className="space-y-5">
      <Section title="属性">
        {entries.length === 0 ? (
          <p className="text-[11px] text-[#98a2b3]">没有属性</p>
        ) : (
          <dl className="space-y-1.5">
            {entries.map(([key, value]) => (
              <div key={key} className="flex items-baseline justify-between gap-3">
                <dt className="shrink-0 text-[10px] text-[#98a2b3]">{key}</dt>
                <dd className="min-w-0 text-right font-mono text-[10px] break-all text-[#475467]">
                  {formatValue(value)}
                </dd>
              </div>
            ))}
          </dl>
        )}
      </Section>

      <Section title="标识">
        <Field label="对象 ID">
          <span className="font-mono text-[10px] break-all">{node.objectId}</span>
        </Field>
      </Section>

      <GraphId id={node.id} />
    </div>
  );
}

function GraphId({ id }: { id: string }) {
  return (
    <details className="group">
      <summary className="cursor-pointer list-none text-[10px] text-[#98a2b3] transition-colors hover:text-[#475467]">
        图元素 ID
      </summary>
      <p className="mt-1 font-mono text-[10px] break-all text-[#98a2b3]">{id}</p>
    </details>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section>
      <h4 className="mb-2 text-[9.5px] font-medium uppercase tracking-[0.08em] text-[#98a2b3]">{title}</h4>
      <div className="space-y-1.5">{children}</div>
    </section>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-3 text-[11px]">
      <span className="shrink-0 text-[#98a2b3]">{label}</span>
      <span className="min-w-0 text-right text-[#344054]">{children}</span>
    </div>
  );
}

function Metric({ label, value, suffix }: { label: string; value: number; suffix?: string }) {
  return (
    <div className="rounded-[8px] border border-[#f2f4f7] bg-[#f9fafb] px-2.5 py-2">
      <div className="text-[9.5px] uppercase tracking-[0.06em] text-[#98a2b3]">{label}</div>
      <div className="tnum mt-0.5 text-[15px] font-semibold leading-tight text-ink">
        {formatCount(value)}
        {suffix && <span className="ml-0.5 text-[10px] font-normal text-[#98a2b3]">{suffix}</span>}
      </div>
    </div>
  );
}

function Notice({ tone, children }: { tone: 'info' | 'warn'; children: React.ReactNode }) {
  const className =
    tone === 'warn'
      ? 'border-[#fedf89] bg-[#fffaeb] text-[#b54708]'
      : 'border-brand/25 bg-brand-soft text-brand';
  return <div className={`rounded-[8px] border px-3 py-2 text-[11px] leading-4 ${className}`}>{children}</div>;
}

function ViewModeBadge({ mode }: { mode: 'overview' | 'expanded' | 'loading' | null }) {
  if (!mode) return null;

  const text =
    mode === 'loading' ? '正在加载更多分组' : mode === 'expanded' ? '已展开分组' : '概览视图';
  const className =
    mode === 'loading'
      ? 'border-[#fedf89] bg-[#fffaeb] text-[#b54708]'
      : mode === 'expanded'
        ? 'border-brand/25 bg-brand-soft text-brand'
        : 'border-line bg-[#f9fafb] text-[#475467]';

  return <div className={`rounded-[8px] border px-3 py-2 text-[11px] ${className}`}>{text}</div>;
}

function formatValue(value: unknown) {
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (value == null) return '—';
  return String(value);
}

function asString(value: unknown): string | null {
  if (typeof value !== 'string' || value.length === 0) return null;
  return value;
}

/** `endpoints` 是后端摊平后的 `{protocol, address, port}` 全集。 */
function endpointStrings(properties: Record<string, unknown>): string[] {
  const raw = properties.endpoints;
  if (!Array.isArray(raw)) return [];
  return raw
    .map((item) =>
      typeof item === 'object' && item !== null ? (item as Record<string, unknown>) : null,
    )
    .filter((item): item is Record<string, unknown> => Boolean(item))
    .map((item) =>
      `${String(item.protocol ?? 'tcp')}/${String(item.port ?? '')} ${String(item.address ?? '')}`.trim(),
    )
    .filter((label) => label.length > 0);
}

function asNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}
