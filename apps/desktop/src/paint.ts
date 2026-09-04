export function observePaint(report: (name: string, startTime: number) => void) {
  for (const entry of performance.getEntriesByType("paint")) {
    report(entry.name, entry.startTime);
  }
  if (typeof PerformanceObserver === "undefined") return;
  const observer = new PerformanceObserver((list) => {
    for (const entry of list.getEntries()) {
      report(entry.name, entry.startTime);
    }
  });
  observer.observe({ type: "paint", buffered: true });
}
