export interface KeyboardMoveLike {
  key: string;
  shiftKey?: boolean;
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  isComposing?: boolean;
}

export interface KeyboardMoveDelta {
  dx: number;
  dy: number;
}

export const KEYBOARD_MOVE_STEP = 16;
export const KEYBOARD_MOVE_FAST_STEP = 48;

export function keyboardMoveDelta(
  event: KeyboardMoveLike,
  step = KEYBOARD_MOVE_STEP,
  fastStep = KEYBOARD_MOVE_FAST_STEP,
): KeyboardMoveDelta | null {
  if (event.isComposing || event.altKey || event.ctrlKey || event.metaKey) return null;

  const amount = event.shiftKey ? fastStep : step;
  switch (event.key.toLowerCase()) {
    case 'a':
    case 'arrowleft':
      return { dx: -amount, dy: 0 };
    case 'd':
    case 'arrowright':
      return { dx: amount, dy: 0 };
    case 'w':
    case 'arrowup':
      return { dx: 0, dy: -amount };
    case 's':
    case 'arrowdown':
      return { dx: 0, dy: amount };
    default:
      return null;
  }
}
