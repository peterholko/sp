// zIndexManager.ts
export class ZIndexManager {
  private base = 6;   // base layer for z-index
  private top = this.base;
  private order = new Map<string, number>();

  register(id: string) {
    if (!this.order.has(id)) {
      this.top += 1;
      this.order.set(id, this.top);
    }
    return this.order.get(id)!;
  }

  bringToFront(id: string) {
    this.top += 1;
    this.order.set(id, this.top);
    return this.top;
  }

  get(id: string) {
    return this.order.get(id) ?? this.base;
  }

  unregister(id: string) {
    this.order.delete(id);
  }

  getTop() { return this.top; } // add this
}