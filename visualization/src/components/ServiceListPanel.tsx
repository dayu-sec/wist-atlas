import type { ServiceSummaryDto } from '../types/domain';
import { formatCount } from '../utils/format';
import { ChevronRight, KindGlyph } from './icons';

type Props = {
  services: ServiceSummaryDto[] | null;
  loading: boolean;
  error?: string | null;
  onOpenService: (serviceObjectId: string) => void;
  onClose: () => void;
  collapsed: boolean;
  onToggle: () => void;
};

/** 默认先给这么多行：28 个服务全铺开会把左栏的资产卡挤出屏幕。 */
const VISIBLE_LIMIT = 12;

/**
 * 左栏的「服务清单」。
 *
 * 画布上只放 TopN 服务（一台开发机就是 28 个），但「到底有哪几套服务、各自铺在多
 * 少台机上」这个问题不能只靠搜。这里按主机数排序列全量，点一行就进服务焦点视图。
 */
export default function ServiceListPanel({
  services,
  loading,
  error,
  onOpenService,
  onClose,
  collapsed,
  onToggle,
}: Props) {
  const total = services?.length ?? 0;
  const visible = services?.slice(0, VISIBLE_LIMIT) ?? [];
  const hidden = Math.max(0, total - visible.length);

  return (
    <section className="border-b border-line">
      <div className="flex items-center justify-between px-4 py-3">
        <button
          type="button"
          onClick={onToggle}
          className="flex items-center gap-1 text-[9.5px] font-medium uppercase tracking-[0.08em] text-[#98a2b3] transition-colors hover:text-[#475467]"
        >
          <ChevronRight
            className={`size-3 transition-transform ${collapsed ? '' : 'rotate-90'}`}
          />
          服务清单
          {total > 0 && <span className="text-[#b4bcc8]"> {formatCount(total)}</span>}
        </button>
        <button
          type="button"
          onClick={onClose}
          className="rounded px-1 text-[10px] text-[#b4bcc8] transition-colors hover:text-[#475467]"
          aria-label="关闭服务清单"
        >
          ✕
        </button>
      </div>

      {collapsed ? null : (
        <div className="px-4 pb-3.5">
          {error ? (
            <p className="text-[10.5px] leading-4 text-[#b42318]">{error}</p>
          ) : loading && !services ? (
            <p className="text-[10.5px] text-[#98a2b3]">正在读取服务清单…</p>
          ) : visible.length === 0 ? (
            <p className="text-[10.5px] text-[#98a2b3]">还没有采集到服务</p>
          ) : (
            <ul className="-mx-1 space-y-0.5">
              {visible.map((service) => (
                <li key={service.id}>
                  <button
                    type="button"
                    onClick={() => onOpenService(service.id)}
                    title={`${service.name} · ${service.externalRef ?? ''} · 点击查看监听详情`}
                    className="flex w-full items-center gap-2 rounded-[7px] px-1 py-1 text-left transition-colors hover:bg-[#f9fafb]"
                  >
                    <span
                      className="grid size-5 shrink-0 place-items-center rounded-[6px]"
                      style={{ backgroundColor: '#F4F3FF', color: '#7A5AF8' }}
                      aria-hidden
                    >
                      <KindGlyph kind="ServiceEntity" className="size-3" />
                    </span>
                    <span className="min-w-0 flex-1 truncate text-[11px] leading-4 text-[#344054]">
                      {service.name}
                    </span>
                    <span className="tnum shrink-0 font-mono text-[9.5px] text-[#98a2b3]">
                      {service.hostCount > 0
                        ? `${service.hostCount} 主机 · ${service.instanceCount} 实例`
                        : '未运行'}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}

          {hidden > 0 && (
            <p className="mt-1.5 px-1 text-[9.5px] text-[#b4bcc8]">
              还有 {hidden} 个服务未列出（按主机数排序后取前 {VISIBLE_LIMIT}）
            </p>
          )}
        </div>
      )}
    </section>
  );
}
