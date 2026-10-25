import type { NodeKind } from '../types/domain';

/**
 * 图上节点的「版式尺寸」。
 *
 * 一张卡片能放多少字，决定了布局要留多大的间距 —— 所以尺寸和布局必须在同一处定义。
 * 之前节点是不带内容的几何体（菱形/六边形），尺寸只需要「看起来有层级」；
 * 现在每张卡片上有标题、副标题、计数，尺寸就是信息密度的约束。
 */
export const NODE_DIMS: Record<NodeKind, { width: number; height: number }> = {
  HostInventory: { width: 320, height: 148 },
  NetworkSegment: { width: 256, height: 64 },
  ProcessSummary: { width: 320, height: 272 },
  ProcessGroup: { width: 224, height: 60 },
  ProcessRuntime: { width: 176, height: 44 },
  ServiceEntity: { width: 232, height: 68 },
  ServiceInstance: { width: 208, height: 56 },
  ContainerRuntime: { width: 220, height: 60 },
  Subject: { width: 184, height: 56 },
};

export function nodeDims(kind: NodeKind) {
  return NODE_DIMS[kind] ?? { width: 200, height: 56 };
}

/** 主机、网段、运行时这些「版式块」的圆角与卡片语言保持一致。 */
export const CARD_RADIUS = 12;

/** 对象类型的中文名。图例、详情面板、卡片角标共用一份，避免三处各写一份。 */
export const KIND_LABELS: Record<NodeKind, string> = {
  HostInventory: '主机',
  NetworkSegment: '网段',
  ProcessSummary: '运行时',
  ProcessGroup: '进程分组',
  ProcessRuntime: '进程',
  ServiceEntity: '服务',
  ServiceInstance: '服务实例',
  ContainerRuntime: '容器',
  Subject: '责任人',
};

export function kindLabel(kind: NodeKind) {
  return KIND_LABELS[kind] ?? kind;
}

/** `Ipv4` / `ipv6` 这种后端枚举，在卡片上要显示成人能读的 `IPv4`。 */
export function formatAddressFamily(value: unknown): string {
  const raw = String(value ?? '').toLowerCase();
  if (raw === 'ipv4' || raw === 'v4') return 'IPv4';
  if (raw === 'ipv6' || raw === 'v6') return 'IPv6';
  return raw ? raw.toUpperCase() : '';
}

/**
 * IPv6 的 CIDR 有 38 个字符，卡片一行放不下。
 * 中间截断而不是尾部截断：前缀段（fd07:…）和掩码（/64）都是判断依据，不能丢。
 */
export function shortCidr(cidr: string, max = 27): string {
  if (cidr.length <= max) return cidr;
  const head = Math.ceil((max - 1) / 2);
  const tail = Math.floor((max - 1) / 2);
  return `${cidr.slice(0, head)}…${cidr.slice(-tail)}`;
}

export type ProgramCount = { name: string; count: number };

/**
 * `ProcessSummary.topPrograms` 是一句人读的摘要：
 * `plugin-container x35, com.apple.audio.SandboxHelper x32, …`
 * 卡片要把它拆成结构化行，不要原样塞进去。
 */
export function parseTopPrograms(value: unknown): ProgramCount[] {
  if (typeof value !== 'string' || !value.trim()) return [];
  return value
    .split(',')
    .map((part) => /^(.*?)\s+x(\d+)\s*$/.exec(part.trim()))
    .filter((match): match is RegExpExecArray => Boolean(match))
    .map((match) => ({ name: match[1].trim(), count: Number(match[2]) }))
    .filter((item) => Number.isFinite(item.count));
}

export function numericProperty(properties: Record<string, unknown>, key: string): number {
  const value = Number(properties[key] ?? 0);
  return Number.isFinite(value) ? value : 0;
}
