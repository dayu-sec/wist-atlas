import { useEffect, useState, useCallback } from 'react';
import { useAppState } from './hooks/useAppState';
import TopologyCanvas from './components/TopologyCanvas';
import CanvasToolbar from './components/CanvasToolbar';
import LegendCard from './components/LegendCard';
import HostSummaryCard from './components/HostSummaryCard';
import StatusBar from './components/StatusBar';
import NodeDetailPanel from './components/NodeDetailPanel';
import {
  fetchHostProcessGroupsPage,
  fetchHostProcessOverview,
  fetchFirstHostProcessTopology,
  fetchHostProcessTopology,
  fetchHostTopology,
  fetchNetworkTopology,
  loadFixture,
} from './api/client';
import type {
  HostProcessGroupDto,
  LoadedProcessGroupsState,
  HostProcessTopologyEdge,
  HostProcessTopologyGraph,
  HostProcessTopologyNode,
  NodeKind,
} from './types/domain';

const PROCESS_SUMMARY_PREFIX = 'process-summary:';
const PROCESS_GROUP_PREFIX = 'process-group:';
const PROCESS_GROUP_OVERFLOW_PREFIX = 'process-group-overflow:';
const PROCESS_EXPAND_LIMIT = 20;
const PROCESS_GROUP_DISPLAY_LIMIT = 12;
const PROCESS_GROUP_PAGE_SIZE = 20;
const DEFAULT_HOST_ID = import.meta.env.VITE_DEFAULT_HOST_ID as string | undefined;
const USE_FIXTURE = import.meta.env.VITE_USE_FIXTURE === '1';

export default function App() {
  const [state, dispatch] = useAppState();
  const [hostId, setHostId] = useState('');
  const [networkId, setNetworkId] = useState('');
  const [expandedHosts, setExpandedHosts] = useState<Set<string>>(new Set());
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [overviewGroupsByHost, setOverviewGroupsByHost] = useState<Record<string, HostProcessGroupDto[]>>({});
  const [truncatedGroupsByHost, setTruncatedGroupsByHost] = useState<Record<string, number>>({});
  const [loadedGroupsByHost, setLoadedGroupsByHost] = useState<Record<string, LoadedProcessGroupsState>>({});
  const [groupPageLoadingByHost, setGroupPageLoadingByHost] = useState<Record<string, boolean>>({});

  const loadData = useCallback(() => {
    dispatch({ type: 'FETCH_GRAPH_START' });
    const done = (res: Awaited<ReturnType<typeof fetchHostProcessTopology>>) => {
      if (res.status === 'ok') {
        setExpandedHosts(new Set());
        setExpandedGroups(new Set());
        setOverviewGroupsByHost({});
        setTruncatedGroupsByHost({});
        setLoadedGroupsByHost({});
        setGroupPageLoadingByHost({});
        dispatch({ type: 'FETCH_GRAPH_SUCCESS', graph: res.data });
        dispatch({ type: 'FETCH_HOST_TOPOLOGY_SUCCESS', hostTopology: null });
      } else {
        dispatch({ type: 'FETCH_GRAPH_ERROR', error: res.message });
      }
    };

    if (USE_FIXTURE) {
      loadFixture('live-topology').then(done);
      return;
    }

    const host = hostId.trim() || DEFAULT_HOST_ID;
    if (!host) {
      fetchFirstHostProcessTopology().then(done);
      return;
    }
    fetchHostProcessTopology(host).then(done);
  }, [dispatch, hostId]);

  // Load fixture data on first mount so there is something to render.
  useEffect(() => {
    loadData();
  }, [loadData]);

  const queryHost = useCallback(() => {
    const trimmed = hostId.trim();
    if (!trimmed) return;
    dispatch({ type: 'FETCH_GRAPH_START' });
    fetchHostProcessTopology(trimmed).then((res) => {
      if (res.status === 'ok') {
        setExpandedHosts(new Set());
        setExpandedGroups(new Set());
        setOverviewGroupsByHost({});
        setTruncatedGroupsByHost({});
        setLoadedGroupsByHost({});
        setGroupPageLoadingByHost({});
        dispatch({ type: 'FETCH_GRAPH_SUCCESS', graph: res.data });
      } else {
        dispatch({ type: 'FETCH_GRAPH_ERROR', error: `[${res.code}] ${res.message}` });
      }
    });
  }, [hostId, dispatch]);

  const queryNetwork = useCallback(() => {
    const trimmed = networkId.trim();
    if (!trimmed) return;
    dispatch({ type: 'FETCH_GRAPH_START' });
    fetchNetworkTopology(trimmed).then((res) => {
      if (res.status === 'ok') {
        setExpandedHosts(new Set());
        setExpandedGroups(new Set());
        setOverviewGroupsByHost({});
        setTruncatedGroupsByHost({});
        setLoadedGroupsByHost({});
        setGroupPageLoadingByHost({});
        dispatch({ type: 'FETCH_GRAPH_SUCCESS', graph: res.data });
      } else {
        dispatch({ type: 'FETCH_GRAPH_ERROR', error: `[${res.code}] ${res.message}` });
      }
    });
  }, [networkId, dispatch]);

  const displayGraph = buildDisplayGraph(
    state.graph,
    expandedHosts,
    expandedGroups,
    overviewGroupsByHost,
    truncatedGroupsByHost,
    loadedGroupsByHost,
    groupPageLoadingByHost,
  );
  const selectedNode =
    displayGraph?.nodes.find((n) => n.id === state.selectedNodeId) ?? null;

  // 左栏的资产卡要的是「当前这张图的主机」，不是「当前选中的节点」。
  // 之前 `hostTopology` 绑在选中项上：一点网段，资产卡就跟着空了。
  const primaryHostNode =
    displayGraph?.nodes.find((node) => node.objectKind === 'HostInventory') ?? null;

  useEffect(() => {
    const hostObjectId = primaryHostNode?.objectId;
    if (!hostObjectId) {
      dispatch({ type: 'FETCH_HOST_TOPOLOGY_SUCCESS', hostTopology: null });
      return;
    }
    if (state.hostTopology?.host.id === hostObjectId) return;

    fetchHostTopology(hostObjectId).then((res) => {
      if (res.status === 'ok') {
        dispatch({ type: 'FETCH_HOST_TOPOLOGY_SUCCESS', hostTopology: res.data });
      }
    });
  }, [primaryHostNode?.objectId, state.hostTopology?.host.id, dispatch]);

  useEffect(() => {
    if (USE_FIXTURE) return;
    if (!state.graph) return;

    const hostNodes = state.graph.nodes.filter((node) => node.objectKind === 'HostInventory');
    for (const hostNode of hostNodes) {
      if (overviewGroupsByHost[hostNode.objectId]) continue;
      fetchHostProcessOverview(hostNode.objectId, PROCESS_GROUP_DISPLAY_LIMIT).then((res) => {
        if (res.status !== 'ok') return;
        setOverviewGroupsByHost((current) => {
          if (current[hostNode.objectId]) return current;
          return { ...current, [hostNode.objectId]: res.data.topGroups };
        });
        setTruncatedGroupsByHost((current) => {
          if (current[hostNode.objectId] != null) return current;
          return { ...current, [hostNode.objectId]: res.data.truncatedGroupCount };
        });
      });
    }
  }, [state.graph, overviewGroupsByHost]);

  const handleSelectNode = useCallback((nodeId: string | null) => {
    if (nodeId && nodeId.startsWith(PROCESS_SUMMARY_PREFIX)) {
      const hostNodeId = nodeId.slice(PROCESS_SUMMARY_PREFIX.length);
      setExpandedHosts((current) => {
        const next = new Set(current);
        if (next.has(hostNodeId)) {
          next.delete(hostNodeId);
        } else {
          next.add(hostNodeId);
        }
        return next;
      });
      dispatch({ type: 'SELECT_NODE', nodeId: null });
      return;
    }
    if (nodeId && nodeId.startsWith(PROCESS_GROUP_OVERFLOW_PREFIX)) {
      const hostNodeId = nodeId.slice(PROCESS_GROUP_OVERFLOW_PREFIX.length);
      const hostNode = state.graph?.nodes.find((node) => node.id === hostNodeId);
      dispatch({ type: 'SELECT_NODE', nodeId });
      if (hostNode && !groupPageLoadingByHost[hostNode.objectId]) {
        const hostObjectId = hostNode.objectId;
        const loadedState = loadedGroupsByHost[hostObjectId];
        const overviewCount = overviewGroupsByHost[hostObjectId]?.length ?? 0;
        if (loadedState && !loadedState.hasMore) {
          return;
        }
        const offset = loadedState?.offset ?? overviewCount;
        setGroupPageLoadingByHost((current) => ({ ...current, [hostObjectId]: true }));
        fetchHostProcessGroupsPage(hostObjectId, PROCESS_GROUP_PAGE_SIZE, offset).then((res) => {
          setGroupPageLoadingByHost((current) => ({ ...current, [hostObjectId]: false }));
          if (res.status !== 'ok') return;
          setLoadedGroupsByHost((current) => {
            const previous = current[hostObjectId];
            const existingGroups = previous?.groups ?? [];
            const mergedGroups = [...existingGroups];
            for (const group of res.data.groups) {
              if (!mergedGroups.some((item) => item.executable === group.executable)) {
                mergedGroups.push(group);
              }
            }
            return {
              ...current,
              [hostObjectId]: {
                groups: mergedGroups,
                offset: res.data.offset + res.data.groups.length,
                limit: res.data.limit,
                hasMore: res.data.hasMore,
                totalGroups: res.data.totalGroups,
              },
            };
          });
        });
      }
      return;
    }
    if (nodeId && nodeId.startsWith(PROCESS_GROUP_PREFIX)) {
      setExpandedGroups((current) => {
        const next = new Set(current);
        if (next.has(nodeId)) {
          next.delete(nodeId);
        } else {
          next.add(nodeId);
        }
        return next;
      });
      dispatch({ type: 'SELECT_NODE', nodeId });
      return;
    }
    dispatch({ type: 'SELECT_NODE', nodeId });
  }, [dispatch, groupPageLoadingByHost, loadedGroupsByHost, overviewGroupsByHost, state.graph]);

  const railHost = state.hostTopology?.host ?? (primaryHostNode
    ? {
        id: primaryHostNode.objectId,
        hostName: primaryHostNode.label,
        machineId: null,
        osName: (primaryHostNode.properties.osName as string | undefined) ?? null,
        osVersion: (primaryHostNode.properties.osVersion as string | undefined) ?? null,
        arch: null,
      }
    : null);

  const graphNodes = displayGraph?.nodes ?? [];
  const graphEdges = displayGraph?.edges ?? [];
  const presentKinds: NodeKind[] = [];
  for (const node of graphNodes) {
    if (!presentKinds.includes(node.objectKind)) presentKinds.push(node.objectKind);
  }

  const segmentCount = state.hostTopology?.networkSegments.length
    ?? graphNodes.filter((node) => node.objectKind === 'NetworkSegment').length;
  const processCount = state.hostTopology?.processes.length
    ?? Number(displayGraph?.metadata?.processCount ?? 0);
  const groupCount = state.hostTopology?.processGroups.length
    ?? graphNodes.filter((node) => node.objectKind === 'ProcessGroup').length;
  const serviceCount = state.hostTopology?.services.length
    ?? graphNodes.filter((node) => node.objectKind === 'ServiceEntity').length;
  const hostCount = Number(displayGraph?.metadata?.hostCount ?? (primaryHostNode ? 1 : 0));

  const query = state.searchQuery.trim().toLowerCase();
  const matchCount = query
    ? graphNodes.filter((node) => {
        const haystack = [node.label, ...Object.values(node.properties).map(String)]
          .join(' ')
          .toLowerCase();
        return haystack.includes(query);
      }).length
    : null;

  return (
    <div className="flex h-screen flex-col overflow-hidden">
      <header className="flex h-14 shrink-0 items-center gap-3 bg-slate-900 px-5 text-slate-100">
        <div className="flex items-center gap-2">
          <svg viewBox="0 0 20 20" className="size-5 text-indigo-400" fill="none" aria-hidden>
            <path d="M10 1.6 18.4 10 10 18.4 1.6 10Z" stroke="currentColor" strokeWidth="1.4" />
            <circle cx="10" cy="10" r="2.4" fill="currentColor" />
          </svg>
          <span className="text-[13px] font-semibold tracking-wide">wist-atlas</span>
        </div>

        <span className="h-5 w-px bg-slate-700" aria-hidden />

        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-[13px] font-medium text-slate-200">
            {railHost?.hostName ?? '未选择主机'}
          </span>
          {railHost?.osName && (
            <span className="shrink-0 rounded bg-slate-800 px-1.5 py-0.5 text-[10px] text-slate-300">
              {railHost.osName} {railHost.osVersion ?? ''}
            </span>
          )}
          {railHost?.arch && (
            <span className="shrink-0 rounded bg-slate-800 px-1.5 py-0.5 font-mono text-[10px] text-slate-400">
              {railHost.arch}
            </span>
          )}
        </div>

        <div className="ml-auto flex items-center gap-3">
          <div
            className={`h-0.5 w-24 overflow-hidden rounded-full bg-slate-800 ${state.loading ? '' : 'opacity-0'}`}
            aria-hidden
          >
            <div className="h-full w-1/2 animate-pulse rounded-full bg-indigo-400" />
          </div>
          <button
            type="button"
            onClick={loadData}
            disabled={state.loading}
            className="flex h-8 items-center gap-1.5 rounded-md bg-slate-800 px-3 text-[12px] font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <svg viewBox="0 0 20 20" className="size-3.5" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="M16 10a6 6 0 1 1-1.8-4.3M16 3.5V7h-3.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            刷新
          </button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <aside className="flex w-72 shrink-0 flex-col overflow-y-auto border-r border-line bg-surface">
          <HostSummaryCard
            host={railHost}
            segmentCount={segmentCount}
            processCount={processCount}
            groupCount={groupCount}
            serviceCount={serviceCount}
            generatedAt={state.hostTopology?.generatedAt ?? displayGraph?.metadata?.queryTime ?? null}
            loading={state.loading}
          />

          <LegendCard kinds={presentKinds} />

          <details className="group px-4 py-3.5">
            <summary className="flex cursor-pointer list-none items-center gap-1.5 text-[11px] text-ink-mute transition-colors hover:text-ink-soft">
              <svg
                viewBox="0 0 20 20"
                className="size-3 transition-transform group-open:rotate-90"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="m8 5 5 5-5 5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              按对象 ID 查询
            </summary>

            <div className="mt-3 space-y-3">
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  queryHost();
                }}
                className="space-y-1.5"
              >
                <label className="block text-[10px] text-ink-mute" htmlFor="host-id-input">
                  主机对象 ID
                </label>
                <div className="flex gap-1.5">
                  <input
                    id="host-id-input"
                    type="text"
                    placeholder="UUID"
                    value={hostId}
                    onChange={(e) => setHostId(e.target.value)}
                    className="h-7 min-w-0 flex-1 rounded-md border border-line bg-canvas px-2 font-mono text-[11px] text-ink placeholder:text-ink-mute focus:border-brand focus:bg-surface focus:outline-none focus:ring-2 focus:ring-brand/15"
                  />
                  <button
                    type="submit"
                    className="h-7 shrink-0 rounded-md bg-brand px-2.5 text-[11px] font-medium text-white transition-colors hover:bg-indigo-700"
                  >
                    查询
                  </button>
                </div>
              </form>

              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  queryNetwork();
                }}
                className="space-y-1.5"
              >
                <label className="block text-[10px] text-ink-mute" htmlFor="network-id-input">
                  网段对象 ID
                </label>
                <div className="flex gap-1.5">
                  <input
                    id="network-id-input"
                    type="text"
                    placeholder="UUID"
                    value={networkId}
                    onChange={(e) => setNetworkId(e.target.value)}
                    className="h-7 min-w-0 flex-1 rounded-md border border-line bg-canvas px-2 font-mono text-[11px] text-ink placeholder:text-ink-mute focus:border-brand focus:bg-surface focus:outline-none focus:ring-2 focus:ring-brand/15"
                  />
                  <button
                    type="submit"
                    className="h-7 shrink-0 rounded-md border border-line bg-surface px-2.5 text-[11px] font-medium text-ink-soft transition-colors hover:border-line-strong"
                  >
                    查询
                  </button>
                </div>
              </form>
            </div>
          </details>
        </aside>

        <main className="flex min-w-0 flex-1 flex-col">
          <CanvasToolbar
            searchQuery={state.searchQuery}
            onSearchChange={(q) => dispatch({ type: 'SET_SEARCH', query: q })}
            layerVisibility={state.layerVisibility}
            onToggleLayer={(l) => dispatch({ type: 'TOGGLE_LAYER', layer: l })}
            layout={state.layout}
            onLayoutChange={(l) => dispatch({ type: 'SET_LAYOUT', layout: l })}
            matchCount={matchCount}
          />

          <div className="relative min-h-0 flex-1">
            <TopologyCanvas
              graph={displayGraph}
              selectedNodeId={state.selectedNodeId}
              onSelectNode={handleSelectNode}
              searchQuery={state.searchQuery}
              layout={state.layout}
            />
            <NodeDetailPanel
              node={selectedNode}
              hostTopology={state.hostTopology}
              processViewMode={getProcessViewMode(selectedNode, loadedGroupsByHost, groupPageLoadingByHost)}
              loadedProcessGroups={getSelectedLoadedProcessGroups(selectedNode, loadedGroupsByHost)}
              onClose={() => dispatch({ type: 'SELECT_NODE', nodeId: null })}
            />

            {state.error && (
              <div className="absolute bottom-3 left-3 z-20 flex max-w-md items-start gap-2 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-[11px] text-rose-800 shadow-sm">
                <span className="mt-0.5 font-semibold">出错</span>
                <span className="min-w-0 break-all">{state.error}</span>
                <button
                  type="button"
                  onClick={() => dispatch({ type: 'FETCH_GRAPH_ERROR', error: '' })}
                  className="ml-auto shrink-0 rounded px-1 text-rose-400 hover:text-rose-700"
                  aria-label="关闭错误提示"
                >
                  ✕
                </button>
              </div>
            )}
          </div>
        </main>
      </div>

      <StatusBar
        hostCount={hostCount}
        segmentCount={segmentCount}
        processCount={processCount}
        nodeCount={graphNodes.length}
        edgeCount={graphEdges.length}
        queryTime={displayGraph?.metadata?.queryTime ?? null}
        loading={state.loading}
      />
    </div>
  );
}

function buildDisplayGraph(
  source: HostProcessTopologyGraph | null,
  expandedHosts: Set<string>,
  expandedGroups: Set<string>,
  overviewGroupsByHost: Record<string, HostProcessGroupDto[]>,
  truncatedGroupsByHost: Record<string, number>,
  loadedGroupsByHost: Record<string, LoadedProcessGroupsState>,
  groupPageLoadingByHost: Record<string, boolean>,
): HostProcessTopologyGraph | null {
  if (!source) return null;

  const nodesById = new Map(source.nodes.map((node) => [node.id, node]));
  const processSummaryEdgesByHost = new Map<string, HostProcessTopologyEdge[]>();
  const processGroupEdgesBySummary = new Map<string, HostProcessTopologyEdge[]>();
  const processEdgesByGroup = new Map<string, HostProcessTopologyEdge[]>();
  const hostServiceEdgesByHost = new Map<string, HostProcessTopologyEdge[]>();
  const serviceInstanceEdgesByInstance = new Map<string, HostProcessTopologyEdge[]>();
  const processServiceEdgesByProcess = new Map<string, HostProcessTopologyEdge[]>();

  for (const edge of source.edges) {
    if (edge.edgeKind === 'host_process_assoc') {
      if (edge.target.startsWith(PROCESS_SUMMARY_PREFIX)) {
        const edges = processSummaryEdgesByHost.get(edge.source) ?? [];
        edges.push(edge);
        processSummaryEdgesByHost.set(edge.source, edges);
        continue;
      }
      if (edge.source.startsWith(PROCESS_SUMMARY_PREFIX) && edge.target.startsWith(PROCESS_GROUP_PREFIX)) {
        const edges = processGroupEdgesBySummary.get(edge.source) ?? [];
        edges.push(edge);
        processGroupEdgesBySummary.set(edge.source, edges);
        continue;
      }
      if (edge.source.startsWith(PROCESS_GROUP_PREFIX)) {
        const edges = processEdgesByGroup.get(edge.source) ?? [];
        edges.push(edge);
        processEdgesByGroup.set(edge.source, edges);
      }
      continue;
    }
    if (edge.edgeKind === 'host_service_assoc') {
      const edges = hostServiceEdgesByHost.get(edge.source) ?? [];
      edges.push(edge);
      hostServiceEdgesByHost.set(edge.source, edges);
      continue;
    }
    if (edge.edgeKind === 'service_instance_assoc') {
      const edges = serviceInstanceEdgesByInstance.get(edge.source) ?? [];
      edges.push(edge);
      serviceInstanceEdgesByInstance.set(edge.source, edges);
      continue;
    }
    if (edge.edgeKind === 'process_service_assoc') {
      const edges = processServiceEdgesByProcess.get(edge.source) ?? [];
      edges.push(edge);
      processServiceEdgesByProcess.set(edge.source, edges);
    }
  }

  const displayNodes: HostProcessTopologyNode[] = [];
  const displayEdges: HostProcessTopologyEdge[] = [];
  const includedNodeIds = new Set<string>();
  const visibleProcessNodeIds = new Set<string>();
  const visibleHostNodeIds = new Set<string>();
  const runtimeBoundServiceNodeIds = new Set<string>();

  const addNode = (node: HostProcessTopologyNode | undefined) => {
    if (!node || includedNodeIds.has(node.id)) return;
    displayNodes.push(node);
    includedNodeIds.add(node.id);
  };

  const addEdge = (edge: HostProcessTopologyEdge) => {
    if (!includedNodeIds.has(edge.source) || !includedNodeIds.has(edge.target)) return;
    if (displayEdges.some((item) => item.id === edge.id)) return;
    displayEdges.push(edge);
  };

  for (const node of source.nodes) {
    if (node.objectKind !== 'HostInventory' && node.objectKind !== 'NetworkSegment' && node.objectKind !== 'Subject') {
      continue;
    }
    addNode(node);
    if (node.objectKind === 'HostInventory') {
      visibleHostNodeIds.add(node.id);
    }
  }

  for (const edge of source.edges) {
    if (edge.edgeKind === 'host_process_assoc') continue;
    if (edge.edgeKind === 'host_service_assoc') continue;
    if (edge.edgeKind === 'service_instance_assoc') continue;
    if (edge.edgeKind === 'process_service_assoc') continue;
    addEdge(edge);
  }

  for (const [hostNodeId, summaryEdges] of processSummaryEdgesByHost.entries()) {
    for (const summaryEdge of summaryEdges) {
      const summaryNode = nodesById.get(summaryEdge.target);
      if (!summaryNode) continue;

      const groupEdges = processGroupEdgesBySummary.get(summaryNode.id) ?? [];
      const singleGroupEdge = groupEdges.length === 1 ? groupEdges[0] : null;
      const singleGroupNode = singleGroupEdge ? nodesById.get(singleGroupEdge.target) : undefined;
      const processEdges = singleGroupNode ? (processEdgesByGroup.get(singleGroupNode.id) ?? []) : [];
      const canCollapseSingleProcess = groupEdges.length === 1 && processEdges.length === 1;

      if (canCollapseSingleProcess) {
        const processEdge = processEdges[0];
        const processNode = nodesById.get(processEdge.target);
        addNode(processNode);
        if (processNode) {
          visibleProcessNodeIds.add(processNode.id);
          addEdge({
            id: `edge:${hostNodeId}:${processNode.id}:collapsed`,
            edgeKind: 'host_process_assoc',
            source: hostNodeId,
            target: processNode.id,
            properties: {
              collapsed: true,
              originalSummaryId: summaryNode.id,
              originalGroupId: singleGroupNode?.id ?? '',
            },
          });
        }
        continue;
      }

      addNode(summaryNode);
      addEdge(summaryEdge);

      const isExpanded = expandedHosts.has(hostNodeId);
      if (!isExpanded) continue;

      const hostObjectId = summaryNode.objectId;
      const visibleGroups = [
        ...(overviewGroupsByHost[hostObjectId] ?? []),
        ...(loadedGroupsByHost[hostObjectId]?.groups ?? []),
      ];
      const rankedGroupEdges = [...groupEdges].sort((left, right) => {
        const leftNode = nodesById.get(left.target);
        const rightNode = nodesById.get(right.target);
        const leftCount = Number(leftNode?.properties.processCount ?? 0);
        const rightCount = Number(rightNode?.properties.processCount ?? 0);
        return rightCount - leftCount;
      });
      const visibleGroupEdges = visibleGroups.length > 0
        ? rankedGroupEdges.filter((edge) => {
            const node = nodesById.get(edge.target);
            const executable = typeof node?.properties.executable === 'string'
              ? node.properties.executable
              : '';
            return visibleGroups.some((group) => group.executable === executable);
          })
        : rankedGroupEdges.slice(0, PROCESS_GROUP_DISPLAY_LIMIT);
      const visibleGroupEdgeIds = new Set(visibleGroupEdges.map((edge) => edge.id));

      for (const groupEdge of visibleGroupEdges) {
        const groupNode = nodesById.get(groupEdge.target);
        if (!groupNode) continue;

        addNode(groupNode);
        addEdge(groupEdge);

        const isGroupExpanded = expandedGroups.has(groupNode.id);
        if (!isGroupExpanded) continue;

        const groupedProcessEdges = processEdgesByGroup.get(groupNode.id) ?? [];
        for (const processEdge of groupedProcessEdges.slice(0, PROCESS_EXPAND_LIMIT)) {
          const processNode = nodesById.get(processEdge.target);
          if (!processNode) continue;
          addNode(processNode);
          addEdge(processEdge);
          visibleProcessNodeIds.add(processNode.id);
        }
      }

      const totalGroups =
        (
          loadedGroupsByHost[hostObjectId]?.totalGroups
          ?? ((overviewGroupsByHost[hostObjectId]?.length ?? 0) + (truncatedGroupsByHost[hostObjectId] ?? 0))
        )
        || rankedGroupEdges.length;
      const hiddenGroupCount = Math.max(0, totalGroups - visibleGroupEdges.length);
      if (hiddenGroupCount > 0) {
        const hiddenProcessCount = rankedGroupEdges
          .filter((edge) => !visibleGroupEdgeIds.has(edge.id))
          .reduce((sum, edge) => {
            const node = nodesById.get(edge.target);
            return sum + Number(node?.properties.processCount ?? 0);
          }, 0);
        const overflowNodeId = `process-group-overflow:${hostNodeId}`;
        addNode({
          id: overflowNodeId,
          objectKind: 'ProcessGroup',
          objectId: summaryNode.objectId,
          layer: 'resource',
          label: groupPageLoadingByHost[hostObjectId]
            ? `loading ${hiddenGroupCount} groups...`
            : `+ ${hiddenGroupCount} groups`,
          properties: {
            collapsed: true,
            hiddenGroups: hiddenGroupCount,
            hiddenProcesses: hiddenProcessCount,
            displayLimit: PROCESS_GROUP_DISPLAY_LIMIT,
            fullGroupsLoading: Boolean(groupPageLoadingByHost[hostObjectId]),
            loadedGroups: loadedGroupsByHost[hostObjectId]?.groups.length ?? 0,
            totalGroups,
            pageSize: loadedGroupsByHost[hostObjectId]?.limit ?? PROCESS_GROUP_PAGE_SIZE,
            hasMore: loadedGroupsByHost[hostObjectId]?.hasMore ?? true,
          },
        });
        addEdge({
          id: `edge:${summaryNode.id}:${overflowNodeId}`,
          edgeKind: 'host_process_assoc',
          source: summaryNode.id,
          target: overflowNodeId,
          properties: {
            collapsed: true,
          },
        });
      }
    }
  }

  for (const processNodeId of visibleProcessNodeIds) {
    const bindingEdges = processServiceEdgesByProcess.get(processNodeId) ?? [];
    for (const bindingEdge of bindingEdges) {
      const instanceNode = nodesById.get(bindingEdge.target);
      addNode(instanceNode);
      addEdge(bindingEdge);

      const instanceToServiceEdges = serviceInstanceEdgesByInstance.get(bindingEdge.target) ?? [];
      for (const instanceEdge of instanceToServiceEdges) {
        const serviceNode = nodesById.get(instanceEdge.target);
        addNode(serviceNode);
        addEdge(instanceEdge);
        runtimeBoundServiceNodeIds.add(instanceEdge.target);
      }
    }
  }

  for (const hostNodeId of visibleHostNodeIds) {
    const serviceEdges = hostServiceEdgesByHost.get(hostNodeId) ?? [];
    for (const serviceEdge of serviceEdges) {
      if (runtimeBoundServiceNodeIds.has(serviceEdge.target)) {
        continue;
      }
      const serviceNode = nodesById.get(serviceEdge.target);
      addNode(serviceNode);
      addEdge(serviceEdge);
    }
  }

  return {
    nodes: displayNodes,
    edges: displayEdges,
    metadata: source.metadata,
  };
}

function getProcessViewMode(
  selectedNode: HostProcessTopologyNode | null,
  loadedGroupsByHost: Record<string, LoadedProcessGroupsState>,
  groupPageLoadingByHost: Record<string, boolean>,
): 'overview' | 'expanded' | 'loading' | null {
  if (!selectedNode || selectedNode.objectKind !== 'ProcessGroup') {
    return null;
  }
  const hostId = selectedNode.objectId;
  if (groupPageLoadingByHost[hostId]) return 'loading';
  if (loadedGroupsByHost[hostId]?.groups.length) return 'expanded';
  return 'overview';
}

function getSelectedLoadedProcessGroups(
  selectedNode: HostProcessTopologyNode | null,
  loadedGroupsByHost: Record<string, LoadedProcessGroupsState>,
): LoadedProcessGroupsState | null {
  if (!selectedNode || selectedNode.objectKind !== 'ProcessGroup') {
    return null;
  }
  return loadedGroupsByHost[selectedNode.objectId] ?? null;
}
