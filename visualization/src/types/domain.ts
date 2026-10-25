// ============================================================
// wist-atlas visualization — V1 domain types
// Aligned with topology-visualization-frontend-architecture.md
// V1 scope: host + network + responsibility only
// ============================================================

// ---- Layer classification (V1: only 2 layers) ----
export type LayerKind = 'resource' | 'governance';

// ---- Node kind (V1: only 3 object types) ----
export type NodeKind =
  | 'HostInventory'
  | 'NetworkSegment'
  | 'ProcessRuntime'
  | 'ProcessSummary'
  | 'ProcessGroup'
  | 'ServiceEntity'
  | 'ServiceInstance'
  | 'Subject';

// ---- Edge kind (V1: only 2 relation types) ----
export type EdgeKind =
  | 'host_network_assoc'
  | 'host_process_assoc'
  | 'host_service_assoc'
  | 'service_instance_assoc'
  | 'process_service_assoc'
  | 'responsibility_assignment';

// ---- Layer → NodeKind mapping ----
export const LAYER_NODE_KINDS: Record<LayerKind, readonly NodeKind[]> = {
  resource:   ['HostInventory', 'NetworkSegment', 'ProcessRuntime', 'ProcessSummary', 'ProcessGroup', 'ServiceEntity', 'ServiceInstance'],
  governance: ['Subject'],
};

// ---- Core DTOs ----
export type HostProcessTopologyNode = {
  id: string;                     // visual element ID (not necessarily the backend UUID)
  objectKind: NodeKind;           // backend object type
  objectId: string;               // backend object UUID
  layer: LayerKind;
  label: string;                  // display name
  properties: Record<string, unknown>;
};

export type HostProcessTopologyEdge = {
  id: string;
  edgeKind: EdgeKind;
  source: string;                 // node id
  target: string;                 // node id
  label?: string;
  properties?: Record<string, unknown>;
};

export type HostProcessTopologyGraph = {
  nodes: HostProcessTopologyNode[];
  edges: HostProcessTopologyEdge[];
  metadata?: {
    queryTime: string;
    tenantId?: string;
    focusObjectKind?: NodeKind;
    focusObjectId?: string;
    truncated?: boolean;
    /** 后端 graph 接口实际返回的计数（状态栏与左栏用） */
    hostCount?: number;
    processCount?: number;
  };
};

export type HostTopologyHost = {
  id: string;
  hostName: string;
  machineId?: string | null;
  osName?: string | null;
  osVersion?: string | null;
  /** CPU 架构（`aarch64` / `x86_64`）。M1 之后的后端才有，老数据为 null。 */
  arch?: string | null;
};

export type ProcessStateCountDto = {
  state: string;
  count: number;
};

export type HostProcessGroupDto = {
  executable: string;
  displayName: string;
  processCount: number;
  totalMemoryRssKiB: number;
  dominantState?: string | null;
  stateSummary: ProcessStateCountDto[];
};

export type HostTopologyServiceInstance = {
  instance: Record<string, unknown>;
  bindings: Record<string, unknown>[];
  processes: Record<string, unknown>[];
};

export type HostTopologyService = {
  service: Record<string, unknown>;
  instances: HostTopologyServiceInstance[];
};

export type HostTopologyDto = {
  host: HostTopologyHost;
  latestRuntime?: Record<string, unknown> | null;
  processGroups: Record<string, unknown>[];
  processes: Record<string, unknown>[];
  networkSegments: Record<string, unknown>[];
  networkAssocs: Record<string, unknown>[];
  services: HostTopologyService[];
  assignments: Record<string, unknown>[];
  generatedAt: string;
};

/** `GET /api/hosts` 的条目：只够「选主机」用的规模信息，不带进程明细。 */
export type HostSummaryDto = {
  id: string;
  hostName: string;
  machineId?: string | null;
  osName?: string | null;
  osVersion?: string | null;
  arch?: string | null;
  processCount: number;
  segmentCount: number;
  lastInventoryAt: string;
  stale: boolean;
  staleAt?: string | null;
};

export type HostListDto = {
  hosts: HostSummaryDto[];
  limit: number;
  offset: number;
};

export type ServiceSummaryDto = {
  id: string;
  name: string;
  serviceType: string;
  boundary: string;
  externalRef?: string | null;
  /** 仍有存活进程的实例数。 */
  instanceCount: number;
  /** 这些实例落在几台主机上。 */
  hostCount: number;
};

export type ServiceListDto = {
  services: ServiceSummaryDto[];
  limit: number;
  offset: number;
};

export type HostProcessOverviewDto = {
  host: HostTopologyHost;
  totalProcesses: number;
  totalGroups: number;
  topGroups: HostProcessGroupDto[];
  truncatedGroupCount: number;
  generatedAt: string;
};

export type HostProcessGroupsPageDto = {
  host: HostTopologyHost;
  totalProcesses: number;
  totalGroups: number;
  groups: HostProcessGroupDto[];
  limit: number;
  offset: number;
  hasMore: boolean;
  generatedAt: string;
};

export type LoadedProcessGroupsState = {
  groups: HostProcessGroupDto[];
  offset: number;
  limit: number;
  hasMore: boolean;
  totalGroups: number;
};

// ---- API response wrapper ----
export type ApiResponse<T> =
  | { status: 'ok'; data: T }
  | { status: 'error'; code: string; message: string };

// ---- App state (single graph view, no multi-view routing in V1) ----
export type AppState = {
  graph: HostProcessTopologyGraph | null;
  hostTopology: HostTopologyDto | null;
  loading: boolean;
  error: string | null;

  selectedNodeId: string | null;
  layerVisibility: Record<LayerKind, boolean>;
  searchQuery: string;
};

export type AppAction =
  | { type: 'FETCH_GRAPH_START' }
  | { type: 'FETCH_GRAPH_SUCCESS'; graph: HostProcessTopologyGraph }
  | { type: 'FETCH_HOST_TOPOLOGY_SUCCESS'; hostTopology: HostTopologyDto | null }
  | { type: 'FETCH_GRAPH_ERROR'; error: string }
  | { type: 'SELECT_NODE'; nodeId: string | null }
  | { type: 'TOGGLE_LAYER'; layer: LayerKind }
  | { type: 'SET_SEARCH'; query: string };

// ---- Cytoscape element data extensions ----
export type CyNodeData = {
  id: string;
  objectKind: NodeKind;
  layer: LayerKind;
  label: string;
  properties: Record<string, unknown>;
};

export type CyEdgeData = {
  id: string;
  source: string;
  target: string;
  edgeKind: EdgeKind;
  label: string;
  properties?: Record<string, unknown>;
};
