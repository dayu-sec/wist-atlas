import { formatClock, formatCount } from '../utils/format';

type Props = {
  hostCount: number;
  segmentCount: number;
  processCount: number;
  nodeCount: number;
  edgeCount: number;
  queryTime?: string | null;
  loading: boolean;
};

/** 底部状态栏：把「这张图到底是什么」用一行常数说清楚。 */
export default function StatusBar({
  hostCount,
  segmentCount,
  processCount,
  nodeCount,
  edgeCount,
  queryTime,
  loading,
}: Props) {
  return (
    <footer className="flex h-7 shrink-0 items-center gap-3 border-t border-line bg-surface px-4 text-[11px] text-ink-mute">
      <span className="flex items-center gap-1.5">
        <span
          className={`size-1.5 rounded-full ${loading ? 'animate-pulse bg-amber-400' : 'bg-emerald-500'}`}
          aria-hidden
        />
        {loading ? '加载中' : '数据就绪'}
      </span>

      <span className="h-3 w-px bg-line" aria-hidden />

      <span className="tnum">
        {formatCount(hostCount)} 主机 · {formatCount(segmentCount)} 网段 ·{' '}
        {formatCount(processCount)} 进程
      </span>

      <span className="tnum text-ink-mute/70">
        （图中 {formatCount(nodeCount)} 节点 / {formatCount(edgeCount)} 边）
      </span>

      <span className="ml-auto tnum">
        {queryTime ? `查询于 ${formatClock(queryTime)}` : '未查询'}
      </span>
    </footer>
  );
}
