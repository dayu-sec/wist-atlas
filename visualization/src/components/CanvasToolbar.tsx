import type { LayerKind, LayoutName } from '../types/domain';
import { LAYER_COLORS } from '../utils/colors';

type Props = {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  layerVisibility: Record<LayerKind, boolean>;
  onToggleLayer: (layer: LayerKind) => void;
  layout: LayoutName;
  onLayoutChange: (layout: LayoutName) => void;
  matchCount: number | null;
};

const ALL_LAYERS: LayerKind[] = ['resource', 'governance'];

const LAYOUT_OPTIONS: { value: LayoutName; label: string }[] = [
  { value: 'auto', label: '自动' },
  { value: 'dagre', label: '层次' },
  { value: 'concentric', label: '中心放射' },
  { value: 'cose-bilkent', label: '力导向' },
];

export default function CanvasToolbar({
  searchQuery,
  onSearchChange,
  layerVisibility,
  onToggleLayer,
  layout,
  onLayoutChange,
  matchCount,
}: Props) {
  return (
    <div className="flex h-11 shrink-0 items-center gap-2 border-b border-line bg-surface px-3">
      <div className="relative">
        <svg
          className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-ink-mute"
          viewBox="0 0 20 20"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
        >
          <circle cx="8.5" cy="8.5" r="5.5" />
          <path d="m13 13 4 4" strokeLinecap="round" />
        </svg>
        <input
          type="search"
          placeholder="搜索节点"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          className="h-7 w-52 rounded-md border border-line bg-canvas pl-7 pr-2 text-[12px] text-ink placeholder:text-ink-mute focus:border-brand focus:bg-surface focus:outline-none focus:ring-2 focus:ring-brand/15"
        />
      </div>

      {searchQuery.trim() !== '' && matchCount != null && (
        <span className="tnum text-[11px] text-ink-mute">{matchCount} 个匹配</span>
      )}

      <div className="mx-1 h-5 w-px bg-line" aria-hidden />

      <div className="flex items-center gap-1">
        {ALL_LAYERS.map((layer) => {
          const active = layerVisibility[layer];
          return (
            <button
              key={layer}
              type="button"
              onClick={() => onToggleLayer(layer)}
              data-active={active}
              className="seg flex h-7 items-center gap-1.5 rounded-md px-2 text-[11px] text-ink-soft transition-colors hover:border-line-strong"
            >
              <span
                className="size-2 rounded-[2px]"
                style={{
                  backgroundColor: active ? LAYER_COLORS[layer].border : 'transparent',
                  border: `1.5px solid ${LAYER_COLORS[layer].border}`,
                }}
                aria-hidden
              />
              {LAYER_COLORS[layer].label}
            </button>
          );
        })}
      </div>

      <div className="ml-auto flex items-center gap-2">
        <label className="text-[11px] text-ink-mute" htmlFor="layout-select">
          布局
        </label>
        <select
          id="layout-select"
          value={layout}
          onChange={(e) => onLayoutChange(e.target.value as LayoutName)}
          className="h-7 rounded-md border border-line bg-surface px-1.5 text-[11px] text-ink-soft focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/15"
        >
          {LAYOUT_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
