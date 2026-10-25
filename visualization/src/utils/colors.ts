import type { LayerKind, NodeKind } from '../types/domain';
import { LAYER_NODE_KINDS } from '../types/domain';

export type NodeStyle = {
  /** 节点填充 */
  fill: string;
  /** 节点描边 */
  stroke: string;
  /** 该类型的角标色（图例、徽章、详情面板标题） */
  accent: string;
  /** 中文名 */
  label: string;
};

/**
 * 按 **objectKind** 着色，不是按 layer。
 *
 * 之前只有 layer 两级色（绿=资源、紫=治理），而图上同时有主机/网段/进程汇总/分组/进程五种
 * 节点 —— 图例解释不了图上看到的东西。现在一种对象一个色，图例只列「当前这张图里出现过」的。
 */
export const NODE_STYLES: Record<NodeKind, NodeStyle> = {
  HostInventory: { fill: '#fef3c7', stroke: '#f59e0b', accent: '#b45309', label: '主机' },
  NetworkSegment: { fill: '#d1fae5', stroke: '#10b981', accent: '#047857', label: '网段' },
  ProcessSummary: { fill: '#dbeafe', stroke: '#3b82f6', accent: '#1d4ed8', label: '进程汇总' },
  ProcessGroup: { fill: '#e0f2fe', stroke: '#0ea5e9', accent: '#0369a1', label: '进程分组' },
  ProcessRuntime: { fill: '#f1f5f9', stroke: '#94a3b8', accent: '#475569', label: '进程' },
  ServiceEntity: { fill: '#ede9fe', stroke: '#8b5cf6', accent: '#6d28d9', label: '服务' },
  ServiceInstance: { fill: '#f3e8ff', stroke: '#a855f7', accent: '#7e22ce', label: '服务实例' },
  Subject: { fill: '#fce7f3', stroke: '#ec4899', accent: '#be185d', label: '责任人' },
};

export const NODE_SHAPES: Record<NodeKind, string> = {
  HostInventory: 'round-diamond',
  NetworkSegment: 'round-hexagon',
  ProcessSummary: 'round-rectangle',
  ProcessGroup: 'round-rectangle',
  ProcessRuntime: 'ellipse',
  ServiceEntity: 'round-tag',
  ServiceInstance: 'round-octagon',
  Subject: 'ellipse',
};

export const LAYER_COLORS: Record<LayerKind, { bg: string; border: string; label: string }> = {
  resource: { bg: '#ecfdf5', border: '#10b981', label: '资源目录' },
  governance: { bg: '#f5f3ff', border: '#8b5cf6', label: '责任治理' },
};

export function nodeKindToLayer(kind: NodeKind): LayerKind {
  for (const [layer, kinds] of Object.entries(LAYER_NODE_KINDS)) {
    if ((kinds as readonly NodeKind[]).includes(kind)) return layer as LayerKind;
  }
  return 'resource';
}

export function getNodeStyle(kind: NodeKind): NodeStyle {
  return NODE_STYLES[kind] ?? NODE_STYLES.ProcessRuntime;
}

/** 兼容旧调用：详情面板标题用 layer 色。 */
export function getLayerStyle(kind: NodeKind) {
  const style = getNodeStyle(kind);
  return { bg: style.fill, border: style.stroke, label: style.label };
}
