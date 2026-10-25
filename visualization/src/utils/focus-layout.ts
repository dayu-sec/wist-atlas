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

const FOCUS_LEAF_LIMIT = 20;
const LANE_GAP = 132;

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

  const isStar = leaves.every(
    (node) => attachedToHost.has(node.id) && (degree.get(node.id) ?? 0) <= 1,
  );
  if (!isStar) return null;

  const left = leaves
    .filter((node) => node.objectKind === 'NetworkSegment')
    .sort((a, b) => a.label.localeCompare(b.label, 'en'));
  const right = leaves.filter((node) => node.objectKind !== 'NetworkSegment');

  const hostDims = nodeDims(host.objectKind);
  const positions: Record<string, GraphPoint> = { [host.id]: { x: 0, y: 0 } };
  const annotations: LaneAnnotation[] = [];
  placeNetworkLane(left, hostDims, positions, annotations);

  if (right.length > 0) {
    const panelWidth = Math.max(...right.map((node) => nodeDims(node.objectKind).width));
    const x = hostDims.width / 2 + LANE_GAP + panelWidth / 2;
    // 右栏高度不一致（运行时卡片高、服务卡片矮），按行累加而不是等距排。
    const gap = 16;
    const total = right.reduce((sum, node) => sum + nodeDims(node.objectKind).height, 0)
      + gap * Math.max(0, right.length - 1);
    let cursor = -total / 2;
    for (const node of right) {
      const dims = nodeDims(node.objectKind);
      positions[node.id] = { x, y: cursor + dims.height / 2 };
      cursor += dims.height + gap;
    }
    const summary = right.find((node) => node.objectKind === 'ProcessSummary');
    annotations.push({
      id: 'lane-runtime',
      x: x - panelWidth / 2,
      y: -total / 2 - 36,
      width: panelWidth,
      text: summary ? '运行时' : '关联对象',
      count: summary ? `${summaryCount(summary).toLocaleString('zh-CN')} 进程` : String(right.length),
    });
  }

  centerComposition(positions, annotations, graph);
  return { positions, annotations };
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

  // 出现进程明细/服务/责任人时结构更深，交回 dagre，别硬摆。
  const allowed = new Set(['HostInventory', 'NetworkSegment', 'ProcessSummary', 'ProcessGroup']);
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
