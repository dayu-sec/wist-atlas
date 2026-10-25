import type { HostProcessTopologyGraph, HostProcessTopologyNode } from '../types/domain';
import { nodeDims } from './presentation';

export type GraphPoint = { x: number; y: number };

export type LaneAnnotation = {
  id: string;
  x: number;
  y: number;
  /** 栏宽：给栏标题画一条与卡片同宽的细线，构图才有「分栏」而不是漂浮的标签。 */
  width: number;
  text: string;
  count: string;
};

export type FocusLayout = {
  positions: Record<string, GraphPoint>;
  annotations: LaneAnnotation[];
};

/**
 * 焦点版式的叶子数上限。
 *
 * 它现在是**防呆**用的（避免把几百个节点硬套进三栏），不再是版式质量的门槛 ——
 * 数量由右栏网格消化。服务展开后一台机器就是 30~40 个叶子，卡在 20 会让
 * 「点开查看全部」直接退回 dagre 的一列竖排。
 */
const FOCUS_LEAF_LIMIT = 64;
const LANE_GAP = 132;
/** 右栏竖向堆叠的上限。超过就换网格 —— 一列堆 28 张服务卡会把整图压到 14%。 */
const RIGHT_LANE_STACK_LIMIT = 4;
/** 网格每列最多多少行。14 行 ≈ 左栏网段列的高度，两列就能装下 28 个服务。 */
const RIGHT_LANE_MAX_ROWS = 14;
const CARD_GAP = 16;

/**
 * 「聚焦视图」布局：一台主机 + 一圈直连对象。
 *
 * 这是本产品最常见的入口。上一版把它摆成一个正圆环 —— 八个网段绕主机一圈，
 * 每条边长度一样、方向随机，读起来像曼陀罗而不像结构。这里改成一条横向剖面：
 *
 *     网络接口（左栏）  →  主机（中心）  →  运行时 / 服务（右栏）
 *
 * 左边是「这台机器接在哪些网段上」，右边是「这台机器里跑着什么」。
 * 信息有了阅读方向，宽屏也用上了；边只负责连接，不再负责构图。
 */
export function computeFocusLayout(graph: HostProcessTopologyGraph): FocusLayout | null {
  // 服务焦点图有自己的版式（服务居中，主机在左），先让给它。
  if (isServiceFocusGraph(graph)) return null;

  const hosts = graph.nodes.filter((node) => node.objectKind === 'HostInventory');
  if (hosts.length !== 1) return null;

  const host = hosts[0];
  const leaves = graph.nodes.filter((node) => node.id !== host.id);
  if (leaves.length === 0 || leaves.length > FOCUS_LEAF_LIMIT) return null;

  const degree = new Map<string, number>();
  const attachedToHost = new Set<string>();
  for (const edge of graph.edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
    if (edge.source === host.id) attachedToHost.add(edge.target);
    if (edge.target === host.id) attachedToHost.add(edge.source);
  }

  const containers = leaves
    .filter((node) => node.objectKind === 'ContainerRuntime')
    .sort((a, b) => a.label.localeCompare(b.label, 'en'));
  const left = leaves
    .filter((node) => node.objectKind === 'NetworkSegment')
    .sort((a, b) => a.label.localeCompare(b.label, 'en'));
  const right = leaves.filter(
    (node) => node.objectKind !== 'NetworkSegment' && node.objectKind !== 'ContainerRuntime',
  );

  // 「还是这颗星的内部结构」的判定：
  // 一层：直接挂在主机上（度数 ≤ 1）；容器、服务是有自己的叶子的，单独放行。
  // 两层以内：宿主 → 容器 / 服务 → 服务实例；三层：实例 → 监听进程。
  // 展开后的进程树（汇总 → 分组 → 进程）不在其中 —— 那种层数交给 dagre。
  const reachable = reachableWithinHostStar(graph, host.id);
  const isStar = leaves.every((node) => {
    if (attachedToHost.has(node.id) && (degree.get(node.id) ?? 0) <= 1) return true;
    if (
      attachedToHost.has(node.id) &&
      (node.objectKind === 'ContainerRuntime' || node.objectKind === 'ServiceEntity')
    ) {
      return true;
    }
    return reachable.has(node.id);
  });
  if (!isStar) return null;

  const hostDims = nodeDims(host.objectKind);
  const positions: Record<string, GraphPoint> = { [host.id]: { x: 0, y: 0 } };
  const annotations: LaneAnnotation[] = [];
  placeNetworkLane(left, hostDims, positions, annotations);

  // 容器车道夹在主机与「运行时/服务」之间：左边是它落在哪台机，右边是它里面跑什么。
  const containerLaneWidth =
    containers.length > 0 ? nodeDims('ContainerRuntime').width + LANE_GAP : 0;
  if (containers.length > 0) {
    const containerX = hostDims.width / 2 + LANE_GAP + nodeDims('ContainerRuntime').width / 2;
    placeContainerLane(containers, containerX, positions, annotations);
  }

  if (right.length > 0) {
    const summary = right.find((node) => node.objectKind === 'ProcessSummary');

    if (right.length <= RIGHT_LANE_STACK_LIMIT) {
      // 少量对象：竖向堆叠，运行时卡片在上、服务卡片在下。常见入口就长这样。
      const panelWidth = Math.max(...right.map((node) => nodeDims(node.objectKind).width));
      const x = hostDims.width / 2 + LANE_GAP + panelWidth / 2;
      // 右栏高度不一致（运行时卡片高、服务卡片矮），按行累加而不是等距排。
      const gap = CARD_GAP;
      const total = right.reduce((sum, node) => sum + nodeDims(node.objectKind).height, 0)
        + gap * Math.max(0, right.length - 1);
      let cursor = -total / 2;
      for (const node of right) {
        const dims = nodeDims(node.objectKind);
        positions[node.id] = { x, y: cursor + dims.height / 2 };
        cursor += dims.height + gap;
      }
      annotations.push({
        id: 'lane-runtime',
        x: x - panelWidth / 2,
        y: -total / 2 - 36,
        width: panelWidth,
        text: summary ? '运行时' : '关联对象',
        count: summary ? `${summaryCount(summary).toLocaleString('zh-CN')} 进程` : String(right.length),
      });
    } else {
      placeRightLaneGrid(hostDims, summary, right, positions, annotations);
    }
  }

  // 容器车道占了一段宽度，右栏整体往右让 —— 否则两组卡片会叠在一起。
  if (containerLaneWidth > 0) {
    const shifted = new Set([host.id, ...left.map((node) => node.id), ...containers.map((node) => node.id)]);
    for (const [id, point] of Object.entries(positions)) {
      if (shifted.has(id)) continue;
      point.x += containerLaneWidth;
    }
    for (const annotation of annotations) {
      if (annotation.id === 'lane-network' || annotation.id === 'lane-containers') continue;
      annotation.x += containerLaneWidth;
    }
  }

  centerComposition(positions, annotations, graph);
  return { positions, annotations };
}

/**
 * 容器车道。数量通常个位数，但 k8s 节点上几十个是常态 —— 与右栏服务同一套网格策略，
 * 否则一列堆 30 张卡会把整图压成一团。
 */
function placeContainerLane(
  containers: HostProcessTopologyNode[],
  x: number,
  positions: Record<string, GraphPoint>,
  annotations: LaneAnnotation[],
) {
  const dims = nodeDims('ContainerRuntime');
  const columns = Math.max(1, Math.ceil(containers.length / RIGHT_LANE_MAX_ROWS));
  const rows = Math.ceil(containers.length / columns);
  const stepX = dims.width + CARD_GAP;
  const stepY = dims.height + 14;
  const gridHeight = rows * stepY - 14;

  containers.forEach((node, index) => {
    const column = Math.floor(index / rows);
    const row = index % rows;
    positions[node.id] = {
      x: x + column * stepX,
      y: -gridHeight / 2 + dims.height / 2 + row * stepY,
    };
  });
  annotations.push({
    id: 'lane-containers',
    x: x - dims.width / 2,
    y: -gridHeight / 2 - 36,
    width: columns * stepX - CARD_GAP,
    text: '容器',
    count: String(containers.length),
  });
}

/**
 * 「服务焦点图」的唯一判据：这张图没有任何主机进程树（`host_process_assoc`），
 * 且恰好一个服务节点挂着实例。主机视图即使只展平成一个进程，也仍然有这条边。
 */
export function isServiceFocusGraph(graph: HostProcessTopologyGraph): boolean {
  if (graph.edges.some((edge) => edge.edgeKind === 'host_process_assoc')) return false;
  const services = graph.nodes.filter(
    (node) => node.objectKind === 'ServiceEntity' && node.properties.collapsed !== true,
  );
  if (services.length !== 1) return false;
  return graph.edges.some(
    (edge) => edge.edgeKind === 'service_instance_assoc' && edge.target === services[0].id,
  );
}

/** 从主机出发、最多三跳、且中间节点只能是 容器 / 服务 / 服务实例 的可达集合。 */
function reachableWithinHostStar(
  graph: HostProcessTopologyGraph,
  hostId: string,
): Set<string> {
  const intermediate = new Set(['ContainerRuntime', 'ServiceEntity', 'ServiceInstance']);
  const kindById = new Map(graph.nodes.map((node) => [node.id, node.objectKind]));
  const adjacency = new Map<string, string[]>();
  for (const edge of graph.edges) {
    adjacency.set(edge.source, [...(adjacency.get(edge.source) ?? []), edge.target]);
    adjacency.set(edge.target, [...(adjacency.get(edge.target) ?? []), edge.source]);
  }

  const reachable = new Set<string>([hostId]);
  let frontier = [hostId];
  for (let depth = 0; depth < 3 && frontier.length > 0; depth += 1) {
    const next: string[] = [];
    for (const id of frontier) {
      for (const neighbor of adjacency.get(id) ?? []) {
        if (reachable.has(neighbor)) continue;
        reachable.add(neighbor);
        next.push(neighbor);
      }
    }
    frontier = next.filter((id) => intermediate.has(kindById.get(id) ?? ''));
  }
  return reachable;
}

/**
 * 展开运行时之后的布局：主机 → 网段 / 运行时 → 进程分组。
 *
 * 默认的 dagre 会把 12 个分组竖着排成一列，整张图被迫缩到 39%，字全糊了。
 * 分组本来就是等宽的卡片，按两列网格铺开，宽度换高度，缩放能保住 0.8 以上。
 */
export function computeRuntimeLayout(graph: HostProcessTopologyGraph): FocusLayout | null {
  const hosts = graph.nodes.filter((node) => node.objectKind === 'HostInventory');
  if (hosts.length !== 1) return null;

  const summary = graph.nodes.find((node) => node.objectKind === 'ProcessSummary');
  if (!summary) return null;

  const segments = graph.nodes
    .filter((node) => node.objectKind === 'NetworkSegment')
    .sort((a, b) => a.label.localeCompare(b.label, 'en'));
  const groups = graph.nodes
    .filter((node) => node.objectKind === 'ProcessGroup')
    .sort((a, b) => Number(b.properties.processCount ?? 0) - Number(a.properties.processCount ?? 0));
  if (groups.length === 0) return null;

  // 出现进程明细/服务/容器时结构更深，交回 dagre，别硬摆。
  const allowed = new Set([
    'HostInventory',
    'NetworkSegment',
    'ProcessSummary',
    'ProcessGroup',
    'ContainerRuntime',
  ]);
  if (graph.nodes.some((node) => !allowed.has(node.objectKind))) return null;

  const host = hosts[0];
  const hostDims = nodeDims(host.objectKind);
  const summaryDims = nodeDims('ProcessSummary');
  const groupDims = nodeDims('ProcessGroup');
  const positions: Record<string, GraphPoint> = { [host.id]: { x: 0, y: 0 } };
  const annotations: LaneAnnotation[] = [];

  placeNetworkLane(segments, hostDims, positions, annotations);

  const summaryX = hostDims.width / 2 + 120 + summaryDims.width / 2;
  positions[summary.id] = { x: summaryX, y: 0 };

  const columns = Math.min(3, Math.max(1, Math.ceil(groups.length / 7)));
  const rows = Math.ceil(groups.length / columns);
  const columnGap = 16;
  const rowGap = 16;
  const gridWidth = columns * groupDims.width + (columns - 1) * columnGap;
  const gridHeight = rows * groupDims.height + (rows - 1) * rowGap;
  const gridLeft = summaryX + summaryDims.width / 2 + 96;
  const gridTop = -gridHeight / 2;

  groups.forEach((node, index) => {
    const column = Math.floor(index / rows);
    const row = index % rows;
    positions[node.id] = {
      x: gridLeft + column * (groupDims.width + columnGap) + groupDims.width / 2,
      y: gridTop + row * (groupDims.height + rowGap) + groupDims.height / 2,
    };
  });

  const totalGroups = Number(summary.properties.totalPrograms ?? groups.length);
  // 「+693 个分组」是伪节点，不能算进「已展示 12 组」里。
  const shownGroups = groups.filter(
    (node) => !node.id.startsWith('process-group-overflow:'),
  ).length;
  annotations.push({
    id: 'lane-groups',
    x: gridLeft,
    y: gridTop - 36,
    width: gridWidth,
    text: '进程分组',
    count: `${shownGroups} / ${totalGroups.toLocaleString('zh-CN')}`,
  });

  // 容器是主机级事实，与进程分组没有从属关系：放在分组网格之后单独一列。
  const containers = graph.nodes
    .filter((node) => node.objectKind === 'ContainerRuntime')
    .sort((a, b) => a.label.localeCompare(b.label, 'en'));
  if (containers.length > 0) {
    const containerX = gridLeft + gridWidth + 96 + nodeDims('ContainerRuntime').width / 2;
    placeContainerLane(containers, containerX, positions, annotations);
  }

  centerComposition(positions, annotations, graph);
  return { positions, annotations };
}

/**
 * 服务焦点版式：主机（左栏）← 服务（中）→ 实例（右栏）→ 监听进程（最右）。
 *
 * 这张图回答的是「谁在监听、这套服务还跑在哪些主机上」，所以服务必须居中：
 * 左边是它落到哪台机器，右边是实例与持有 socket 的进程。
 * 实例与它的进程按块对齐（而不是两栏各自竖向排），否则连线会全体交叉。
 */
export function computeServiceLayout(graph: HostProcessTopologyGraph): FocusLayout | null {
  // 判据只写一处：`isServiceFocusGraph`。
  if (!isServiceFocusGraph(graph)) return null;

  const service = graph.nodes.find(
    (node) => node.objectKind === 'ServiceEntity' && node.properties.collapsed !== true,
  );
  if (!service) return null;

  const hosts = graph.nodes.filter((node) => node.objectKind === 'HostInventory');
  const instances = graph.nodes.filter((node) => node.objectKind === 'ServiceInstance');
  if (hosts.length === 0 || instances.length === 0) return null;

  const processes = graph.nodes.filter((node) => node.objectKind === 'ProcessRuntime');
  if (hosts.length + instances.length + processes.length > FOCUS_LEAF_LIMIT) return null;

  const nodesById = new Map(graph.nodes.map((node) => [node.id, node]));
  const processesByInstance = new Map<string, HostProcessTopologyNode[]>();
  for (const edge of graph.edges) {
    if (edge.edgeKind !== 'process_service_assoc') continue;
    const process = nodesById.get(edge.source);
    if (!process) continue;
    const bucket = processesByInstance.get(edge.target) ?? [];
    bucket.push(process);
    processesByInstance.set(edge.target, bucket);
  }

  const serviceDims = nodeDims(service.objectKind);
  const positions: Record<string, GraphPoint> = { [service.id]: { x: 0, y: 0 } };
  const annotations: LaneAnnotation[] = [];

  // 左栏：主机。数量通常一两位，竖向堆叠足够。
  const hostDims = nodeDims('HostInventory');
  const hostX = -(serviceDims.width / 2 + LANE_GAP + hostDims.width / 2);
  const hostSpan = stackSpan(hosts.length, hostDims.height, 16);
  hosts.forEach((node, index) => {
    positions[node.id] = {
      x: hostX,
      y: -hostSpan / 2 + hostDims.height / 2 + index * (hostDims.height + 16),
    };
  });
  annotations.push({
    id: 'lane-service-hosts',
    x: hostX - hostDims.width / 2,
    y: -hostSpan / 2 - 36,
    width: hostDims.width,
    text: '运行主机',
    count: String(hosts.length),
  });

  // 右栏：实例竖向排，每个实例的监听进程紧贴在它右边。
  const instanceDims = nodeDims('ServiceInstance');
  const processDims = nodeDims('ProcessRuntime');
  const instanceX = serviceDims.width / 2 + LANE_GAP + instanceDims.width / 2;
  const processX =
    instanceX + instanceDims.width / 2 + LANE_GAP * 0.6 + processDims.width / 2;
  const blockGap = 24;
  const processGap = 10;

  const blocks = instances.map((instance) => {
    const bound = processesByInstance.get(instance.id) ?? [];
    const boundHeight = stackSpan(bound.length, processDims.height, processGap);
    return { instance, bound, height: Math.max(instanceDims.height, boundHeight) };
  });
  const totalHeight =
    blocks.reduce((sum, block) => sum + block.height, 0) +
    blockGap * Math.max(0, blocks.length - 1);

  let cursor = -totalHeight / 2;
  for (const block of blocks) {
    const center = cursor + block.height / 2;
    positions[block.instance.id] = { x: instanceX, y: center };
    const boundHeight = stackSpan(block.bound.length, processDims.height, processGap);
    let processCursor = center - boundHeight / 2;
    for (const process of block.bound) {
      positions[process.id] = { x: processX, y: processCursor + processDims.height / 2 };
      processCursor += processDims.height + processGap;
    }
    cursor += block.height + blockGap;
  }

  annotations.push({
    id: 'lane-service-instances',
    x: instanceX - instanceDims.width / 2,
    y: -totalHeight / 2 - 36,
    width: instanceDims.width,
    text: '服务实例',
    count: String(instances.length),
  });
  if (processes.length > 0) {
    annotations.push({
      id: 'lane-service-processes',
      x: processX - processDims.width / 2,
      y: -totalHeight / 2 - 36,
      width: processDims.width,
      text: '监听进程',
      count: String(processes.length),
    });
  }

  centerComposition(positions, annotations, graph);
  return { positions, annotations };
}

function placeNetworkLane(
  segments: HostProcessTopologyNode[],
  hostDims: { width: number; height: number },
  positions: Record<string, GraphPoint>,
  annotations: LaneAnnotation[],
) {
  if (segments.length === 0) return;
  const dims = nodeDims('NetworkSegment');
  const x = -(hostDims.width / 2 + LANE_GAP + dims.width / 2);
  const span = stackSpan(segments.length, dims.height, 14);
  segments.forEach((node, index) => {
    positions[node.id] = { x, y: -span / 2 + dims.height / 2 + index * (dims.height + 14) };
  });
  annotations.push({
    id: 'lane-network',
    x: x - dims.width / 2,
    y: -span / 2 - 36,
    width: dims.width,
    text: '网络接口',
    count: String(segments.length),
  });
}

function stackSpan(count: number, height: number, gap: number) {
  return count * height + Math.max(0, count - 1) * gap;
}

/**
 * 数量多时的右栏：运行时卡片独占一列，服务按网格铺开。
 *
 * 一列堆 N 张卡片的问题这个文件里已经写过一次 ——「12 个分组竖排一列，整张图被迫缩到
 * 39%」。服务接上真实端口后一台开发机就是 28 个，不换网格会把整图压到 14%。
 *
 * 行数上限取 14：这个高度与左栏的网段列大致齐平，于是 28 个服务刚好两列，
 * 宽高两个方向都不吃亏。
 */
function placeRightLaneGrid(
  hostDims: { width: number; height: number },
  summary: HostProcessTopologyNode | undefined,
  right: HostProcessTopologyNode[],
  positions: Record<string, GraphPoint>,
  annotations: LaneAnnotation[],
) {
  const laneLeft = hostDims.width / 2 + LANE_GAP;
  const others = right.filter((node) => node !== summary);

  const columns = Math.max(1, Math.ceil(others.length / RIGHT_LANE_MAX_ROWS));
  const rows = Math.max(1, Math.ceil(others.length / columns));
  const cardWidth = Math.max(0, ...others.map((node) => nodeDims(node.objectKind).width));
  const cardHeight = Math.max(0, ...others.map((node) => nodeDims(node.objectKind).height));
  const stepX = cardWidth + CARD_GAP;
  const stepY = cardHeight + CARD_GAP;
  const gridHeight = others.length === 0 ? 0 : rows * stepY - CARD_GAP;
  const summaryDims = summary ? nodeDims(summary.objectKind) : null;
  const laneHeight = Math.max(gridHeight, summaryDims?.height ?? 0);

  let cursorX = laneLeft;
  if (summary && summaryDims) {
    // 运行时卡片比服务卡高得多，混进网格会把行高撑乱；让它独占一列并垂直居中。
    positions[summary.id] = { x: cursorX + summaryDims.width / 2, y: 0 };
    cursorX += summaryDims.width + CARD_GAP * 2;
  }

  const gridTop = -gridHeight / 2;
  others.forEach((node, index) => {
    const column = Math.floor(index / rows);
    const row = index % rows;
    const dims = nodeDims(node.objectKind);
    positions[node.id] = {
      x: cursorX + column * stepX + dims.width / 2,
      y: gridTop + row * stepY + dims.height / 2,
    };
  });

  const laneWidth = cursorX + columns * stepX - CARD_GAP - laneLeft;
  annotations.push({
    id: 'lane-runtime',
    x: laneLeft,
    y: -laneHeight / 2 - 36,
    width: laneWidth,
    text: summary ? '运行时与服务' : '关联对象',
    count: summary ? `${summaryCount(summary).toLocaleString('zh-CN')} 进程` : String(others.length),
  });
}

function summaryCount(summary: HostProcessTopologyNode | undefined): number {
  if (!summary) return 0;
  const total = Number(summary.properties.totalProcesses ?? 0);
  return Number.isFinite(total) ? total : 0;
}

/** 把整组卡片的包围盒挪到 (0,0)，让 fit 之后构图在画布正中，而不是主机在正中。 */
function centerComposition(
  positions: Record<string, GraphPoint>,
  annotations: LaneAnnotation[],
  graph: HostProcessTopologyGraph,
) {
  const kindById = new Map(graph.nodes.map((node) => [node.id, node.objectKind]));
  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;

  for (const [id, point] of Object.entries(positions)) {
    const dims = nodeDims(kindById.get(id) ?? 'ProcessGroup');
    minX = Math.min(minX, point.x - dims.width / 2);
    maxX = Math.max(maxX, point.x + dims.width / 2);
    minY = Math.min(minY, point.y - dims.height / 2);
    maxY = Math.max(maxY, point.y + dims.height / 2);
  }

  const dx = -((minX + maxX) / 2);
  const dy = -((minY + maxY) / 2);
  if (!Number.isFinite(dx) || !Number.isFinite(dy)) return;

  // 主机跟着一起平移，构图整体居中后它自然仍在两条栏之间。
  for (const point of Object.values(positions)) {
    point.x += dx;
    point.y += dy;
  }
  // 栏标题不在卡片包围盒里，必须一起平移，否则会偏出半张卡片。
  for (const annotation of annotations) {
    annotation.x += dx;
    annotation.y += dy;
  }
}
