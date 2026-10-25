import type { NodeKind } from '../types/domain';
import { getNodeStyle } from '../utils/colors';

type Props = {
  /** 当前图里实际出现的对象类型。图例只解释看得见的东西。 */
  kinds: NodeKind[];
};

export default function LegendCard({ kinds }: Props) {
  if (kinds.length === 0) return null;

  return (
    <section className="px-4 py-3.5 border-b border-line">
      <h3 className="text-[10px] font-medium uppercase tracking-wider text-ink-mute">
        图例
      </h3>
      <ul className="mt-2 grid grid-cols-2 gap-x-3 gap-y-1.5">
        {kinds.map((kind) => {
          const style = getNodeStyle(kind);
          return (
            <li key={kind} className="flex items-center gap-1.5 text-[11px] text-ink-soft">
              <span
                className="size-2.5 shrink-0 rounded-[3px]"
                style={{ backgroundColor: style.fill, border: `1.5px solid ${style.stroke}` }}
                aria-hidden
              />
              <span className="truncate">{style.label}</span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
