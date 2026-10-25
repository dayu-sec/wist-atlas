import { useEffect, useMemo, useRef } from 'react';
import cytoscape, { type Core } from 'cytoscape';
import dagre from 'cytoscape-dagre';
import coseBilkent from 'cytoscape-cose-bilkent';
import type { HostProcessTopologyGraph, LayoutName, NodeKind } from '../types/domain';
import { graphToCyElements } from '../utils/graph-to-cy';
import { NODE_SHAPES, getNodeStyle } from '../utils/colors';
import { basename } from '../utils/format';

cytoscape.use(dagre);
cytoscape.use(coseBilkent);

type CytoscapeNodeShape =
  | 'rectangle'
  | 'roundrectangle'
  | 'ellipse'
  | 'triangle'
  | 'pentagon'
  | 'hexagon'
  | 'heptagon'
  | 'octagon'
  | 'star'
  | 'barrel'
  | 'diamond'
  | 'vee'
  | 'rhomboid'
  | 'polygon'
  | 'tag'
  | 'round-rectangle'
  | 'round-triangle'
  | 'round-diamond'
  | 'round-pentagon'
  | 'round-hexagon'
  | 'round-heptagon'
  | 'round-octagon'
  | 'round-tag'
  | 'cut-rectangle'
  | 'bottom-round-rectangle'
  | 'concave-hexagon';

type Props = {
  graph: HostProcessTopologyGraph | null;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string | null) => void;
  searchQuery: string;
  layout: LayoutName;
};

const LAYOUTS: Record<ConcreteLayout, cytoscape.LayoutOptions> = {
  // LR 而不是 TB：主机的「网段」是 rank-1 的兄弟节点，横排能用上宽屏；
  // 之前 rankSep 太小，纵向排出来的标签会压到下一个节点上。
  dagre: {
    name: 'dagre',
    rankDir: 'LR',
    nodeSep: 56,
    rankSep: 130,
    edgeSep: 12,
    padding: 48,
    animate: false,
  } as cytoscape.LayoutOptions,
  concentric: {
    name: 'concentric',
    minNodeSpacing: 34,
    spacingFactor: 1.35,
    // 半径层级：主机在最内圈，进程汇总结其次，网段等叶子在最外圈。
    concentric: (node: cytoscape.NodeSingular) => {
      const kind = node.data('objectKind') as string;
      if (kind === 'HostInventory') return 3;
      if (kind === 'ProcessSummary') return 2;
      return 1;
    },
    levelWidth: () => 1,
    animate: false,
  } as cytoscape.LayoutOptions,
  'cose-bilkent': {
    name: 'cose-bilkent',
    idealEdgeLength: 140,
    nodeRepulsion: 6000,
    animate: false,
  } as cytoscape.LayoutOptions,
};

/** 「自动」模式下真正会落到的一个具体布局（星形图走自己算的 preset，不在表里）。 */
type ConcreteLayout = 'dagre' | 'concentric' | 'cose-bilkent';

type Position = { x: number; y: number };

/**
 * 星形资产图的半径。横向拉长：这个画布永远是宽屏，正圆会把两边留空。
 */
const STAR_LEAF_LIMIT = 14;
const STAR_RX = 340;
const STAR_RY = 250;

/**
 * 星形资产图（1 台主机 + 一圈直连叶子）的专用环形布局。
 *
 * 为什么不用 concentric：它把「独占一层」的节点固定从正北开始排，第一个叶子也从正北开始，
 * 于是进程汇总节点会跟某个网段共线 —— 看起来像那条边上的中转点。这里显式算角度，位置可控。
 */
function starPresetPositions(graph: HostProcessTopologyGraph): Record<string, Position> | null {
  const hosts = graph.nodes.filter((node) => node.objectKind === 'HostInventory');
  if (hosts.length !== 1) return null;

  const host = hosts[0];
  const leaves = graph.nodes.filter((node) => node.id !== host.id);
  if (leaves.length === 0 || leaves.length > STAR_LEAF_LIMIT) return null;

  const degree = new Map<string, number>();
  const attachedToHost = new Set<string>();
  for (const edge of graph.edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
    if (edge.source === host.id) attachedToHost.add(edge.target);
    if (edge.target === host.id) attachedToHost.add(edge.source);
  }

  // 有一个节点不是「直连主机且只有一条边」，就不是星形图，交给通用布局。
  const isStar = leaves.every(
    (node) => attachedToHost.has(node.id) && (degree.get(node.id) ?? 0) <= 1,
  );
  if (!isStar) return null;

  // 进程汇总排第一位 → 从正下方起铺，于是它独占正下方，不会和网段挤成一圈。
  const ordered = [...leaves].sort((a, b) => {
    const rankA = a.objectKind === 'ProcessSummary' ? 0 : 1;
    const rankB = b.objectKind === 'ProcessSummary' ? 0 : 1;
    return rankA - rankB;
  });

  const positions: Record<string, Position> = { [host.id]: { x: 0, y: 0 } };
  const step = (Math.PI * 2) / ordered.length;
  ordered.forEach((node, index) => {
    // cytoscape 的 y 轴向下，所以 sin(π/2)=1 就是屏幕下方。
    const angle = Math.PI / 2 + step * index;
    positions[node.id] = {
      x: Math.round(Math.cos(angle) * STAR_RX),
      y: Math.round(Math.sin(angle) * STAR_RY),
    };
  });

  return positions;
}

/** 每种节点的视觉尺寸。主机最大，进程最小 —— 尺寸本身要能读出层级。 */
const NODE_SIZE: Record<string, number> = {
  HostInventory: 76,
  ProcessSummary: 56,
  ProcessGroup: 40,
  NetworkSegment: 34,
  ServiceEntity: 34,
  ServiceInstance: 30,
  ProcessRuntime: 18,
  Subject: 28,
};

/** 节点主标签：用结构化字段重排，而不是照抄后端那句英文 `processes: 1120`。 */
function displayLabel(kind: NodeKind, label: string, properties: Record<string, unknown>): string {
  if (kind === 'ProcessSummary') {
    const total = Number(properties.totalProcesses ?? 0);
    return Number.isFinite(total) && total > 0 ? `进程 · ${total.toLocaleString('zh-CN')}` : '进程';
  }
  if (kind === 'ProcessGroup') {
    const executable = typeof properties.executable === 'string' ? properties.executable : '';
    const count = Number(properties.processCount ?? 0);
    const name = executable ? basename(executable) : label;
    return Number.isFinite(count) && count > 0 ? `${name} · ${count}` : name;
  }
  return label;
}

export default function TopologyCanvas({ graph, selectedNodeId, onSelectNode, searchQuery, layout }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);
  const onSelectNodeRef = useRef(onSelectNode);
  const fitRef = useRef<() => void>(() => {});

  useEffect(() => {
    onSelectNodeRef.current = onSelectNode;
  }, [onSelectNode]);

  useEffect(() => {
    if (!containerRef.current || cyRef.current) return;

    const cy = cytoscape({
      container: containerRef.current,
      selectionType: 'single',
      boxSelectionEnabled: false,
      wheelSensitivity: 0.25,
      minZoom: 0.15,
      maxZoom: 3,
      style: [
        {
          selector: 'node',
          style: {
            shape: (el: cytoscape.NodeSingular) =>
              (NODE_SHAPES[el.data('objectKind') as NodeKind] ?? 'ellipse') as CytoscapeNodeShape,
            width: (el: cytoscape.NodeSingular) => NODE_SIZE[el.data('objectKind') as string] ?? 24,
            height: (el: cytoscape.NodeSingular) => NODE_SIZE[el.data('objectKind') as string] ?? 24,
            'background-color': (el: cytoscape.NodeSingular) => getNodeStyle(el.data('objectKind') as NodeKind).fill,
            'border-color': (el: cytoscape.NodeSingular) => getNodeStyle(el.data('objectKind') as NodeKind).stroke,
            'border-width': (el: cytoscape.NodeSingular) => (el.data('objectKind') === 'HostInventory' ? 3 : 1.75),
            label: (el: cytoscape.NodeSingular) =>
              displayLabel(
                el.data('objectKind') as NodeKind,
                String(el.data('label') ?? ''),
                (el.data('properties') ?? {}) as Record<string, unknown>,
              ),
            'text-valign': 'bottom',
            'text-halign': 'center',
            'text-margin-y': 7,
            'font-size': (el: cytoscape.NodeSingular) => (el.data('objectKind') === 'HostInventory' ? 13 : 10),
            'font-weight': (el: cytoscape.NodeSingular) => (el.data('objectKind') === 'HostInventory' ? 600 : 400),
            color: (el: cytoscape.NodeSingular) => getNodeStyle(el.data('objectKind') as NodeKind).accent,
            'text-wrap': 'wrap',
            'text-max-width': (el: cytoscape.NodeSingular) => (el.data('objectKind') === 'HostInventory' ? '160px' : '110px'),
            // 关键：标签后面垫一块半透明白底，否则标签压在边上就成了那团糊字。
            'text-background-color': '#ffffff',
            'text-background-opacity': 0.88,
            'text-background-padding': '2px',
            'text-background-shape': 'roundrectangle',
            'overlay-opacity': 0,
          },
        },
        {
          selector: 'edge',
          style: {
            width: 1.25,
            'line-color': '#cbd5e1',
            'target-arrow-color': '#cbd5e1',
            'target-arrow-shape': 'none',
            'curve-style': 'bezier',
            'overlay-opacity': 0,
          },
        },
        {
          selector: 'edge[edgeKind = "host_process_assoc"]',
          style: {
            'line-color': '#b8c4d6',
            'line-style': 'solid',
            width: 1.5,
          },
        },
        {
          selector: 'edge[edgeKind = "responsibility_assignment"]',
          style: {
            'line-color': '#d8c7f5',
            'line-style': 'dashed',
          },
        },
        {
          selector: 'node:selected',
          style: {
            'border-width': 3.5,
            'border-color': '#4f46e5',
            'overlay-color': '#4f46e5',
            'overlay-opacity': 0.12,
            'overlay-padding': 6,
          },
        },
        {
          selector: '.dimmed',
          style: { opacity: 0.12 },
        },
        {
          selector: '.neighbor',
          style: { 'border-width': 2.5 },
        },
      ],
      layout: LAYOUTS.dagre,
    });

    cy.on('tap', (evt: cytoscape.EventObject) => {
      if (evt.target === cy) onSelectNodeRef.current(null);
    });
    cy.on('tap', 'node', (evt: cytoscape.EventObject) => {
      onSelectNodeRef.current(evt.target.id());
    });

    const fit = () => {
      const elements = cy.elements();
      if (elements.length === 0) return;
      cy.resize();
      cy.fit(elements, 64);
      // 小图放大有上限：节点被拉成色块比空白更难看。
      if (cy.zoom() > 1.6) {
        cy.zoom({ level: 1.6, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
        cy.center(elements);
      }
    };
    fitRef.current = fit;

    // 容器尺寸变化必须重新 fit —— 之前只在数据到达时 fit 一次，而那一刻容器
    // 往往还没有最终尺寸，于是图缩在中间一小团、四周全是空白。
    const observer = new ResizeObserver(() => fit());
    observer.observe(containerRef.current);

    cyRef.current = cy;

    return () => {
      observer.disconnect();
      cy.destroy();
      cyRef.current = null;
    };
  }, []);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !graph) return;
    cy.elements().remove();
    cy.add(graphToCyElements(graph));

    const effective = chooseEffectiveLayout(graph, layout);
    const instance =
      effective === 'star'
        ? cy.layout({
            name: 'preset',
            positions: starPresetPositions(graph) ?? {},
            animate: false,
            fit: false,
          } as cytoscape.LayoutOptions)
        : cy.layout(LAYOUTS[effective]);

    instance.one('layoutstop', () => fitRef.current());
    instance.run();
  }, [graph, layout]);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.nodes().removeClass('neighbor');
    if (!selectedNodeId) return;
    const node = cy.getElementById(selectedNodeId);
    if (node.length) node.neighborhood().addClass('neighbor');
  }, [selectedNodeId]);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !graph) return;

    const query = searchQuery.trim().toLowerCase();
    cy.nodes().removeClass('dimmed');
    if (!query) return;

    cy.nodes().forEach((node: cytoscape.NodeSingular) => {
      const label = String(node.data('label') ?? '');
      const properties = (node.data('properties') ?? {}) as Record<string, unknown>;
      const haystack = [label, ...Object.values(properties).map(String)].join(' ').toLowerCase();
      if (!haystack.includes(query)) node.addClass('dimmed');
    });
  }, [searchQuery, graph]);

  const empty = useMemo(() => !graph || graph.nodes.length === 0, [graph]);

  return (
    <div className="relative h-full w-full">
      <div ref={containerRef} className="h-full w-full" />

      {empty && (
        <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-2 text-center">
          <svg
            className="size-9 text-line-strong"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
          >
            <circle cx="12" cy="5" r="2.4" />
            <circle cx="5" cy="18" r="2.4" />
            <circle cx="19" cy="18" r="2.4" />
            <path d="M12 7.4v4.2M10.2 13.2 6.6 16M13.8 13.2l3.6 2.8" strokeLinecap="round" />
          </svg>
          <p className="text-[13px] font-medium text-ink-soft">还没有可展示的拓扑</p>
          <p className="max-w-xs text-[11px] text-ink-mute">
            先导入一份发现快照，或在左栏按对象 ID 查询。
          </p>
        </div>
      )}
    </div>
  );
}

/**
 * 「自动」下按图形状选布局；用户显式选了某个布局就照办，不静默替换。
 *
 * 之前默认就是 dagre，却对小图偷偷改成放射，而下拉框还写着「层次」——
 * 画出来的东西和标签对不上；现在「自动」是一个显式选项。
 */
function chooseEffectiveLayout(
  graph: HostProcessTopologyGraph,
  requestedLayout: LayoutName,
): ConcreteLayout | 'star' {
  if (requestedLayout !== 'auto') return requestedLayout;

  // 星形资产图是最常见的入口视图（一台主机 + 一圈网段），值得单独摆。
  if (starPresetPositions(graph)) return 'star';

  const count = (kind: NodeKind) => graph.nodes.filter((node) => node.objectKind === kind).length;
  const groups = count('ProcessGroup');
  const runtimes = count('ProcessRuntime');

  // 展开到「只有分组、没有具体进程」那一层时，dagre 会排成一条几百节点的长链，
  // 力导向才好读。这是原来就有的判断，保留。
  if (count('ProcessSummary') >= 1 && groups >= 16 && runtimes === 0) {
    return 'cose-bilkent';
  }

  return graph.nodes.length <= 48 ? 'concentric' : 'dagre';
}
