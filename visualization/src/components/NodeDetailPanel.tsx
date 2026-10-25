import type { HostProcessTopologyNode, HostTopologyDto, LoadedProcessGroupsState } from '../types/domain';
import { getNodeStyle } from '../utils/colors';
import { basename, formatCount, truncate } from '../utils/format';

type Props = {
  node: HostProcessTopologyNode | null;
  hostTopology: HostTopologyDto | null;
  processViewMode: 'overview' | 'expanded' | 'loading' | null;
  loadedProcessGroups: LoadedProcessGroupsState | null;
  onClose: () => void;
};

export default function NodeDetailPanel({
  node,
  hostTopology,
  processViewMode,
  loadedProcessGroups,
  onClose,
}: Props) {
  if (!node) return null;

  const style = getNodeStyle(node.objectKind);
  const showHostTopology = node.objectKind === 'HostInventory' && hostTopology?.host.id === node.objectId;

  return (
    <aside className="absolute inset-y-0 right-0 z-10 flex w-80 flex-col border-l border-line bg-surface">
      <header className="flex items-start justify-between gap-3 border-b border-line px-4 py-3">
        <div className="min-w-0">
          <span
            className="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px] font-medium"
            style={{ backgroundColor: style.fill, color: style.accent }}
          >
            <span className="size-1.5 rounded-full" style={{ backgroundColor: style.stroke }} />
            {style.label}
          </span>
          <h3 className="mt-1.5 truncate text-[13px] font-semibold text-ink" title={node.label}>
            {node.objectKind === 'ProcessGroup'
              ? basename(String(node.properties.executable ?? node.label))
              : node.label}
          </h3>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="关闭"
          className="-mr-1 shrink-0 rounded-md p-1 text-ink-mute transition-colors hover:bg-canvas hover:text-ink"
        >
          <svg viewBox="0 0 20 20" className="size-4" fill="none" stroke="currentColor" strokeWidth="1.8">
            <path d="m5 5 10 10M15 5 5 15" strokeLinecap="round" />
          </svg>
        </button>
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-3.5">
        {showHostTopology ? (
          <HostDetail topology={hostTopology} />
        ) : node.objectKind === 'ProcessGroup' ? (
          <ProcessGroupDetail
            node={node}
            processViewMode={processViewMode}
            loadedProcessGroups={loadedProcessGroups}
          />
        ) : (
          <PropertiesDetail node={node} />
        )}
      </div>
    </aside>
  );
}

function HostDetail({ topology }: { topology: HostTopologyDto }) {
  const os = [topology.host.osName, topology.host.osVersion].filter(Boolean).join(' ');

  return (
    <div className="space-y-4">
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
          <Metric label="服务" value={topology.services.length} />
        </div>
      </Section>

      {topology.networkSegments.length > 0 && (
        <Section title={`网段 ${topology.networkSegments.length}`}>
          <ul className="space-y-1.5">
            {topology.networkSegments.map((segment, index) => {
              const name = String(segment.name ?? segment.cidr ?? '-');
              const family = String(segment.address_family ?? '').toLowerCase();
              return (
                <li
                  key={`${String(segment.network_segment_id ?? index)}`}
                  className="flex items-center justify-between gap-2 rounded-md bg-canvas px-2.5 py-1.5"
                >
                  <span className="truncate font-mono text-[11px] text-ink" title={name}>
                    {truncate(name, 26)}
                  </span>
                  {family && (
                    <span className="shrink-0 rounded bg-white px-1.5 py-0.5 text-[9px] uppercase tracking-wide text-ink-mute">
                      {family}
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        </Section>
      )}

      {topology.services.length > 0 && (
        <Section title={`服务 ${topology.services.length}`}>
          <ul className="space-y-1.5">
            {topology.services.map((service) => (
              <li key={String(service.service.external_ref ?? service.service.name)} className="rounded-md bg-canvas px-2.5 py-1.5">
                <div className="text-[11px] font-medium text-ink">
                  {String(service.service.name ?? '-')}
                </div>
                <div className="tnum text-[10px] text-ink-mute">
                  实例 {service.instances.length} · 绑定进程{' '}
                  {service.instances.reduce((sum, item) => sum + item.processes.length, 0)}
                </div>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {topology.processGroups.length > 0 && (
        <Section title="进程分组 Top 5">
          <ul className="space-y-1">
            {topology.processGroups.slice(0, 5).map((group, index) => (
              <li
                key={`${String(group.display_name)}-${index}`}
                className="flex items-center justify-between gap-2 text-[11px]"
              >
                <span className="truncate text-ink" title={String(group.display_name)}>
                  {String(group.display_name ?? '-')}
                </span>
                <span className="tnum shrink-0 text-ink-mute">
                  {String(group.process_count ?? '-')}
                </span>
              </li>
            ))}
          </ul>
        </Section>
      )}
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
      <div className="space-y-4">
        <Section title="收起的分组">
          <div className="grid grid-cols-2 gap-2">
            <Metric label="隐藏分组" value={hiddenGroups ?? 0} />
            <Metric label="隐藏进程" value={hiddenProcesses ?? 0} />
          </div>
          <p className="mt-2 text-[11px] leading-relaxed text-ink-mute">
            当前只显示进程数最多的前 {displayLimit ?? '-'} 组。点这个节点可以继续加载更多分组。
          </p>
        </Section>
        <ViewModeBadge mode={processViewMode} />
        {processViewMode === 'loading' && (
          <Notice tone="warn">正在加载下一批分组…</Notice>
        )}
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
    <div className="space-y-4">
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
        <Field label="内存">
          {totalMemoryRssKiB != null ? `${formatCount(totalMemoryRssKiB)} KiB` : '—'}
        </Field>
      </Section>

      <ViewModeBadge mode={processViewMode} />
      <GraphId id={node.id} />
    </div>
  );
}

function PropertiesDetail({ node }: { node: HostProcessTopologyNode }) {
  const entries = Object.entries(node.properties);
  return (
    <div className="space-y-4">
      <Section title="属性">
        {entries.length === 0 ? (
          <p className="text-[11px] text-ink-mute">没有属性</p>
        ) : (
          <dl className="space-y-1.5">
            {entries.map(([key, value]) => (
              <div key={key} className="flex items-baseline justify-between gap-3">
                <dt className="shrink-0 text-[10px] text-ink-mute">{key}</dt>
                <dd className="min-w-0 text-right font-mono text-[10px] break-all text-ink-soft">
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
      <summary className="cursor-pointer list-none text-[10px] text-ink-mute transition-colors hover:text-ink-soft">
        图元素 ID
      </summary>
      <p className="mt-1 font-mono text-[10px] break-all text-ink-mute">{id}</p>
    </details>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section>
      <h4 className="mb-2 text-[10px] font-medium uppercase tracking-wider text-ink-mute">{title}</h4>
      <div className="space-y-1.5">{children}</div>
    </section>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-3 text-[11px]">
      <span className="shrink-0 text-ink-mute">{label}</span>
      <span className="min-w-0 text-right text-ink-soft">{children}</span>
    </div>
  );
}

function Metric({ label, value, suffix }: { label: string; value: number; suffix?: string }) {
  return (
    <div className="rounded-md bg-canvas px-2.5 py-2">
      <div className="text-[10px] text-ink-mute">{label}</div>
      <div className="tnum text-[15px] font-semibold leading-tight text-ink">
        {formatCount(value)}
        {suffix && <span className="ml-0.5 text-[10px] font-normal text-ink-mute">{suffix}</span>}
      </div>
    </div>
  );
}

function Notice({ tone, children }: { tone: 'info' | 'warn'; children: React.ReactNode }) {
  const className =
    tone === 'warn'
      ? 'border-amber-200 bg-amber-50 text-amber-800'
      : 'border-brand/25 bg-brand-soft text-brand';
  return <div className={`rounded-md border px-3 py-2 text-[11px] ${className}`}>{children}</div>;
}

function ViewModeBadge({ mode }: { mode: 'overview' | 'expanded' | 'loading' | null }) {
  if (!mode) return null;

  const text =
    mode === 'loading' ? '正在加载更多分组' : mode === 'expanded' ? '已展开分组' : '概览视图';
  const className =
    mode === 'loading'
      ? 'border-amber-200 bg-amber-50 text-amber-800'
      : mode === 'expanded'
        ? 'border-brand/25 bg-brand-soft text-brand'
        : 'border-line bg-canvas text-ink-soft';

  return <div className={`rounded-md border px-3 py-2 text-[11px] ${className}`}>{text}</div>;
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

function asNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}
