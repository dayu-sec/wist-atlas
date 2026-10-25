import { formatClock, formatCount } from '../utils/format';

type Props = {
  hostCount: number;
  segmentCount: number;
  processCount: number;
  groupCount: number;
  nodeCount: number;
  edgeCount: number;
  queryTime?: string | null;
  loading: boolean;
};

/** 底部状态栏：一行说完「这份图覆盖了什么、什么时候查的」。 */
export default function StatusBar({
  hostCount,
  segmentCount,
  processCount,
  groupCount,
  nodeCount,
  edgeCount,
  queryTime,
  loading,
}: Props) {
  return (
    <footer className="flex h-7 shrink-0 items-center gap-3 border-t border-line bg-surface px-4 text-[10.5px] text-[#98a2b3]">
      <span className="flex items-center gap-1.5">
        <span
          className={`size-1.5 rounded-full ${loading ? 'animate-pulse bg-[#f79009]' : 'bg-[#12b76a]'}`}
          aria-hidden
        />
        {loading ? '同步中' : '数据就绪'}
      </span>

      <span className="h-3 w-px bg-line" aria-hidden />

      <span className="tnum text-[#667085]">
        {formatCount(hostCount)} 主机 · {formatCount(segmentCount)} 网段 ·{' '}
        {formatCount(processCount)} 进程 · {formatCount(groupCount)} 分组
      </span>

      <span className="ml-auto tnum hidden sm:inline">
        图中 {formatCount(nodeCount)} 节点 / {formatCount(edgeCount)} 条关系
      </span>

      <span className="h-3 w-px bg-line" aria-hidden />

      <span className="tnum">{queryTime ? `查询于 ${formatClock(queryTime)}` : '未查询'}</span>
    </footer>
  );
}
