export interface LatencySample {
  action: string;
  milliseconds: number;
}

const samples: LatencySample[] = [];
const CAPACITY = 500;

export function measurePaint(action: string, started: number) {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      samples.push({ action, milliseconds: performance.now() - started });
      if (samples.length > CAPACITY) samples.shift();
    });
  });
}

export function percentile(fraction: number): number | null {
  if (samples.length === 0) return null;
  const sorted = samples.map((sample) => sample.milliseconds).sort((left, right) => left - right);
  const rank = Math.round(fraction * (sorted.length - 1));
  return sorted[Math.min(rank, sorted.length - 1)];
}

export function observed(): number {
  return samples.length;
}

export function drain(): LatencySample[] {
  return samples.splice(0, samples.length);
}
