export const ROW_HEIGHT = 28;
const OVERSCAN = 6;

export interface Window {
  first: number;
  count: number;
  above: number;
  below: number;
}

export function windowOf(total: number, scrollTop: number, viewport: number): Window {
  if (total === 0) return { first: 0, count: 0, above: 0, below: 0 };
  const visible = Math.max(1, Math.ceil(viewport / ROW_HEIGHT));
  const first = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const count = Math.min(total - first, visible + OVERSCAN * 2);
  return {
    first,
    count,
    above: first * ROW_HEIGHT,
    below: Math.max(0, (total - first - count) * ROW_HEIGHT),
  };
}

export function scrollToKeep(index: number, scrollTop: number, viewport: number): number {
  const top = index * ROW_HEIGHT;
  const bottom = top + ROW_HEIGHT;
  if (top < scrollTop) return top;
  if (bottom > scrollTop + viewport) return bottom - viewport;
  return scrollTop;
}
