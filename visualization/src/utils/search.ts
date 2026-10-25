import type { HostProcessTopologyGraph } from '../types/domain';

/**
 * 节点搜索的统一索引。
 *
 * 网段的「接口名 / IP」不在节点属性里，而在 `host_network_assoc` 边上：
 * 只索引节点属性的话，搜 `bridge`、搜 `en0` 都是 0 结果 —— 而这两样正是用户最常搜的东西。
 * 把边上的属性并进两端节点的文本，工具条的命中数和画布的变淡才能对齐。
 */
export function buildSearchIndex(graph: HostProcessTopologyGraph | null): Map<string, string> {
  const index = new Map<string, string>();
  if (!graph) return index;

  for (const node of graph.nodes) {
    index.set(
      node.id,
      [node.label, ...Object.values(node.properties).map(String)].join(' ').toLowerCase(),
    );
  }

  for (const edge of graph.edges) {
    const extra = Object.values(edge.properties ?? {}).map(String).join(' ').toLowerCase();
    if (!extra.trim()) continue;
    for (const id of [edge.source, edge.target]) {
      index.set(id, `${index.get(id) ?? ''} ${extra}`);
    }
  }

  return index;
}

export function countMatches(index: Map<string, string>, query: string): number {
  const needle = query.trim().toLowerCase();
  if (!needle) return 0;
  let count = 0;
  for (const text of index.values()) {
    if (text.includes(needle)) count += 1;
  }
  return count;
}
