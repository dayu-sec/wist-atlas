import { useEffect, useState, useCallback, useMemo } from 'react';
import { useAppState } from './hooks/useAppState';
import AppHeader from './components/AppHeader';
import TopologyCanvas from './components/TopologyCanvas';
import CanvasToolbar from './components/CanvasToolbar';
import LegendCard from './components/LegendCard';
import HostSummaryCard from './components/HostSummaryCard';
import StatusBar from './components/StatusBar';
import NodeDetailPanel from './components/NodeDetailPanel';
import { ChevronRight } from './components/icons';
import { buildSearchIndex, countMatches } from './utils/search';
import {
  fetchHostProcessGroupsPage,
  fetchHostProcessOverview,
  fetchFirstHostProcessTopology,
  fetchHostProcessTopology,
  fetchHosts,
  fetchHostTopology,
  fetchNetworkTopology,
  loadFixture,
} from './api/client';
import type {
  HostProcessGroupDto,
  HostSummaryDto,
  LoadedProcessGroupsState,
  HostProcessTopologyEdge,
  HostProcessTopologyGraph,
  HostProcessTopologyNode,
  LayerKind,
  NodeKind,
} from './types/domain';

const PROCESS_SUMMARY_PREFIX = 'process-summary:';
const PROCESS_GROUP_PREFIX = 'process-group:';
const PROCESS_GROUP_OVERFLOW_PREFIX = 'process-group-overflow:';
/** 服务也走 TopN + 溢出节点：一台机器 28 个监听服务时，全铺在画布上没人看得清。 */
const SERVICE_OVERFLOW_PREFIX = 'service-overflow:';
const SERVICE_DISPLAY_LIMIT = 6;
const PROCESS_EXPAND_LIMIT = 20;
const PROCESS_GROUP_DISPLAY_LIMIT = 12;
const PROCESS_GROUP_PAGE_SIZE = 20;
const DEFAULT_HOST_ID = import.meta.env.VITE_DEFAULT_HOST_ID as string | undefined;
const USE_FIXTURE = import.meta.env.VITE_USE_FIXTURE === '1';

export default function App() {
  const [state, dispatch] = useAppState();
  const [hostId, setHostId] = useState('');
  const [hostQueryId, setHostQueryId] = useState('');
  const [hosts, setHosts] = useState<HostSummaryDto[]>([]);
  const [networkId, setNetworkId] = useState('');
  const [expandedHosts, setExpandedHosts] = useState<Set<string>>(new Set());
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [expandedServices, setExpandedServices] = useState<Set<string>>(new Set());
  const [overviewGroupsByHost, setOverviewGroupsByHost] = useState<Record<string, HostProcessGroupDto[]>>({});
  const [truncatedGroupsByHost, setTruncatedGroupsByHost] = useState<Record<string, number>>({});
  const [loadedGroupsByHost, setLoadedGroupsByHost] = useState<Record<string, LoadedProcessGroupsState>>({});
  const [groupPageLoadingByHost, setGroupPageLoadingByHost] = useState<Record<string, boolean>>({});

  // 主机列表只用于顶栏切换器。
  // 多于一台时默认聚焦第一台：左栏的资产卡、画布、地址栏才说的是同一台机器；
  // 想看全局就用下拉里的「全部主机（总览）」（`hostId = ''`）。
  useEffect(() => {
    if (USE_FIXTURE) return;
    let cancelled = false;
    fetchHosts().then((res) => {
      if (cancelled || res.status !== 'ok') return;
      setHosts(res.data.hosts);
      if (!DEFAULT_HOST_ID) {
        setHostId((current) => current || res.data.hosts[0]?.id || '');
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

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
    const trimmed = hostQueryId.trim();
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
  }, [hostQueryId, dispatch]);

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

  // 关键：选中节点/输入搜索词都会让 App 重渲染，如果把 displayGraph 每次重建，
  // 画布的「图结构签名」就每次都变，点一下节点就会重新布局。
  const displayGraph = useMemo(
    () =>
      buildDisplayGraph(
        state.graph,
        expandedHosts,
        expandedGroups,
        expandedServices,
        overviewGroupsByHost,
        truncatedGroupsByHost,
        loadedGroupsByHost,
        groupPageLoadingByHost,
      ),
    [
      state.graph,
      expandedHosts,
      expandedGroups,
      expandedServices,
      overviewGroupsByHost,
      truncatedGroupsByHost,
      loadedGroupsByHost,
      groupPageLoadingByHost,
    ],
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

  /**
   * 运行时卡片有两种意图：点卡片是「看详情」，点卡片底部的「展开进程分组」才是「铺开」。
   * 之前一次点击同时做两件事，详情抽屉正好盖住刚展开的分组。
   */
  const handleExpandNode = useCallback((nodeId: string) => {
    const hostNodeId = nodeId.startsWith(PROCESS_SUMMARY_PREFIX)
      ? nodeId.slice(PROCESS_SUMMARY_PREFIX.length)
      : nodeId;
    setExpandedHosts((current) => {
      const next = new Set(current);
      if (next.has(hostNodeId)) next.delete(hostNodeId);
      else next.add(hostNodeId);
      return next;
    });
    dispatch({ type: 'SELECT_NODE', nodeId: null });
  }, [dispatch]);

  const handleSelectNode = useCallback((nodeId: string | null) => {
    if (nodeId && nodeId.startsWith(PROCESS_SUMMARY_PREFIX)) {
      dispatch({ type: 'SELECT_NODE', nodeId });
      return;
    }
    if (nodeId && nodeId.startsWith(SERVICE_OVERFLOW_PREFIX)) {
      // 服务数据已全量在内存里，展开/收起都是本地行为，不用取接口。
      const hostNodeId = nodeId.slice(SERVICE_OVERFLOW_PREFIX.length);
      setExpandedServices((current) => {
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

  const graphNodes = useMemo(() => displayGraph?.nodes ?? [], [displayGraph]);
  const graphEdges = useMemo(() => displayGraph?.edges ?? [], [displayGraph]);
  const presentLayers = useMemo(() => {
    const layers: LayerKind[] = [];
    for (const node of graphNodes) {
      if (!layers.includes(node.layer)) layers.push(node.layer);
    }
    return layers;
  }, [graphNodes]);

  const presentKinds = useMemo(() => {
    const kinds: NodeKind[] = [];
    for (const node of graphNodes) {
      if (!kinds.includes(node.objectKind)) kinds.push(node.objectKind);
    }
    return kinds;
  }, [graphNodes]);

  const topGroups = useMemo(() => {
    return (state.hostTopology?.processGroups ?? []).slice(0, 5).map((group) => ({
      name: String(group.display_name ?? group.executable ?? '-'),
      count: Number(group.process_count ?? 0),
    }));
  }, [state.hostTopology]);

  // Esc 收起详情面板：图上的选中态和右侧抽屉必须能一键退出来。
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') dispatch({ type: 'SELECT_NODE', nodeId: null });
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [dispatch]);

  const segmentCount = state.hostTopology?.networkSegments.length
    ?? graphNodes.filter((node) => node.objectKind === 'NetworkSegment').length;
  const processCount = state.hostTopology?.processes.length
    ?? Number(displayGraph?.metadata?.processCount ?? 0);
  const groupCount = state.hostTopology?.processGroups.length
    ?? graphNodes.filter((node) => node.objectKind === 'ProcessGroup').length;
  const serviceCount = state.hostTopology?.services.length
    ?? graphNodes.filter((node) => node.objectKind === 'ServiceEntity').length;
  const hostCount = Number(displayGraph?.metadata?.hostCount ?? (primaryHostNode ? 1 : 0));

  const searchIndex = useMemo(() => buildSearchIndex(displayGraph), [displayGraph]);
  const matchCount = state.searchQuery.trim()
    ? countMatches(searchIndex, state.searchQuery)
    : null;

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-canvas">
      <AppHeader
        host={railHost}
        hostName={railHost?.hostName ?? '未选择主机'}
        hosts={hosts}
        selectedHostId={hostId}
        onSelectHost={(nextHostId) => setHostId(nextHostId)}
        scannedAt={state.hostTopology?.generatedAt ?? displayGraph?.metadata?.queryTime ?? null}
        loading={state.loading}
        onRefresh={loadData}
      />

      <div className="flex min-h-0 flex-1">
        <aside className="flex w-[304px] shrink-0 flex-col overflow-y-auto border-r border-line bg-surface">
          <HostSummaryCard
            host={railHost}
            segmentCount={segmentCount}
            processCount={processCount}
            groupCount={groupCount}
            serviceCount={serviceCount}
            topGroups={topGroups}
            generatedAt={state.hostTopology?.generatedAt ?? displayGraph?.metadata?.queryTime ?? null}
            loading={state.loading}
          />

          <LegendCard kinds={presentKinds} />

          <details className="group border-b border-line px-4 py-3.5">
            <summary className="flex cursor-pointer list-none items-center gap-1 text-[10.5px] text-[#98a2b3] transition-colors hover:text-[#475467]">
              <ChevronRight className="size-3 transition-transform group-open:rotate-90" />
              按对象 ID 查询
            </summary>

            <div className="mt-3 space-y-3">
              <form
                onSubmit={(event) => {
                  event.preventDefault();
                  queryHost();
                }}
                className="space-y-1.5"
              >
                <label className="block text-[9.5px] text-[#98a2b3]" htmlFor="host-id-input">
                  主机对象 ID
                </label>
                <div className="flex gap-1.5">
                  <input
                    id="host-id-input"
                    type="text"
                    placeholder="UUID"
                    value={hostQueryId}
                    onChange={(e) => setHostQueryId(e.target.value)}
                    className="h-7 min-w-0 flex-1 rounded-[7px] border border-line bg-[#f9fafb] px-2 font-mono text-[10.5px] text-ink placeholder:text-[#b4bcc8] focus:border-brand focus:bg-surface focus:outline-none focus:ring-2 focus:ring-brand/15"
                  />
                  <button
                    type="submit"
                    className="h-7 shrink-0 rounded-[7px] bg-ink px-2.5 text-[10.5px] font-medium text-white transition-colors hover:bg-[#344054]"
                  >
                    查询
                  </button>
                </div>
              </form>

              <form
                onSubmit={(event) => {
                  event.preventDefault();
                  queryNetwork();
                }}
                className="space-y-1.5"
              >
                <label className="block text-[9.5px] text-[#98a2b3]" htmlFor="network-id-input">
                  网段对象 ID
                </label>
                <div className="flex gap-1.5">
                  <input
                    id="network-id-input"
                    type="text"
                    placeholder="UUID"
                    value={networkId}
                    onChange={(event) => setNetworkId(event.target.value)}
                    className="h-7 min-w-0 flex-1 rounded-[7px] border border-line bg-[#f9fafb] px-2 font-mono text-[10.5px] text-ink placeholder:text-[#b4bcc8] focus:border-brand focus:bg-surface focus:outline-none focus:ring-2 focus:ring-brand/15"
                  />
                  <button
                    type="submit"
                    className="h-7 shrink-0 rounded-[7px] border border-line bg-surface px-2.5 text-[10.5px] font-medium text-[#475467] transition-colors hover:border-[#d0d5dd] hover:bg-[#f9fafb]"
                  >
                    查询
                  </button>
                </div>
              </form>
            </div>
          </details>

          <details className="group px-4 py-3.5">
            <summary className="flex cursor-pointer list-none items-center gap-1 text-[10.5px] text-[#98a2b3] transition-colors hover:text-[#475467]">
              <ChevronRight className="size-3 transition-transform group-open:rotate-90" />
              对象标识
            </summary>
            <div className="mt-2.5 space-y-2">
              <IdRow label="主机 ID" value={railHost?.id} />
              <IdRow label="机器 ID" value={railHost?.machineId} />
            </div>
          </details>

          <div className="mt-auto px-4 py-4 text-[10px] leading-4 text-[#b4bcc8]">
            只读视图 · 数据来自最近一次主机盘点
          </div>
        </aside>

        <main className="relative min-w-0 flex-1">
          <TopologyCanvas
            graph={displayGraph}
            selectedNodeId={state.selectedNodeId}
            onSelectNode={handleSelectNode}
            onExpandNode={handleExpandNode}
            searchQuery={state.searchQuery}
            layerVisibility={state.layerVisibility}
            scannedAt={state.hostTopology?.generatedAt ?? displayGraph?.metadata?.queryTime ?? null}
            hostArch={state.hostTopology?.host.arch ?? null}
            loading={state.loading}
          />

          <div className="pointer-events-none absolute left-4 top-4 z-20">
            <CanvasToolbar
              searchQuery={state.searchQuery}
              onSearchChange={(query) => dispatch({ type: 'SET_SEARCH', query })}
              layerVisibility={state.layerVisibility}
              layers={presentLayers}
              onToggleLayer={(layer) => dispatch({ type: 'TOGGLE_LAYER', layer })}
              matchCount={matchCount}
            />
          </div>

          <NodeDetailPanel
            node={selectedNode}
            graph={displayGraph}
            hostTopology={state.hostTopology}
            processViewMode={getProcessViewMode(selectedNode, loadedGroupsByHost, groupPageLoadingByHost)}
            loadedProcessGroups={getSelectedLoadedProcessGroups(selectedNode, loadedGroupsByHost)}
            onClose={() => dispatch({ type: 'SELECT_NODE', nodeId: null })}
            onExpand={handleExpandNode}
          />

          {state.error && (
            <div className="absolute bottom-4 left-4 z-30 flex max-w-md items-start gap-2 rounded-[10px] border border-[#fecdca] bg-[#fffbfa] px-3 py-2 text-[11px] text-[#b42318] shadow-[0_8px_24px_-12px_rgba(16,24,40,0.25)]">
              <span className="mt-px shrink-0 font-semibold">出错</span>
              <span className="min-w-0 break-all">{state.error}</span>
              <button
                type="button"
                onClick={() => dispatch({ type: 'FETCH_GRAPH_ERROR', error: '' })}
                className="ml-auto shrink-0 rounded px-1 text-[#f97066] hover:text-[#b42318]"
                aria-label="关闭错误提示"
              >
                ✕
              </button>
            </div>
          )}
        </main>
      </div>

      <StatusBar
        hostCount={hostCount}
        segmentCount={segmentCount}
        processCount={processCount}
        groupCount={groupCount}
        nodeCount={graphNodes.length}
        edgeCount={graphEdges.length}
        queryTime={displayGraph?.metadata?.queryTime ?? null}
        loading={state.loading}
      />
    </div>
  );
}

function IdRow({ label, value }: { label: string; value?: string | null }) {
  if (!value) return null;
  return (
    <div className="text-[10px]">
      <div className="text-[#98a2b3]">{label}</div>
      <div className="mt-0.5 truncate font-mono text-[9.5px] leading-4 text-[#667085]" title={value}>
        {value}
      </div>
    </div>
  );
}

function buildDisplayGraph(
  source: HostProcessTopologyGraph | null,
  expandedHosts: Set<string>,
  expandedGroups: Set<string>,
  expandedServices: Set<string>,
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
    const hostNode = nodesById.get(hostNodeId);
    const serviceEdges = (hostServiceEdgesByHost.get(hostNodeId) ?? []).filter(
      (edge) => !runtimeBoundServiceNodeIds.has(edge.target),
    );
    if (serviceEdges.length === 0) continue;

    // 画布是概览，不是清单：只放最值得看的几个，剩下的收进溢出节点。
    // 排序按「对外暴露的端点数」优先（0.0.0.0 / ::）—— 监听在回环上的服务再重要，
    // 也不如一个对外开的端口值得占画布。
    const ranked = [...serviceEdges].sort((left, right) => {
      const leftRank = serviceExposureRank(nodesById.get(left.target));
      const rightRank = serviceExposureRank(nodesById.get(right.target));
      if (rightRank.external !== leftRank.external) {
        return rightRank.external - leftRank.external;
      }
      if (rightRank.total !== leftRank.total) {
        return rightRank.total - leftRank.total;
      }
      return (nodesById.get(left.target)?.label ?? '').localeCompare(
        nodesById.get(right.target)?.label ?? '',
        'en',
      );
    });

    const expanded = expandedServices.has(hostNodeId);
    const visible = expanded ? ranked : ranked.slice(0, SERVICE_DISPLAY_LIMIT);
    for (const serviceEdge of visible) {
      addNode(nodesById.get(serviceEdge.target));
      addEdge(serviceEdge);
    }

    const hiddenCount = ranked.length - visible.length;
    const needsOverflow = hiddenCount > 0 || (expanded && ranked.length > SERVICE_DISPLAY_LIMIT);
    if (needsOverflow) {
      const overflowNodeId = `${SERVICE_OVERFLOW_PREFIX}${hostNodeId}`;
      const label = hiddenCount > 0 ? `+ ${hiddenCount} 个服务` : '收起列表';
      addNode({
        id: overflowNodeId,
        objectKind: 'ServiceEntity',
        objectId: hostNode?.objectId ?? '',
        layer: 'resource',
        label,
        properties: {
          collapsed: true,
          name: label,
          hiddenServices: hiddenCount,
          totalServices: ranked.length,
          displayLimit: SERVICE_DISPLAY_LIMIT,
        },
      });
      addEdge({
        id: `edge:${hostNodeId}:${overflowNodeId}`,
        edgeKind: 'host_service_assoc',
        source: hostNodeId,
        target: overflowNodeId,
        properties: { collapsed: true },
      });
    }
  }

  return {
    nodes: displayNodes,
    edges: displayEdges,
    metadata: source.metadata,
  };
}

/**
 * 服务的「暴露程度」：对外（`0.0.0.0` / `::`）的端口数优先，其次总数。
 * 只按端点数排会把一堆回环监听顶上来 —— 那恰恰是最不值得先看的。
 */
function serviceExposureRank(node: HostProcessTopologyNode | undefined): {
  external: number;
  total: number;
} {
  const raw = node?.properties.endpoints;
  if (!Array.isArray(raw)) return { external: 0, total: 0 };

  let external = 0;
  for (const item of raw) {
    if (typeof item !== 'object' || item === null) continue;
    const address = String((item as Record<string, unknown>).address ?? '');
    if (address === '0.0.0.0' || address === '::') external += 1;
  }
  return { external, total: raw.length };
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
