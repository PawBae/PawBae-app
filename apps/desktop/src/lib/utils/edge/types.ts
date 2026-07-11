export interface MonitorRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface MascotRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface EdgeState {
  monitor: MonitorRect;
  mascot: MascotRect;
  onLeft: boolean;
  onRight: boolean;
  onTop: boolean;
  onBottom: boolean;
  activeWindow: ActiveWindowEdge | null;
}

export interface ActiveWindowEdge {
  rect: { x: number; y: number; width: number; height: number };
  windowId: number;
  ownerName: string;
  ownerPid: number;
  onTopOfWindow: boolean;
  onLeftOfWindow: boolean;
  onRightOfWindow: boolean;
  onBottomOfWindow: boolean;
  withinHorizontalRange: boolean;
  withinVerticalRange: boolean;
}

export interface SpritePad {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface PetFloorInfo {
  onDockY: number;
  offDockY: number;
  dockXRange: [number, number] | null;
}

export interface SpriteAnchorsCSS {
  topPx: number | null;
  rightPx: number | null;
  bottomPx: number | null;
  leftPx: number | null;
}
