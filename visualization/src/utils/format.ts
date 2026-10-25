/** 展示层格式化。集中在一处，免得每个组件各写一份 `toLocaleString`。 */

/** 本地时钟 `17:26:05`。取不到就返回 `-`，不要输出 "Invalid Date"。 */
export function formatClock(iso?: string | null): string {
  if (!iso) return '-';
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '-';
  return date.toLocaleTimeString('zh-CN', { hour12: false });
}

/** 相对时间。后端给的是 RFC3339 带 6 位小数，原样显示没人看。 */
export function formatRelative(iso?: string | null): string {
  if (!iso) return '-';
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '-';

  const seconds = Math.round((Date.now() - date.getTime()) / 1000);
  if (seconds < 5) return '刚刚';
  if (seconds < 60) return `${seconds} 秒前`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟前`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时前`;
  return `${Math.floor(seconds / 86400)} 天前`;
}

/** 千分位。计数是这一屏里最常见的数字，要能一眼读出量级。 */
export function formatCount(value?: number | null): string {
  if (value == null || !Number.isFinite(value)) return '-';
  return value.toLocaleString('zh-CN');
}

/** 路径末段。进程分组用可执行文件全路径当标签会撑爆节点。 */
export function basename(path: string): string {
  const trimmed = path.replace(/[/\\]+$/, '');
  const index = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
  return index >= 0 ? trimmed.slice(index + 1) : trimmed;
}

/** 图上标签的超长截断。完整值在详情面板里。 */
export function truncate(value: string, max = 24): string {
  if (value.length <= max) return value;
  return `${value.slice(0, max - 1)}…`;
}
