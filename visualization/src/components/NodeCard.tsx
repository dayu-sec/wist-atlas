import type { HostProcessTopologyNode } from '../types/domain';
import {
  formatAddressFamily,
  kindLabel,
  numericProperty,
  parseTopPrograms,
  shortCidr,
} from '../utils/presentation';
import { ArrowRightIcon, KindGlyph } from './icons';
import { basename, formatCount, formatRelative } from '../utils/format';

export type GraphNodeHint = {
  ifaceName?: string;
  ipAddr?: string;
};

export type HostStats = {
  segments: number;
  processes: number;
  groups: number;
  arch?: string | null;
  scannedAt?: string | null;
};

type Props = {
  node: HostProcessTopologyNode;
  hint?: GraphNodeHint;
  hostStats?: HostStats;
  selected: boolean;
  dimmed: boolean;
  expanded: boolean;
  onSelect: (nodeId: string) => void;
  onExpand?: (nodeId: string) => void;
  onHover: (nodeId: string | null) => void;
};

/**
 * 图上的节点不再是一个「几何形状 + 漂浮标签」，而是一张真正的信息卡：
 * 图标、主标题、副标题、角标各就各位。边只画在卡片之间，标签不再需要白底纸片。
 */
export default function NodeCard({
  node,
  hint,
  hostStats,
  selected,
  dimmed,
  expanded,
  onSelect,
  onExpand,
  onHover,
}: Props) {
  const className = [
    'node-card flex h-full w-full flex-col overflow-hidden rounded-xl border bg-white text-left',
    selected ? 'node-card-selected' : '',
    dimmed ? 'node-card-dimmed' : '',
  ].join(' ');

  return (
    <div
      className={className}
      role="button"
      tabIndex={0}
      aria-label={`${kindLabel(node.objectKind)} ${node.label}`}
      data-node-id={node.id}
      data-selected={selected || undefined}
      data-dimmed={dimmed || undefined}
      onClick={(event) => {
        event.stopPropagation();
        onSelect(node.id);
      }}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return;
        if (event.key !== 'Enter' && event.key !== ' ') return;
        event.preventDefault();
        onSelect(node.id);
      }}
      onPointerEnter={() => onHover(node.id)}
      onPointerLeave={() => onHover(null)}
    >
      <CardBody
        node={node}
        hint={hint}
        hostStats={hostStats}
        expanded={expanded}
        onExpand={onExpand ? () => onExpand(node.id) : undefined}
      />
    </div>
  );
}

function CardBody({
  node,
  hint,
  hostStats,
  expanded,
  onExpand,
}: Pick<Props, 'node' | 'hint' | 'hostStats' | 'expanded'> & { onExpand?: () => void }) {
  switch (node.objectKind) {
    case 'HostInventory':
      return <HostCard node={node} stats={hostStats} />;
    case 'NetworkSegment':
      return <NetworkCard node={node} hint={hint} />;
    case 'ProcessSummary':
      return <RuntimeCard node={node} expanded={expanded} onExpand={onExpand} />;
    case 'ProcessGroup':
      return <ProcessGroupCard node={node} />;
    case 'ServiceEntity':
      return <ServiceCard node={node} />;
    case 'Subject':
      return <SubjectCard node={node} />;
    default:
      return <RuntimeProcessCard node={node} />;
  }
}

function MicroLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-[9.5px] font-medium uppercase leading-none tracking-[0.08em] text-[#98a2b3]">
      {children}
    </span>
  );
}

/** 主机是这张图的锚点：身份、系统、规模三行，别的一概不塞。 */
function HostCard({ node, stats }: { node: HostProcessTopologyNode; stats?: HostStats }) {
  const os = [node.properties.osName, node.properties.osVersion].filter(Boolean).join(' ');
  const arch =
    stats?.arch ?? (typeof node.properties.arch === 'string' ? node.properties.arch : '');
  // 网段视图里没有「当前主机」的聚合可借，规模数据挂在节点上；两者取有值的那个。
  const segments = stats?.segments ?? optionalNumber(node.properties, 'segmentCount');
  const processes = stats?.processes ?? optionalNumber(node.properties, 'processCount');

  return (
    <div className="flex h-full flex-col px-4 pb-3.5 pt-4">
      <div className="flex items-start gap-3">
        <span
          className="grid size-11 shrink-0 place-items-center rounded-[12px]"
          style={{ backgroundColor: '#FFF6ED', color: '#F79009' }}
          aria-hidden
        >
          <KindGlyph kind="HostInventory" className="size-[22px]" />
        </span>
        <div className="min-w-0 flex-1 pt-0.5">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-[14px] font-semibold leading-5 tracking-[-0.01em] text-[#101828]" title={node.label}>
              {node.label}
            </h3>
            <span className="shrink-0 rounded-[5px] bg-[#f2f4f7] px-1.5 py-0.5 text-[9.5px] font-medium leading-none text-[#475467]">
              本机
            </span>
          </div>
          <p className="mt-1 truncate text-[11px] leading-4 text-[#667085]">
            {os || '系统信息未采集'}
            {arch ? <span className="font-mono text-[10.5px] text-[#98a2b3]"> · {arch}</span> : null}
          </p>
          {stats?.scannedAt ? (
            <p className="mt-1.5 flex items-center gap-1.5 text-[9.5px] leading-3 text-[#667085]">
              <span className="size-1.5 rounded-full bg-[#12b76a]" aria-hidden />
              盘点于 {formatRelative(stats.scannedAt)}
            </p>
          ) : null}
        </div>
      </div>

      <div className="mt-auto grid grid-cols-3 gap-2 border-t border-[#f2f4f7] pt-3">
        <HostStat value={segments} label="网段" />
        <HostStat value={processes} label="进程" />
        <HostStat value={stats?.groups} label="分组" />
      </div>
    </div>
  );
}

/** 节点属性里的可选数字：没有这个键就返回 undefined（而不是 0）——
 * 「没测」和「测出来是 0」在资产界面上是两件事。 */
function optionalNumber(properties: Record<string, unknown>, key: string): number | undefined {
  const raw = properties[key];
  if (raw === undefined || raw === null) return undefined;
  const value = Number(raw);
  return Number.isFinite(value) ? value : undefined;
}

function HostStat({ value, label }: { value?: number; label: string }) {
  return (
    <div className="min-w-0">
      <div className="tnum truncate text-[14px] font-semibold leading-5 text-[#101828]">
        {formatCount(value)}
      </div>
      <div className="mt-0.5 text-[9.5px] uppercase leading-none tracking-[0.08em] text-[#98a2b3]">
        {label}
      </div>
    </div>
  );
}

/**
 * 网段卡片的高价值信息不是「它是一个网段」，而是「哪个接口、哪个地址段」。
 * 接口名在边上（host_network_assoc.properties.ifaceName），所以由画布作为 hint 传进来。
 */
function NetworkCard({ node, hint }: { node: HostProcessTopologyNode; hint?: GraphNodeHint }) {
  const cidr = String(node.properties.cidr ?? node.label);
  const family = formatAddressFamily(node.properties.addressFamily);
  const iface = hint?.ifaceName;

  return (
    <div className="flex h-full items-center gap-2.5 px-3.5">
      <span
        className="grid size-[30px] shrink-0 place-items-center rounded-[9px]"
        style={{ backgroundColor: '#ECFDF3', color: '#12B76A' }}
        aria-hidden
      >
        <KindGlyph kind="NetworkSegment" className="size-[16px]" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span
            className={`truncate text-[12px] font-semibold leading-4 ${iface ? 'text-[#101828]' : 'text-[#667085]'}`}
            title={iface ?? cidr}
          >
            {iface ?? '网络接口'}
          </span>
          {family && (
            <span className="ml-auto shrink-0 text-[9px] font-medium uppercase leading-none tracking-[0.06em] text-[#98a2b3]">
              {family}
            </span>
          )}
        </div>
        <div className="mt-1 truncate font-mono text-[10.5px] leading-4 text-[#475467]" title={cidr}>
          {shortCidr(cidr)}
        </div>
      </div>
    </div>
  );
}

/**
 * 运行时卡片把「1,120 个进程 / 705 个程序」和「谁在跑」放进同一张卡。
 * 它同时是展开分组的总开关，所以底部明确写出动作，而不是让人猜节点能不能点。
 */
function RuntimeCard({
  node,
  expanded,
  onExpand,
}: {
  node: HostProcessTopologyNode;
  expanded: boolean;
  onExpand?: () => void;
}) {
  const totalProcesses = numericProperty(node.properties, 'totalProcesses');
  const totalPrograms = numericProperty(node.properties, 'totalPrograms');
  const topPrograms = parseTopPrograms(node.properties.topPrograms).slice(0, 3);

  return (
    <div className="flex h-full flex-col px-4 pb-3.5 pt-4">
      <div className="flex items-center gap-2">
        <span
          className="grid size-[26px] shrink-0 place-items-center rounded-[8px]"
          style={{ backgroundColor: '#EEF4FF', color: '#6366F1' }}
          aria-hidden
        >
          <KindGlyph kind="ProcessSummary" className="size-[15px]" />
        </span>
        <span className="text-[12px] font-semibold leading-4 text-[#101828]">运行时</span>
        <span className="ml-auto rounded-[5px] bg-[#f2f4f7] px-1.5 py-0.5 text-[9.5px] font-medium leading-none text-[#475467]">
          {expanded ? '已展开' : '可展开'}
        </span>
      </div>

      <div className="mt-3.5 flex items-end gap-4">
        <div>
          <div className="tnum text-[26px] font-semibold leading-8 tracking-[-0.02em] text-[#101828]">
            {formatCount(totalProcesses)}
          </div>
          <MicroLabel>进程</MicroLabel>
        </div>
        <div className="mb-1 h-7 w-px bg-[#eaecf0]" aria-hidden />
        <div>
          <div className="tnum text-[15px] font-semibold leading-8 text-[#344054]">
            {formatCount(totalPrograms)}
          </div>
          <MicroLabel>程序</MicroLabel>
        </div>
      </div>

      <div className="mt-3 border-t border-[#f2f4f7] pt-2.5">
        <MicroLabel>高频程序</MicroLabel>
        <ul className="mt-1.5 space-y-1">
          {topPrograms.map((program) => (
            <li key={program.name} className="flex items-baseline gap-2 text-[10.5px] leading-4">
              <span className="min-w-0 flex-1 truncate text-[#344054]" title={program.name}>
                {program.name}
              </span>
              <span className="tnum shrink-0 text-[#98a2b3]">{program.count}</span>
            </li>
          ))}
          {topPrograms.length === 0 && (
            <li className="text-[10.5px] leading-4 text-[#98a2b3]">暂无程序摘要</li>
          )}
        </ul>
      </div>

      <button
        type="button"
        data-expand-action="process-summary"
        onClick={(event) => {
          event.stopPropagation();
          onExpand?.();
        }}
        className="mt-auto flex w-full items-center justify-between rounded-[6px] border-t border-[#f2f4f7] pt-2 text-[10px] font-medium text-[#4f46e5] transition-colors hover:text-[#4338ca]"
      >
        <span>{expanded ? '收起进程分组' : '展开进程分组'}</span>
        <ArrowRightIcon className="size-3.5" />
      </button>
    </div>
  );
}

function ProcessGroupCard({ node }: { node: HostProcessTopologyNode }) {
  const isOverflow = Boolean(node.properties.collapsed) && node.properties.hiddenGroups != null;
  if (isOverflow) {
    const hiddenGroups = numericProperty(node.properties, 'hiddenGroups');
    const loading = Boolean(node.properties.fullGroupsLoading);
    return (
      <div className="flex h-full flex-col items-center justify-center gap-0.5 rounded-xl border border-dashed border-[#d0d5dd] bg-[#f9fafb]/90 px-3 text-center">
        <span className="tnum text-[11px] font-medium leading-4 text-[#475467]">
          {loading ? '正在加载…' : `+ ${formatCount(hiddenGroups)} 个分组`}
        </span>
        <span className="text-[9.5px] leading-3 text-[#98a2b3]">点击继续加载</span>
      </div>
    );
  }

  const executable = String(node.properties.executable ?? '');
  const displayName = executable ? basename(executable) : node.label;
  const processCount = numericProperty(node.properties, 'processCount');
  const memory = numericProperty(node.properties, 'totalMemoryRssMiB');

  return (
    <div className="flex h-full items-center gap-2.5 px-3">
      <span
        className="grid size-[26px] shrink-0 place-items-center rounded-[8px]"
        style={{ backgroundColor: '#F0F9FF', color: '#0BA5EC' }}
        aria-hidden
      >
        <KindGlyph kind="ProcessGroup" className="size-[14px]" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[11.5px] font-medium leading-4 text-[#101828]" title={executable || displayName}>
          {displayName}
        </div>
        <div className="tnum mt-0.5 truncate text-[9.5px] leading-3 text-[#667085]">
          {formatCount(processCount)} 个进程
          {memory > 0 ? ` · ${memory.toFixed(0)} MiB` : ''}
        </div>
      </div>
    </div>
  );
}

function RuntimeProcessCard({ node }: { node: HostProcessTopologyNode }) {
  const state = String(node.properties.state ?? '');
  const pid = node.properties.pid;

  return (
    <div className="flex h-full items-center gap-2 px-3">
      <span
        className="size-1.5 shrink-0 rounded-full"
        style={{ backgroundColor: state && state !== '-' ? '#12B76A' : '#98a2b3' }}
        aria-hidden
      />
      <span className="min-w-0 flex-1 truncate text-[10.5px] font-medium text-[#344054]" title={node.label}>
        {node.label}
      </span>
      {pid != null && <span className="tnum shrink-0 text-[9px] text-[#98a2b3]">{String(pid)}</span>}
    </div>
  );
}

function ServiceCard({ node }: { node: HostProcessTopologyNode }) {
  const instanceCount = numericProperty(node.properties, 'instanceCount');
  const name = String(node.properties.name ?? node.properties.serviceName ?? node.label);
  const endpoints = endpointLabels(node.properties);

  // 溢出节点：图只放 TopN，剩下的一起用这一张卡表示。
  if (node.properties.collapsed === true) {
    const hidden = numericProperty(node.properties, 'hiddenServices');
    const total = numericProperty(node.properties, 'totalServices');
    return (
      <div className="flex h-full items-center gap-2.5 px-3.5">
        <span
          className="grid size-[30px] shrink-0 place-items-center rounded-[9px] border border-dashed"
          style={{ borderColor: '#D6BBFB', color: '#7A5AF8' }}
          aria-hidden
        >
          <KindGlyph kind="ServiceEntity" className="size-[16px]" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="truncate text-[12px] font-semibold leading-4 text-[#101828]">
            {name}
          </div>
          <div className="mt-1 truncate text-[9.5px] leading-3 text-[#667085]">
            {hidden > 0 ? `共 ${total} 个服务 · 点开查看` : `共 ${total} 个服务 · 点收起`}
          </div>
        </div>
      </div>
    );
  }

  // 副标题优先给「在哪儿听」。卡片宽度只有 232px，多个端点挤不下：
  // 只展示对外暴露的那个（没有就取第一个），剩下的折成 `+N`，完整列表放 tooltip。
  const primaryEndpoint =
    endpoints.find((endpoint) => endpoint.external) ?? endpoints[0] ?? null;
  const subtitle = primaryEndpoint
    ? endpoints.length > 1
      ? `${primaryEndpoint.label} +${endpoints.length - 1}`
      : primaryEndpoint.label
    : instanceCount > 0
      ? `${instanceCount} 个实例`
      : '服务';
  const subtitleTitle = endpoints.map((endpoint) => endpoint.label).join(' · ') || subtitle;

  return (
    <div className="flex h-full items-center gap-2.5 px-3.5">
      <span
        className="grid size-[30px] shrink-0 place-items-center rounded-[9px]"
        style={{ backgroundColor: '#F4F3FF', color: '#7A5AF8' }}
        aria-hidden
      >
        <KindGlyph kind="ServiceEntity" className="size-[16px]" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[12px] font-semibold leading-4 text-[#101828]" title={name}>
          {name}
        </div>
        <div
          className="mt-1 truncate font-mono text-[9.5px] leading-3 text-[#667085]"
          title={subtitleTitle}
        >
          {subtitle}
        </div>
      </div>
    </div>
  );
}

/** `endpoints` 是后端按 `{protocol, address, port}` 给的全集。 */
type EndpointLabel = { label: string; external: boolean };

function endpointLabels(properties: Record<string, unknown>): EndpointLabel[] {
  const raw = properties.endpoints;
  if (!Array.isArray(raw)) return [];

  return raw
    .map((item) =>
      typeof item === 'object' && item !== null ? (item as Record<string, unknown>) : null,
    )
    .filter((item): item is Record<string, unknown> => Boolean(item))
    .map((item) => {
      const address = String(item.address ?? '');
      return {
        label: `${String(item.protocol ?? 'tcp')}/${String(item.port ?? '')} ${address}`.trim(),
        // 通配地址才是「对外」；127.0.0.1 / ::1 只有本机能访问。
        external: address === '0.0.0.0' || address === '::',
      };
    })
    .filter((item) => Boolean(item.label));
}

function SubjectCard({ node }: { node: HostProcessTopologyNode }) {
  const role = String(node.properties.role ?? node.properties.title ?? '');

  return (
    <div className="flex h-full items-center gap-2.5 px-3">
      <span
        className="grid size-[28px] shrink-0 place-items-center rounded-full"
        style={{ backgroundColor: '#FDF2FA', color: '#EE46BC' }}
        aria-hidden
      >
        <KindGlyph kind="Subject" className="size-[15px]" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[11.5px] font-medium leading-4 text-[#101828]">{node.label}</div>
        <div className="mt-0.5 text-[9.5px] leading-3 text-[#667085]">{role || '责任人'}</div>
      </div>
    </div>
  );
}
