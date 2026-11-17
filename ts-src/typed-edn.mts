export class TypedEdnSetView {
  readonly items: any[];

  constructor(items: any[]) {
    this.items = items;
  }
}

// Keep source entries until their keys have been decoded to their declared type.
export class TypedEdnMapView {
  readonly entries: [any, any][];

  constructor(entries: [any, any][]) {
    this.entries = entries;
  }
}
