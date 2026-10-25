import type { LayerKind, NodeKind } from '../types/domain';
import { LAYER_NODE_KINDS } from '../types/domain';

export type NodeStyle = {
  /** 图标块底色 */
  fill: string;
  /** 图标与描边色 */
  stroke: string;
  /** 文字/角标色 */
  accent: string;
  /** 中文名 */
  label: string;
};

/**
 * 颜色只承担「分类」，不承担「装饰」。
 *
 * 上一版的节点是满色块（薄荷绿 / 婴儿蓝 / 琥珀），十来个节点就把画布变成糖果盘；
 * 现在大面积留白交给卡片本身，颜色只出现在 28px 的图标块和一根细角标上。
 */
export const NODE_STYLES: Record<NodeKind, NodeStyle> = {
  HostInventory: { fill: '#FFF6ED', stroke: '#F79009', accent: '#B54708', label: '主机' },
  NetworkSegment: { fill: '#ECFDF3', stroke: '#12B76A', accent: '#027A48', label: '网段' },
  ProcessSummary: { fill: '#EEF4FF', stroke: '#6366F1', accent: '#4338CA', label: '进程汇总' },
  ProcessGroup: { fill: '#F0F9FF', stroke: '#0BA5EC', accent: '#026AA2', label: '进程分组' },
  ProcessRuntime: { fill: '#F8FAFC', stroke: '#98A2B3', accent: '#475467', label: '进程' },
  ServiceEntity: { fill: '#F4F3FF', stroke: '#7A5AF8', accent: '#5925DC', label: '服务' },
  ServiceInstance: { fill: '#F9F5FF', stroke: '#9E77ED', accent: '#6941C6', label: '服务实例' },
  ContainerRuntime: { fill: '#ECFDF3', stroke: '#12B76A', accent: '#027A48', label: '容器' },
  Subject: { fill: '#FDF2FA', stroke: '#EE46BC', accent: '#C11574', label: '责任人' },
};

export const LAYER_COLORS: Record<LayerKind, { bg: string; border: string; label: string; short: string }> = {
  resource: { bg: '#ECFDF3', border: '#12B76A', label: '资源目录', short: '资源' },
  governance: { bg: '#F5F3FF', border: '#8B5CF6', label: '责任治理', short: '治理' },
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
