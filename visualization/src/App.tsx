import { useEffect, useState, useCallback } from 'react';
import { useAppState } from './hooks/useAppState';
import TopologyCanvas from './components/TopologyCanvas';
import FilterBar from './components/FilterBar';
import LayerLegend from './components/LayerLegend';
import NodeDetailPanel from './components/NodeDetailPanel';
import {
  fetchFirstHostTopology,
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

  useEffect(() => {
    const node = selectedNode;
    if (!node || node.objectKind !== 'HostInventory') {
      dispatch({ type: 'FETCH_HOST_TOPOLOGY_SUCCESS', hostTopology: null });
      return;
    }

    fetchHostTopology(node.objectId).then((res) => {
      if (res.status === 'ok') {
        dispatch({ type: 'FETCH_HOST_TOPOLOGY_SUCCESS', hostTopology: res.data });
      }
    });
  }, [selectedNode, dispatch]);

  useEffect(() => {
    if (USE_FIXTURE) return;
    if (state.hostTopology) return;
    if (selectedNode) return;

    const host = hostId.trim() || DEFAULT_HOST_ID;
    const loader = host ? fetchHostTopology(host) : fetchFirstHostTopology();
    loader.then((res) => {
      if (res.status === 'ok') {
        dispatch({ type: 'FETCH_HOST_TOPOLOGY_SUCCESS', hostTopology: res.data });
      }
    });
  }, [dispatch, hostId, selectedNode, state.hostTopology]);

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

  return (
    <div className="h-screen flex flex-col bg-gray-50">
      {/* Header */}
      <header className="flex items-center gap-4 px-4 py-2 bg-white border-b border-gray-200 shrink-0">
        <h1 className="text-sm font-semibold text-gray-700 tracking-wide shrink-0">
          wist-atlas
        </h1>

        {/* Host ID query input */}
        <form
          onSubmit={(e) => { e.preventDefault(); queryHost(); }}
          className="flex items-center gap-1"
        >
          <input
            type="text"
            placeholder="Host object ID..."
            value={hostId}
            onChange={(e) => setHostId(e.target.value)}
            className="px-2 py-1 text-xs border border-gray-300 rounded w-56 focus:outline-none focus:ring-2 focus:ring-blue-400"
          />
          <button
            type="submit"
            className="px-3 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600"
          >
            Query
          </button>
        </form>

        {/* Network ID query input */}
        <form
          onSubmit={(e) => { e.preventDefault(); queryNetwork(); }}
          className="flex items-center gap-1"
        >
          <input
            type="text"
            placeholder="Network object ID..."
            value={networkId}
            onChange={(e) => setNetworkId(e.target.value)}
            className="px-2 py-1 text-xs border border-gray-300 rounded w-56 focus:outline-none focus:ring-2 focus:ring-blue-400"
          />
          <button
            type="submit"
            className="px-3 py-1 text-xs bg-green-500 text-white rounded hover:bg-green-600"
          >
            Network
          </button>
        </form>

        <div className="flex-1" />

        <button
          onClick={loadData}
          disabled={state.loading}
          className="px-3 py-1 text-xs bg-gray-100 text-gray-600 rounded hover:bg-gray-200 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Refresh
        </button>

        {state.graph?.metadata?.queryTime && (
          <span className="text-xs text-gray-400">
            Updated: {state.graph.metadata.queryTime}
          </span>
        )}
      </header>

      {/* Main area */}
      <div className="flex-1 relative overflow-hidden">
        <FilterBar
          searchQuery={state.searchQuery}
          onSearchChange={(q) => dispatch({ type: 'SET_SEARCH', query: q })}
          layerVisibility={state.layerVisibility}
          onToggleLayer={(l) => dispatch({ type: 'TOGGLE_LAYER', layer: l })}
          layout={state.layout}
          onLayoutChange={(l) => dispatch({ type: 'SET_LAYOUT', layout: l })}
        />
        <div className="relative" style={{ height: 'calc(100% - 41px)' }}>
          <TopologyCanvas
            graph={displayGraph}
            selectedNodeId={state.selectedNodeId}
            onSelectNode={handleSelectNode}
            searchQuery={state.searchQuery}
            layout={state.layout}
          />
          <LayerLegend />
          <NodeDetailPanel
            node={selectedNode}
            hostTopology={state.hostTopology}
            processViewMode={getProcessViewMode(selectedNode, loadedGroupsByHost, groupPageLoadingByHost)}
            loadedProcessGroups={getSelectedLoadedProcessGroups(selectedNode, loadedGroupsByHost)}
            onClose={() => dispatch({ type: 'SELECT_NODE', nodeId: null })}
          />
        </div>
      </div>

      {/* Loading bar */}
      <div
        className={`h-0.5 bg-blue-500 transition-all duration-300 ease-out ${state.loading ? 'opacity-100' : 'opacity-0'}`}
        style={{ width: state.loading ? '100%' : '0%' }}
      />

      {/* Error banner */}
      {state.error && (
        <div className="absolute bottom-4 right-4 bg-red-50 border border-red-200 rounded-lg px-4 py-2 text-xs text-red-700 z-20 max-w-md shadow">
          <span className="font-medium">Error</span>
          <span className="ml-1">{state.error}</span>
          <button
            onClick={() => dispatch({ type: 'FETCH_GRAPH_ERROR', error: '' })}
            className="ml-2 text-red-400 hover:text-red-600 font-bold"
          >
            &times;
          </button>
        </div>
      )}
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
