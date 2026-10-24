export interface Versioned { processEpoch: string; revision: number }

export class Replica<T extends Versioned> {
  current: T | null = null;
  private retiredEpochs = new Set<string>();
  apply(next: T): { accepted: boolean; gap: boolean } {
    const old = this.current;
    if (this.retiredEpochs.has(next.processEpoch) || (old?.processEpoch === next.processEpoch && old.revision >= next.revision)) return { accepted: false, gap: false };
    const gap = old?.processEpoch === next.processEpoch && next.revision > old.revision + 1;
    if (old && old.processEpoch !== next.processEpoch) this.retiredEpochs.add(old.processEpoch);
    this.current = next;
    return { accepted: true, gap };
  }
}
