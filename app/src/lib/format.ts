const UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']

/** 1024 进制，1 位小数；小于 1KB 显示整数字节。 */
export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return '0 B'
  let value = n
  let unit = 0
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024
    unit++
  }
  // 1023.97 KB 四舍五入后会显示 1024.0 KB，进位到下一单位。
  if (unit > 0 && value >= 1023.95 && unit < UNITS.length - 1) {
    value /= 1024
    unit++
  }
  return `${unit === 0 ? Math.round(value) : value.toFixed(1)} ${UNITS[unit]}`
}

const pad = (n: number) => String(n).padStart(2, '0')

/** unix 秒 → 本地 YYYY-MM-DD（桌面工具里日期足够，不显示秒）。 */
export function formatMtime(mtime: number): string {
  if (!mtime) return '—'
  const d = new Date(mtime * 1000)
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

export function formatCount(n: number): string {
  return Math.round(n).toLocaleString('en-US')
}

export function percent(part: number, total: number): number {
  return total > 0 ? (part / total) * 100 : 0
}
