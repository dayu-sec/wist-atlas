import type { HostProcessTopologyNode, HostTopologyDto, LoadedProcessGroupsState } from '../types/domain';
import { getLayerStyle } from '../utils/colors';

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

  const style = getLayerStyle(node.objectKind);
  const showHostTopology = node.objectKind === 'HostInventory' && hostTopology?.host.id === node.objectId;
  const showProcessGroupDetail = node.objectKind === 'ProcessGroup';

  return (
    <div className="absolute right-0 top-0 h-full w-72 bg-white border-l border-gray-200 shadow-lg overflow-y-auto z-10">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-100">
        <div className="flex items-center gap-2">
          <span
            className="inline-block w-3 h-3 rounded-sm"
            style={{ backgroundColor: style.bg, border: `1.5px solid ${style.border}` }}
          />
          <span className="text-sm font-medium text-gray-800">{node.objectKind}</span>
        </div>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 text-lg leading-none"
        >
          &times;
        </button>
      </div>

      {/* Body */}
      <div className="p-4 space-y-3">
        {showHostTopology ? (
          <>
            <div>
              <span className="text-xs text-gray-400 block">Host</span>
              <span className="text-sm text-gray-800 font-medium">{hostTopology.host.hostName}</span>
            </div>
            <div>
              <span className="text-xs text-gray-400 block">Machine ID</span>
              <span className="text-sm text-gray-600 font-mono text-xs break-all">
                {hostTopology.host.machineId ?? '-'}
              </span>
            </div>
            <div className="grid grid-cols-3 gap-2 pt-2 border-t border-gray-100">
              <SummaryItem label="Networks" value={hostTopology.networkSegments.length} />
              <SummaryItem label="Processes" value={hostTopology.processes.length} />
              <SummaryItem label="Services" value={hostTopology.services.length} />
            </div>
            {hostTopology.networkSegments.length > 0 && (
              <div className="pt-2 border-t border-gray-100">
                <span className="text-xs text-gray-400 block mb-2">Networks</span>
                <div className="space-y-2">
                  {hostTopology.networkSegments.slice(0, 8).map((segment, index) => (
                    <div key={`${String(segment.network_segment_id ?? segment.cidr ?? index)}`} className="rounded border border-gray-100 bg-gray-50 p-2">
                      <div className="text-xs font-medium text-gray-800 break-all">
                        {String(segment.name ?? segment.cidr ?? '-')}
                      </div>
                      <div className="text-[11px] text-gray-500">
                        {String(segment.address_family ?? '-')}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
            {hostTopology.services.length > 0 && (
              <div className="pt-2 border-t border-gray-100">
                <span className="text-xs text-gray-400 block mb-2">Services</span>
                <div className="space-y-2">
                  {hostTopology.services.map((service) => (
                    <div key={String(service.service.external_ref ?? service.service.name)} className="rounded border border-gray-100 bg-gray-50 p-2">
                      <div className="text-xs font-medium text-gray-800">
                        {String(service.service.name ?? '-')}
                      </div>
                      <div className="text-[11px] text-gray-500">
                        instances {service.instances.length}
                      </div>
                      <div className="text-[11px] text-gray-500">
                        bound processes {service.instances.reduce((sum, item) => sum + item.processes.length, 0)}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
            {hostTopology.processGroups.length > 0 && (
              <div className="pt-2 border-t border-gray-100">
                <span className="text-xs text-gray-400 block mb-2">Top Process Groups</span>
                <div className="space-y-2">
                  {hostTopology.processGroups.slice(0, 5).map((group, index) => (
                    <div key={`${String(group.display_name)}-${index}`} className="rounded border border-gray-100 p-2">
                      <div className="text-xs font-medium text-gray-800">{String(group.display_name ?? '-')}</div>
                      <div className="text-[11px] text-gray-500">
                        count {String(group.process_count ?? '-')} · rss {String(group.total_memory_rss_kib ?? '-')} KiB
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </>
        ) : showProcessGroupDetail ? (
          <ProcessGroupDetail
            node={node}
            processViewMode={processViewMode}
            loadedProcessGroups={loadedProcessGroups}
          />
        ) : (
          <>
        <div>
          <span className="text-xs text-gray-400 block">Label</span>
          <span className="text-sm text-gray-800 font-medium">{node.label}</span>
        </div>
        <div>
          <span className="text-xs text-gray-400 block">Graph ID</span>
          <span className="text-sm text-gray-600 font-mono text-xs break-all">{node.id}</span>
        </div>
        <div>
          <span className="text-xs text-gray-400 block">Object ID</span>
          <span className="text-sm text-gray-600 font-mono text-xs break-all">{node.objectId}</span>
        </div>
        <div>
          <span className="text-xs text-gray-400 block">Layer</span>
          <span className="text-sm text-gray-600">{node.layer}</span>
        </div>

        {Object.keys(node.properties).length > 0 && (
          <div className="pt-2 border-t border-gray-100">
            <span className="text-xs text-gray-400 block mb-1">Properties</span>
            {Object.entries(node.properties).map(([key, value]) => (
              <div key={key} className="py-1">
                <span className="text-xs text-gray-500 block">{key}</span>
                <span className="text-xs text-gray-700 font-mono break-all">
                  {formatValue(value)}
                </span>
              </div>
            ))}
          </div>
        )}
          </>
        )}
      </div>
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
      <>
        <div>
          <span className="text-xs text-gray-400 block">Collapsed Groups</span>
          <span className="text-sm text-gray-800 font-medium">{node.label}</span>
        </div>
        <div className="grid grid-cols-2 gap-2 pt-2 border-t border-gray-100">
          <SummaryItem label="Hidden Groups" value={hiddenGroups ?? 0} />
          <SummaryItem label="Hidden Processes" value={hiddenProcesses ?? 0} />
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">Display Policy</div>
          <div className="text-xs text-gray-700">
            Showing top {displayLimit ?? '-'} groups by process count. Click this node to append more process groups into the graph.
          </div>
        </div>
        <ViewModeBadge mode={processViewMode} />
        {processViewMode === 'loading' && (
          <div className="rounded border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-700">
            Loading next process group page...
          </div>
        )}
        {loadedProcessGroups && (
          <div className="rounded border border-blue-100 bg-blue-50 px-3 py-2 text-xs text-blue-700">
            Loaded {loadedProcessGroups.groups.length} additional groups
            {loadedProcessGroups.totalGroups > 0 ? ` / ${loadedProcessGroups.totalGroups}` : ''}
            {loadedProcessGroups.hasMore ? `, ${loadedProcessGroups.totalGroups - loadedProcessGroups.offset} still hidden` : ', all groups loaded'}
          </div>
        )}
      </>
    );
  }

  return (
    <>
      <div>
        <span className="text-xs text-gray-400 block">Process Group</span>
        <span className="text-sm text-gray-800 font-medium">{node.label}</span>
      </div>
      <div>
        <span className="text-xs text-gray-400 block">Executable</span>
        <span className="text-sm text-gray-600 font-mono text-xs break-all">{executable ?? '-'}</span>
      </div>
      <ViewModeBadge mode={processViewMode} />
      <div className="grid grid-cols-2 gap-2 pt-2 border-t border-gray-100">
        <SummaryItem label="Processes" value={processCount ?? 0} />
        <SummaryItem label="RSS MiB" value={totalMemoryRssMiB ?? 0} valueClassName="text-sm" />
      </div>
      <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
        <div className="text-[11px] text-gray-500">Memory</div>
        <div className="text-xs text-gray-700">
          {totalMemoryRssKiB != null ? `${totalMemoryRssKiB} KiB` : '-'}
        </div>
      </div>
      <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
        <div className="text-[11px] text-gray-500">Dominant State</div>
        <div className="text-xs text-gray-700">{dominantState ?? '-'}</div>
      </div>
      <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
        <div className="text-[11px] text-gray-500">State Summary</div>
        <div className="text-xs text-gray-700">{states ?? '-'}</div>
      </div>
      <div className="pt-2 border-t border-gray-100">
        <span className="text-xs text-gray-400 block mb-1">Graph ID</span>
        <span className="text-xs text-gray-600 font-mono break-all">{node.id}</span>
      </div>
    </>
  );
}

function SummaryItem({
  label,
  value,
  valueClassName,
}: {
  label: string;
  value: number;
  valueClassName?: string;
}) {
  return (
    <div className="rounded border border-gray-100 bg-gray-50 px-2 py-2">
      <div className="text-[11px] text-gray-500">{label}</div>
      <div className={valueClassName ?? 'text-sm font-medium text-gray-800'}>{value}</div>
    </div>
  );
}

function ViewModeBadge({ mode }: { mode: 'overview' | 'expanded' | 'loading' | null }) {
  if (!mode) return null;

  const text =
    mode === 'loading'
      ? 'Loading more groups'
      : mode === 'expanded'
        ? 'Expanded groups view'
        : 'Overview view';

  const className =
    mode === 'loading'
      ? 'border-amber-200 bg-amber-50 text-amber-700'
      : mode === 'expanded'
        ? 'border-blue-200 bg-blue-50 text-blue-700'
        : 'border-gray-200 bg-gray-50 text-gray-600';

  return (
    <div className={`rounded border px-3 py-2 text-xs ${className}`}>
      {text}
    </div>
  );
}

function formatValue(value: unknown) {
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (value == null) return '-';
  return String(value);
}

function asString(value: unknown): string | null {
  if (typeof value !== 'string' || value.length === 0) return null;
  return value;
}

function asNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}
