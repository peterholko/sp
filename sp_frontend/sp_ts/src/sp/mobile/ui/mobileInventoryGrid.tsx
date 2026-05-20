import * as React from "react";

interface MobileInventoryGridProps {
  ownerId: integer,
  items: any[],
  selectedItemId?: integer,
  disabledItems?: any,
  onSelect?: Function,
  compact?: boolean,
  emptyLabel?: string,
}

export default class MobileInventoryGrid extends React.Component<MobileInventoryGridProps, any> {
  formatQuantity(quantity) {
    if (quantity > 1000000) {
      return (quantity / 1000000).toFixed(2) + 'M';
    } else if (quantity > 1000) {
      return (quantity / 1000).toFixed(2) + 'K';
    }
    return quantity;
  }

  handleSelect(item, index) {
    if (!this.props.onSelect) return;
    this.props.onSelect({
      ownerId: this.props.ownerId,
      itemId: item.id,
      itemName: item.name,
      index,
    });
  }

  render() {
    const compact = Boolean(this.props.compact);
    const items = this.props.items || [];
    const tileSize = compact ? 56 : 58;
    const gap = compact ? 6 : 8;

    const gridStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: `repeat(auto-fill, ${tileSize}px)`,
      gridAutoRows: `${tileSize}px`,
      gap: `${gap}px`,
      alignItems: 'start',
      justifyContent: 'start',
    };

    const emptyStyle: React.CSSProperties = {
      color: '#777d82',
      fontSize: '11px',
      lineHeight: 1.35,
      padding: '12px 0',
      textAlign: 'center',
    };

    if (items.length == 0) {
      return <div style={emptyStyle}>{this.props.emptyLabel || 'Empty'}</div>;
    }

    return (
      <div style={gridStyle}>
        {items.map((item, index) => {
          const disabled = this.props.disabledItems != null && this.props.disabledItems.includes(item.id);
          const selected = this.props.selectedItemId == item.id;

          const cellStyle: React.CSSProperties = {
            position: 'relative',
            width: `${tileSize}px`,
            height: `${tileSize}px`,
            minHeight: `${tileSize}px`,
            border: selected ? '2px solid #c9aa71' : '1px solid rgba(201, 170, 113, 0.28)',
            borderRadius: '4px',
            background: selected ? 'rgba(201, 170, 113, 0.18)' : 'rgba(255,255,255,0.06)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            opacity: disabled ? 0.45 : 1,
            boxSizing: 'border-box',
            overflow: 'hidden',
          };

          const imageStyle: React.CSSProperties = {
            width: '48px',
            height: '48px',
            objectFit: 'contain',
            imageRendering: 'pixelated',
          };

          const quantityStyle: React.CSSProperties = {
            position: 'absolute',
            right: '2px',
            bottom: '1px',
            color: 'white',
            fontFamily: 'Verdana',
            fontSize: '10px',
            WebkitTextStroke: '0.5px black',
            fontWeight: 'bold',
            pointerEvents: 'none',
          };

          return (
            <button
              key={item.id || index}
              type="button"
              style={cellStyle}
              disabled={disabled}
              onClick={() => this.handleSelect(item, index)}
              title={item.equipped ? item.name + ' (equipped)' : item.name}
            >
              <img src={'/static/art/items/' + item.image + '.png'} style={imageStyle} />
              <span style={quantityStyle}>{this.formatQuantity(item.quantity)}</span>
            </button>
          );
        })}
      </div>
    );
  }
}
