import type {
  ApiResponse,
  HostListDto,
  HostProcessGroupsPageDto,
  HostProcessOverviewDto,
  HostProcessTopologyGraph,
  HostTopologyDto,
} from '../types/domain';

const BASE = '/api';

async function get<T>(path: string, params?: Record<string, string>): Promise<ApiResponse<T>> {
  try {
    const url = new URL(`${BASE}${path}`, window.location.origin);
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        url.searchParams.set(k, v);
      }
    }
    const res = await fetch(url.toString());
    if (!res.ok) {
      return { status: 'error', code: String(res.status), message: res.statusText };
    }
    return res.json() as Promise<ApiResponse<T>>;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { status: 'error', code: 'fetch_failed', message };
  }
}

// V1 required
export async function fetchHostTopology(hostId: string): Promise<ApiResponse<HostTopologyDto>> {
  return get<HostTopologyDto>(`/topology/host/${hostId}`);
}

export async function fetchFirstHostTopology(): Promise<ApiResponse<HostTopologyDto>> {
  return get<HostTopologyDto>('/topology/host/first');
}

/** 主机列表：顶栏切换器的数据源。默认不含已退役主机。 */
export async function fetchHosts(): Promise<ApiResponse<HostListDto>> {
  return get<HostListDto>('/hosts');
}

// V1 optional enhancement
export async function fetchNetworkTopology(networkId: string): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>(`/topology/network/${networkId}`);
}

/** 服务焦点图：谁在监听、这套服务还跑在哪些主机上。 */
export async function fetchServiceTopology(serviceId: string): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>(`/topology/service/${serviceId}`);
}

// Dev-only mock (against Rust mock endpoint)
export async function fetchMockHostTopology(): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>('/__mock/topology/host');
}

// Static fixture loader (zero backend dependency)
export async function loadFixture(name: string): Promise<ApiResponse<HostProcessTopologyGraph>> {
  try {
    const res = await fetch(`/fixtures/${name}.json`);
    if (!res.ok) {
      return { status: 'error', code: String(res.status), message: res.statusText };
    }
    return res.json() as Promise<ApiResponse<HostProcessTopologyGraph>>;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { status: 'error', code: 'fetch_failed', message };
  }
}

export async function fetchHostProcessTopology(hostId: string): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>(`/topology/host/${hostId}/processes`);
}

export async function fetchFirstHostProcessTopology(): Promise<ApiResponse<HostProcessTopologyGraph>> {
  return get<HostProcessTopologyGraph>('/topology/host/first/processes');
}

export async function fetchHostProcessOverview(
  hostId: string,
  topN = 12,
): Promise<ApiResponse<HostProcessOverviewDto>> {
  return get<HostProcessOverviewDto>(`/topology/host/${hostId}/process-overview`, {
    top_n: String(topN),
  });
}

export async function fetchHostProcessGroupsPage(
  hostId: string,
  limit = 50,
  offset = 0,
): Promise<ApiResponse<HostProcessGroupsPageDto>> {
  return get<HostProcessGroupsPageDto>(`/topology/host/${hostId}/process-groups`, {
    limit: String(limit),
    offset: String(offset),
  });
}
