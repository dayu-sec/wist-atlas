import type { NodeKind } from '../types/domain';

type IconProps = { className?: string };

/** 所有图标共用 24×24 视框、1.7 描边、round 端点 —— 混用不同线宽会让卡片显脏。 */
function Stroke({ className = 'size-4', children }: IconProps & { children: React.ReactNode }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden
    >
      {children}
    </svg>
  );
}

export function HostGlyph({ className }: IconProps) {
  return (
    <Stroke className={className}>
      <rect x="3" y="4.5" width="18" height="11.5" rx="2" />
      <path d="M2 19.5h20" />
    </Stroke>
  );
}

export function NetworkGlyph({ className }: IconProps) {
  return (
    <Stroke className={className}>
      <circle cx="5.5" cy="12" r="2.2" />
      <circle cx="18.5" cy="6" r="2.2" />
      <circle cx="18.5" cy="18" r="2.2" />
      <path d="M7.6 11.1 16.4 6.9M7.6 12.9l8.8 4.2" />
    </Stroke>
  );
}

export function LayersGlyph({ className }: IconProps) {
  return (
    <Stroke className={className}>
      <path d="m12 3.5 8.5 4.6-8.5 4.6-8.5-4.6L12 3.5Z" />
      <path d="m3.5 12.5 8.5 4.6 8.5-4.6" />
      <path d="m3.5 16.8 8.5 4.6 8.5-4.6" />
    </Stroke>
  );
}

export function TerminalGlyph({ className }: IconProps) {
  return (
    <Stroke className={className}>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="m7.5 9.5 3 3-3 3M13.5 15.5H17" />
    </Stroke>
  );
}

export function CpuGlyph({ className }: IconProps) {
  return (
    <Stroke className={className}>
      <rect x="7" y="7" width="10" height="10" rx="2" />
      <path d="M10 3v4M14 3v4M10 17v4M14 17v4M3 10h4M3 14h4M17 10h4M17 14h4" />
    </Stroke>
  );
}

export function BoxGlyph({ className }: IconProps) {
  return (
    <Stroke className={className}>
      <path d="m12 3 8 4.5v9L12 21l-8-4.5v-9L12 3Z" />
      <path d="m4 7.5 8 4.5 8-4.5M12 12v9" />
    </Stroke>
  );
}

export function InstanceGlyph({ className }: IconProps) {
  return (
    <Stroke className={className}>
      <rect x="4" y="4" width="16" height="16" rx="5" />
      <circle cx="12" cy="12" r="2.4" />
    </Stroke>
  );
}

export function UserGlyph({ className }: IconProps) {
  return (
    <Stroke className={className}>
      <circle cx="12" cy="8" r="3.4" />
      <path d="M5.2 20a6.9 6.9 0 0 1 13.6 0" />
    </Stroke>
  );
}

export function KindGlyph({ kind, className }: { kind: NodeKind; className?: string }) {
  switch (kind) {
    case 'HostInventory':
      return <HostGlyph className={className} />;
    case 'NetworkSegment':
      return <NetworkGlyph className={className} />;
    case 'ProcessSummary':
      return <LayersGlyph className={className} />;
    case 'ProcessGroup':
      return <TerminalGlyph className={className} />;
    case 'ServiceEntity':
      return <BoxGlyph className={className} />;
    case 'ServiceInstance':
      return <InstanceGlyph className={className} />;
    case 'Subject':
      return <UserGlyph className={className} />;
    default:
      return <CpuGlyph className={className} />;
  }
}

export function SearchIcon({ className = 'size-4' }: IconProps) {
  return (
    <Stroke className={className}>
      <circle cx="10.5" cy="10.5" r="6" />
      <path d="m15 15 4.5 4.5" />
    </Stroke>
  );
}

export function RefreshIcon({ className = 'size-4' }: IconProps) {
  return (
    <Stroke className={className}>
      <path d="M19.5 10a7.5 7.5 0 1 0-.7 4.4" />
      <path d="M19.5 4.5V10H14" />
    </Stroke>
  );
}

export function ChevronRight({ className = 'size-3.5' }: IconProps) {
  return (
    <Stroke className={className}>
      <path d="m9 5 7 7-7 7" />
    </Stroke>
  );
}

export function CloseIcon({ className = 'size-4' }: IconProps) {
  return (
    <Stroke className={className}>
      <path d="m5 5 14 14M19 5 5 19" />
    </Stroke>
  );
}

export function PlusIcon({ className = 'size-4' }: IconProps) {
  return (
    <Stroke className={className}>
      <path d="M12 5v14M5 12h14" />
    </Stroke>
  );
}

export function MinusIcon({ className = 'size-4' }: IconProps) {
  return (
    <Stroke className={className}>
      <path d="M5 12h14" />
    </Stroke>
  );
}

export function FitIcon({ className = 'size-4' }: IconProps) {
  return (
    <Stroke className={className}>
      <path d="M9 4H4v5M15 4h5v5M9 20H4v-5M15 20h5v-5" />
    </Stroke>
  );
}

export function ArrowRightIcon({ className = 'size-3.5' }: IconProps) {
  return (
    <Stroke className={className}>
      <path d="M4 12h15M13 6l6 6-6 6" />
    </Stroke>
  );
}

export function BrandMark({ className = 'size-7' }: IconProps) {
  return (
    <svg viewBox="0 0 28 28" className={className} aria-hidden>
      <rect width="28" height="28" rx="8" fill="#101828" />
      <path d="M14 6.4 21.6 14 14 21.6 6.4 14 14 6.4Z" fill="none" stroke="#818CF8" strokeWidth="1.5" />
      <circle cx="14" cy="14" r="2.4" fill="#818CF8" />
    </svg>
  );
}

export function DiamondDot({ className = 'size-2' }: IconProps) {
  return (
    <svg viewBox="0 0 8 8" className={className} aria-hidden>
      <path d="M4 0.6 7.4 4 4 7.4 0.6 4 4 0.6Z" fill="currentColor" />
    </svg>
  );
}
