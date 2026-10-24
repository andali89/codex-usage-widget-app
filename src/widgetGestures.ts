export type PointerPoint = { x: number; y: number };

export function shouldStartWidgetDrag(
  origin: PointerPoint,
  current: PointerPoint,
  positionLocked: boolean,
  dragging: boolean,
): boolean {
  return !positionLocked && !dragging && Math.hypot(current.x - origin.x, current.y - origin.y) >= 6;
}

export function shouldActivateWidgetClick(clickCount: number, suppressClick: boolean): boolean {
  return !suppressClick && clickCount <= 1;
}
