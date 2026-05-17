import * as React from "react";

export interface MobileStatRow {
  label: React.ReactNode,
  value: React.ReactNode,
  hidden?: boolean,
}

export interface MobilePanelAction {
  key: string,
  label: string,
  icon: string,
  onClick?: React.MouseEventHandler<HTMLButtonElement>,
  disabled?: boolean,
  selected?: boolean,
}

export function isLandscapeMobile() {
  return window.innerWidth > window.innerHeight;
}

export function resourceImageForName(name: string) {
  return name.toLowerCase().replace(/\s/g, '');
}

export function formatMobileQuantity(quantity) {
  if (quantity > 1000000) {
    return (quantity / 1000000).toFixed(2) + 'M';
  } else if (quantity > 1000) {
    return (quantity / 1000).toFixed(2) + 'K';
  }

  return quantity;
}

export function MobileSplitPanelLayout(props: {
  left: React.ReactNode,
  right: React.ReactNode,
  leftWeight?: number,
  rightWeight?: number,
}) {
  const landscape = isLandscapeMobile();
  const leftWeight = props.leftWeight || 0.95;
  const rightWeight = props.rightWeight || 1.05;

  const layoutStyle: React.CSSProperties = landscape
    ? {
      width: '100%',
      height: '100%',
      minHeight: 0,
      margin: '0 auto',
      display: 'grid',
      gridTemplateColumns: `minmax(220px, ${leftWeight}fr) minmax(300px, ${rightWeight}fr)`,
      gap: '10px',
      alignItems: 'stretch',
    }
    : {
      width: '100%',
      maxWidth: '460px',
      margin: '0 auto',
      display: 'flex',
      flexDirection: 'column',
      gap: '12px',
    };

  const columnStyle: React.CSSProperties = {
    minHeight: 0,
    display: 'flex',
    flexDirection: 'column',
    gap: landscape ? '8px' : '12px',
    overflowY: landscape ? 'auto' : undefined,
    WebkitOverflowScrolling: 'touch',
  };

  return (
    <div style={layoutStyle}>
      <div style={columnStyle}>{props.left}</div>
      <div style={columnStyle}>{props.right}</div>
    </div>
  );
}

export function MobileCard(props: {
  children: React.ReactNode,
  compact?: boolean,
  style?: React.CSSProperties,
}) {
  const landscape = isLandscapeMobile();
  const cardStyle: React.CSSProperties = {
    border: '1px solid rgba(201, 170, 113, 0.28)',
    borderRadius: '6px',
    background: 'rgba(0, 0, 0, 0.18)',
    padding: props.compact || landscape ? '8px 10px' : '10px 12px',
    boxSizing: 'border-box',
    ...props.style,
  };

  return <div style={cardStyle}>{props.children}</div>;
}

export function MobileSummaryCard(props: {
  imageSrc?: string,
  title: React.ReactNode,
  subtitle?: React.ReactNode,
  status?: React.ReactNode,
  imageSize?: number,
}) {
  const landscape = isLandscapeMobile();
  const isItemImage = typeof props.imageSrc == 'string' && props.imageSrc.indexOf('/items/') != -1;
  const imageSize = props.imageSize || (isItemImage ? 48 : (landscape ? 58 : 82));

  const cardStyle: React.CSSProperties = {
    border: '1px solid rgba(201, 170, 113, 0.34)',
    borderRadius: '6px',
    background: 'rgba(255, 255, 255, 0.05)',
    padding: landscape ? '9px 10px' : '15px 12px 13px',
    boxSizing: 'border-box',
    display: 'flex',
    flexDirection: landscape ? 'row' : 'column',
    alignItems: 'center',
    justifyContent: landscape ? 'flex-start' : 'center',
    gap: landscape ? '10px' : '8px',
    minHeight: landscape ? '78px' : undefined,
  };

  const imageStyle: React.CSSProperties = {
    flex: '0 0 auto',
    width: `${imageSize}px`,
    height: `${imageSize}px`,
    objectFit: 'contain',
    imageRendering: 'pixelated',
  };

  const textWrapStyle: React.CSSProperties = {
    minWidth: 0,
    flex: '1 1 auto',
    textAlign: landscape ? 'left' : 'center',
  };

  const titleStyle: React.CSSProperties = {
    color: '#f2e7cf',
    fontFamily: 'Cinzel, Verdana, serif',
    fontSize: landscape ? '17px' : '20px',
    fontWeight: 'bold',
    letterSpacing: 0,
    lineHeight: 1.12,
    margin: 0,
    overflowWrap: 'anywhere',
  };

  const metaStyle: React.CSSProperties = {
    color: '#c9aa71',
    fontFamily: 'Verdana',
    fontSize: '11px',
    lineHeight: 1.25,
    marginTop: '4px',
  };

  return (
    <div style={cardStyle}>
      {props.imageSrc && <img src={props.imageSrc} style={imageStyle} />}
      <div style={textWrapStyle}>
        <h3 style={titleStyle}>{props.title}</h3>
        {props.subtitle && <div style={metaStyle}>{props.subtitle}</div>}
        {props.status && <div style={metaStyle}>{props.status}</div>}
      </div>
    </div>
  );
}

export function MobileStatsList(props: {
  rows: MobileStatRow[],
  compact?: boolean,
}) {
  const rows = props.rows.filter(row => !row.hidden);
  const tableStyle: React.CSSProperties = {
    width: '100%',
    borderCollapse: 'collapse',
    color: '#f2e7cf',
    fontFamily: 'Verdana',
    fontSize: props.compact || isLandscapeMobile() ? '11px' : '12px',
    lineHeight: 1.32,
  };

  const labelCellStyle: React.CSSProperties = {
    color: '#c9aa71',
    width: '36%',
    padding: '4px 8px 4px 0',
    verticalAlign: 'top',
    whiteSpace: 'nowrap',
  };

  const valueCellStyle: React.CSSProperties = {
    color: '#f2e7cf',
    padding: '4px 0',
    verticalAlign: 'top',
    overflowWrap: 'anywhere',
  };

  return (
    <MobileCard compact={props.compact}>
      <table style={tableStyle}>
        <tbody>
          {rows.map((row, index) =>
            <tr key={index}>
              <td style={labelCellStyle}>{row.label}</td>
              <td style={valueCellStyle}>{row.value}</td>
            </tr>
          )}
        </tbody>
      </table>
    </MobileCard>
  );
}

export function MobileRequirementGrid(props: {
  title?: React.ReactNode,
  requirements: any[],
  emptyLabel?: React.ReactNode,
  showCurrent?: boolean,
}) {
  const landscape = isLandscapeMobile();
  const tileSize = landscape ? 76 : 80;

  const titleStyle: React.CSSProperties = {
    color: '#c9aa71',
    fontFamily: 'Verdana',
    fontSize: '11px',
    fontWeight: 'bold',
    textTransform: 'uppercase',
    marginBottom: '7px',
  };

  const gridStyle: React.CSSProperties = {
    display: 'grid',
    gridTemplateColumns: `repeat(auto-fill, ${tileSize}px)`,
    gridAutoRows: `${tileSize}px`,
    gap: landscape ? '6px' : '8px',
    justifyContent: 'start',
    alignItems: 'start',
  };

  const emptyStyle: React.CSSProperties = {
    color: '#777d82',
    fontFamily: 'Verdana',
    fontSize: '11px',
    textAlign: 'center',
    padding: '8px 0',
  };

  const cellStyle = (complete: boolean): React.CSSProperties => ({
    width: `${tileSize}px`,
    height: `${tileSize}px`,
    minHeight: `${tileSize}px`,
    border: complete ? '1px solid rgba(102, 190, 103, 0.45)' : '1px solid rgba(201, 170, 113, 0.22)',
    borderRadius: '4px',
    background: complete ? 'rgba(55, 120, 62, 0.16)' : 'rgba(255,255,255,0.05)',
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '3px',
    padding: '5px 4px',
    boxSizing: 'border-box',
  });

  const imageStyle: React.CSSProperties = {
    width: '48px',
    height: '48px',
    objectFit: 'contain',
    imageRendering: 'pixelated',
  };

  const quantityStyle: React.CSSProperties = {
    color: '#f2e7cf',
    fontFamily: 'Verdana',
    fontSize: '10px',
    textAlign: 'center',
    lineHeight: 1.15,
  };

  const requirements = props.requirements || [];

  return (
    <MobileCard compact={landscape}>
      {props.title && <div style={titleStyle}>{props.title}</div>}
      {requirements.length == 0 && <div style={emptyStyle}>{props.emptyLabel || 'None'}</div>}
      {requirements.length > 0 &&
        <div style={gridStyle}>
          {requirements.map((req, index) => {
            const resourceName = req.type || req.name || '';
            const currentQuantity = req.cquantity != null ? req.cquantity : null;
            const complete = currentQuantity != null && currentQuantity == 0;
            const quantity = props.showCurrent && currentQuantity != null
              ? `${formatMobileQuantity(currentQuantity)}/${formatMobileQuantity(req.quantity)}`
              : formatMobileQuantity(req.quantity);

            return (
              <div key={index} style={cellStyle(complete)}>
                <img src={'/static/art/items/' + resourceImageForName(resourceName) + '.png'} style={imageStyle} />
                <div style={quantityStyle}>{quantity} {resourceName}</div>
              </div>
            );
          })}
        </div>}
    </MobileCard>
  );
}

export function MobilePanelActions(props: {
  actions: MobilePanelAction[],
  compact?: boolean,
  align?: 'center' | 'start' | 'end',
}) {
  const landscape = isLandscapeMobile();
  const size = props.compact || landscape ? 46 : 54;
  const actions = props.actions.filter(Boolean);

  const rowStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: props.align == 'start' ? 'flex-start' : props.align == 'end' ? 'flex-end' : 'center',
    gap: landscape ? '8px' : '10px',
    flexWrap: 'wrap',
  };

  const buttonStyle = (disabled: boolean, selected: boolean): React.CSSProperties => ({
    width: `${size}px`,
    height: `${size}px`,
    border: selected ? '2px solid #c9aa71' : 0,
    borderRadius: selected ? '4px' : 0,
    padding: 0,
    margin: 0,
    background: 'transparent',
    opacity: disabled ? 0.35 : 1,
  });

  const iconStyle: React.CSSProperties = {
    width: `${size}px`,
    height: `${size}px`,
    objectFit: 'contain',
    display: 'block',
    imageRendering: 'pixelated',
  };

  return (
    <div style={rowStyle}>
      {actions.map(action =>
        <button
          key={action.key}
          type="button"
          style={buttonStyle(Boolean(action.disabled), Boolean(action.selected))}
          onClick={action.onClick}
          disabled={action.disabled}
          aria-label={action.label}>
          <img src={action.icon} style={iconStyle} />
        </button>
      )}
    </div>
  );
}
