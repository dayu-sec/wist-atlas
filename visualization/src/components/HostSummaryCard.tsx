import type { HostTopologyHost } from '../types/domain';
import { formatCount, formatRelative } from '../utils/format';

type Props = {
  host: HostTopologyHost | null;
  segmentCount: number;
  processCount: number;
  groupCount: number;
  serviceCount: number;
  generatedAt?: string | null;
  loading: boolean;
};

/**
 * 主机资产卡 —— 这一屏真正要回答的问题。
 *
 * 之前的界面只有一张图，系统/架构/网段数/进程数一个都看不到；而它们本来就在
 * `hostTopology` 里，已经取回来了却没人渲染。
 */
export default function HostSummaryCard({
  host,
  segmentCount,
  processCount,
  groupCount,
  serviceCount,
  generatedAt,
  loading,
}: Props) {
  const os = [host?.osName, host?.osVersion].filter(Boolean).join(' ');

  return (
    <section className="px-4 py-4 border-b border-line">
      <div className="flex items-start gap-2.5">
        <span
          className="mt-1.5 size-2.5 shrink-0 rotate-45 rounded-[2px] bg-amber-400 ring-2 ring-amber-100"
          aria-hidden
        />
        <div className="min-w-0">
          <h2 className="text-[15px] font-semibold leading-tight tracking-tight text-ink truncate">
            {host?.hostName ?? (loading ? '加载中…' : '未选择主机')}
          </h2>
          <p className="mt-0.5 text-[11px] text-ink-mute">
            {os || '系统信息未采集'}
            {host?.arch ? ` · ${host.arch}` : ''}
          </p>
        </div>
      </div>

      <dl className="mt-4 grid grid-cols-2 gap-2">
        <Metric label="网段" value={formatCount(segmentCount)} />
        <Metric label="进程" value={formatCount(processCount)} />
        <Metric label="进程分组" value={formatCount(groupCount)} />
        <Metric label="服务" value={formatCount(serviceCount)} />
      </dl>

      <div className="mt-4 space-y-2 border-t border-line pt-3">
        <IdRow label="主机 ID" value={host?.id} />
        <IdRow label="机器 ID" value={host?.machineId} />
        <div className="flex items-baseline justify-between gap-3 text-[11px]">
          <span className="shrink-0 text-ink-mute">盘点于</span>
          {generatedAt ? (
            <span className="text-ink-soft" title={generatedAt}>
              {formatRelative(generatedAt)}
            </span>
          ) : (
            <span className="text-ink-soft">—</span>
          )}
        </div>
      </div>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md bg-canvas px-2.5 py-2">
      <dt className="text-[10px] text-ink-mute">{label}</dt>
      <dd className="tnum text-[17px] font-semibold leading-tight text-ink">{value}</dd>
    </div>
  );
}

/**
 * 一行 ID。标签在上、值独占整行 —— UUID 有 36 个字符，塞在标签右侧必然折行。
 * 值为空时整行不渲染（macOS 就没有 machine-id，别留一个孤零零的破折号）。
 */
function IdRow({ label, value }: { label: string; value?: string | null }) {
  if (!value) return null;

  return (
    <div className="text-[11px]">
      <span className="text-ink-mute">{label}</span>
      <p className="mt-0.5 truncate font-mono text-[10px] leading-snug text-ink-soft" title={value}>
        {value}
      </p>
    </div>
  );
}
