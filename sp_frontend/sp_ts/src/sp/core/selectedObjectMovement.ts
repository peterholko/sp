import type { Tile } from './objects/tile';

export interface SelectedObjectMovementPresentation {
  selectedTile: Tile;
  objIdsOnTile: any[];
  selectedBoxPos: number;
}

/**
 * Keep a moving selected object selected without treating its destination as a
 * fresh player tile click. The tile occupies selection position zero, so an
 * object's UI position is its array index plus one.
 */
export function selectedObjectMovementPresentation(
  selectedObjectId: unknown,
  hexX: number,
  hexY: number,
  objectIds: any[],
): SelectedObjectMovementPresentation {
  const objIdsOnTile = [...objectIds];
  const selectedIndex = objIdsOnTile.findIndex(
    (id) => Number(id) === Number(selectedObjectId),
  );

  return {
    // Selection panels only consume the logical coordinates; retaining a live
    // Phaser Tile here would couple UI state to a render object that may be
    // replaced during a map redraw.
    selectedTile: { hexX, hexY } as Tile,
    objIdsOnTile,
    selectedBoxPos: selectedIndex >= 0 ? selectedIndex + 1 : 1,
  };
}
