import * as React from "react";
import { Global } from "../../core/global";
import SelectBox from "./selectBox";
import { TILE, OBJ, DEAD, CORPSE } from "../../core/config";
import { Util } from "../../core/util";
import { Tile } from "../../core/objects/tile";
import styles from "./../ui.module.css";

interface SelectPanelProps {
  selectedTile: Tile,
  objIdsOnTile: any,
  selectedKey: any,
}

export default class SelectPanel extends React.Component<SelectPanelProps, any> {
  render() {
    const tile = this.props.selectedTile as Tile;
    if (!tile) return null;

    const tileIndex = `${tile.hexX}_${tile.hexY}`;
    const tileState = Global.tileStates[tileIndex];
    if (!tileState || !Array.isArray(tileState.tiles) || tileState.tiles.length === 0) return null;

    const tileId = [...tileState.tiles].sort((a, b) => Number(b) - Number(a))[0];
    const tileDef = Global.tileset[tileId];
    const boxes: React.ReactNode[] = [];

    if (tileDef) {
      boxes.push(
        <SelectBox
          key="tile"
          pos={0}
          selectedKey={{ type: TILE, x: tile.hexX, y: tile.hexY }}
          imageName={tileDef.image}
          imageStyle={{ inset: '1px', width: '52px', height: '52px' }}
          showBorder={this.props.selectedKey.type == TILE}
          showGravestone={false}
          label={`${tileDef.name || 'Tile'} at ${tile.hexX}, ${tile.hexY}`}
        />,
      );
    }

    const objectIds = [...(this.props.objIdsOnTile || [])];
    if (this.props.selectedKey.type == OBJ) {
      objectIds.sort((a, b) => {
        if (Number(a) === Number(this.props.selectedKey.id)) return -1;
        if (Number(b) === Number(this.props.selectedKey.id)) return 1;
        return 0;
      });
    }

    let pos = 1;
    for (const rawId of objectIds) {
      const objId: integer = Number(rawId);
      const objectState = Global.objectStates[objId];
      if (!objectState) continue;

      const isDead = objectState.state == DEAD && objectState.class != CORPSE;
      boxes.push(
        <SelectBox
          key={objId}
          pos={pos++}
          selectedKey={{ type: OBJ, id: objId }}
          imageName={Util.getImagePreviewName(objectState.image)}
          showBorder={this.props.selectedKey.id == objId}
          showGravestone={isDead}
          label={objectState.name || `Target ${objId}`}
        />,
      );
    }

    return (
      <nav className={styles.selectionTray} aria-label="Objects on selected tile">
        {boxes}
      </nav>
    );
  }
}
