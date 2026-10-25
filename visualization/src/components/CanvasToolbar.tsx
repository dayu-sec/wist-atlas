import type { LayerKind } from '../types/domain';
import { LAYER_COLORS } from '../utils/colors';
import { SearchIcon } from './icons';

type Props = {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  layerVisibility: Record<LayerKind, boolean>;
  layers: LayerKind[];
  onToggleLayer: (layer: LayerKind) => void;
  matchCount: number | null;
};

/**
 * 画布工具条浮在画布左上角，而不是再占一条通栏。
 *
 * 上一版把「品牌 / 主机名」和「搜索 / 图层」拆成两条 100px 高的横栏，
 * 画布被压到只剩 860px；而搜索和图层的过滤本来就是画布的操作，跟页面导航不是一层。
 *
 * 这里也不再放「布局」下拉：对一张 10 个节点的图暴露 dagre / 力导向，
 * 只会让人选到一个 51% 缩放、字都看不清的结果。布局由画布按图的形状自己选。
 */
export default function CanvasToolbar({
  searchQuery,
  onSearchChange,
  layerVisibility,
  layers,
  onToggleLayer,
  matchCount,
}: Props) {
  return (
    <div className="floating-bar pointer-events-auto flex h-10 items-center gap-0.5 rounded-[11px] px-1">
      <label className="relative flex items-center" aria-label="搜索节点">
        <SearchIcon className="pointer-events-none absolute left-2.5 size-3.5 text-[#98a2b3]" />
        <input
          type="search"
          placeholder="搜索节点"
          value={searchQuery}
          onChange={(event) => onSearchChange(event.target.value)}
          className="h-8 w-[208px] rounded-[8px] bg-transparent pl-8 pr-2 text-[12px] text-ink placeholder:text-[#98a2b3] focus:bg-[#f7f8fa] focus:outline-none"
        />
      </label>

      {searchQuery.trim() !== '' && matchCount != null && (
        <span className="tnum mr-1 shrink-0 text-[10.5px] text-[#98a2b3]">
          {matchCount === 0 ? '无匹配' : `${matchCount} 个匹配`}
        </span>
      )}

      <span className="mx-0.5 h-4 w-px bg-[#eaecf0]" aria-hidden />

      {layers.map((layer) => {
        const active = layerVisibility[layer];
        const color = LAYER_COLORS[layer];
        return (
          <button
            key={layer}
            type="button"
            onClick={() => onToggleLayer(layer)}
            data-active={active}
            title={color.label}
            className={`flex h-7 items-center gap-1.5 rounded-[7px] px-2 text-[11px] transition-colors ${
              active ? 'bg-[#f7f8fa] text-[#344054]' : 'text-[#98a2b3] hover:bg-[#f7f8fa]'
            }`}
          >
            <span
              className="size-2 rounded-[2px]"
              style={{
                backgroundColor: active ? color.border : 'transparent',
                border: `1.5px solid ${active ? color.border : '#cdd3dc'}`,
              }}
              aria-hidden
            />
            {color.short}
          </button>
        );
      })}

    </div>
  );
}
