import type { HostSummaryDto, HostTopologyHost } from '../types/domain';
import { formatRelative } from '../utils/format';
import { BrandMark, ChevronRight, RefreshIcon } from './icons';

type Props = {
  host: HostTopologyHost | null;
  hostName: string;
  /** 主机列表：多于一台时顶栏变成切换器。 */
  hosts: HostSummaryDto[];
  /** 当前聚焦的主机；空字符串 = 全部主机（总览）。 */
  selectedHostId: string;
  onSelectHost: (hostId: string) => void;
  scannedAt?: string | null;
  loading: boolean;
  onRefresh: () => void;
};

/**
 * 一条 52px 的顶栏承担四件事：产品身份、我在哪、换一台机器看、这份数据有多新。
 * 搜索/布局这些画布操作不放这里 —— 它们属于画布，见 CanvasToolbar。
 */
export default function AppHeader({
  host,
  hostName,
  hosts,
  selectedHostId,
  onSelectHost,
  scannedAt,
  loading,
  onRefresh,
}: Props) {
  const os = [host?.osName, host?.osVersion].filter(Boolean).join(' ');
  const relative = scannedAt ? formatRelative(scannedAt) : null;

  return (
    <header className="flex h-[52px] shrink-0 items-center gap-2.5 border-b border-line bg-surface px-4">
      <div className="flex items-center gap-2">
        <BrandMark className="size-[26px]" />
        <span className="text-[13px] font-semibold tracking-[-0.01em] text-ink">wist-atlas</span>
      </div>

      <span className="mx-1 h-4 w-px bg-line-strong/70" aria-hidden />

      <nav className="flex min-w-0 items-center gap-1 text-[12px]" aria-label="位置">
        <span className="shrink-0 text-[#98a2b3]">资产</span>
        <ChevronRight className="size-3 shrink-0 text-[#d0d5dd]" />
        <span className="shrink-0 text-[#98a2b3]">主机</span>
        <ChevronRight className="size-3 shrink-0 text-[#d0d5dd]" />
        {hosts.length > 1 ? (
          <select
            value={selectedHostId}
            onChange={(event) => onSelectHost(event.target.value)}
            aria-label="切换主机"
            title={`共 ${hosts.length} 台主机`}
            className="h-6 max-w-[220px] truncate rounded-md border border-line bg-surface px-1.5 text-[12px] font-medium text-ink focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/15"
          >
            <option value="">全部主机（总览）</option>
            {hosts.map((item) => (
              <option key={item.id} value={item.id}>
                {item.hostName}
                {item.stale ? '（已退役）' : ''}
              </option>
            ))}
          </select>
        ) : (
          <span className="truncate font-medium text-ink" title={hostName}>
            {hostName}
          </span>
        )}
      </nav>

      {os && (
        <span className="ml-1 hidden shrink-0 rounded-md border border-line bg-[#f9fafb] px-1.5 py-0.5 text-[10px] text-[#475467] xl:inline">
          {os}
        </span>
      )}
      {host?.arch && (
        <span className="hidden shrink-0 rounded-md border border-line bg-[#f9fafb] px-1.5 py-0.5 font-mono text-[10px] text-[#667085] xl:inline">
          {host.arch}
        </span>
      )}

      <div className="ml-auto flex shrink-0 items-center gap-3">
        <span className="flex items-center gap-1.5 text-[11px] text-[#667085]">
          <span className="relative flex size-1.5">
            <span className="absolute inline-flex size-full animate-ping rounded-full bg-[#12b76a] opacity-40" />
            <span className="relative inline-flex size-1.5 rounded-full bg-[#12b76a]" />
          </span>
          {loading ? '同步中' : relative ? `盘点于 ${relative}` : '等待同步'}
        </span>

        <button
          type="button"
          onClick={onRefresh}
          disabled={loading}
          className="flex h-8 items-center gap-1.5 rounded-lg border border-line bg-surface px-2.5 text-[12px] font-medium text-[#344054] transition-colors hover:border-[#d0d5dd] hover:bg-[#f9fafb] disabled:cursor-not-allowed disabled:opacity-50"
        >
          <RefreshIcon className={`size-3.5 text-[#667085] ${loading ? 'animate-spin' : ''}`} />
          刷新
        </button>
      </div>
    </header>
  );
}
