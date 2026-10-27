// How times, durations and costs are written on the page.

const two = (n: number) => String(n).padStart(2, '0');

/// A time as month-day hour:minute:second, in the browser's time zone.
export function time(ms: number | null | undefined): string {
  if (ms == null) return '';
  const d = new Date(ms);
  return `${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`;
}

/// A duration in its two largest units: 2h 05m, 3m 07s, 41s.
export function duration(ms: number | null | undefined): string {
  if (ms == null || ms < 0) return '';
  const s = Math.floor(ms / 1000);
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (d > 0) return `${d}d ${two(h)}h`;
  if (h > 0) return `${h}h ${two(m)}m`;
  if (m > 0) return `${m}m ${two(sec)}s`;
  return `${sec}s`;
}

/// A cost in US dollars, to the cent.
export function cost(usd: number | null | undefined): string {
  if (usd == null) return '';
  return `$${usd.toFixed(2)}`;
}
