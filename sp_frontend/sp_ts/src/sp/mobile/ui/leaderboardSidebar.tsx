import * as React from "react";

interface LeaderboardEntry {
  id: string | number;
  heroName: string;
  heroRank: string;
  totalScore: string | number;
  daysSurvived: number;
  legendaryKills: number;
  fate: string;
}

interface Props {
  entries: LeaderboardEntry[];
  maxEntries?: number;
  pageSize?: number;
}

interface State {
  page: number;
}

export default class LeaderboardSidebar extends React.Component<Props, State> {
  state: State = { page: 0 };

  handlePrev = () => {
    this.setState(prev => ({ page: Math.max(0, prev.page - 1) }));
  };

  handleNext = (totalPages: number) => () => {
    this.setState(prev => ({ page: Math.min(totalPages - 1, prev.page + 1) }));
  };

  render() {
    const max = this.props.maxEntries ?? 50;
    const pageSize = this.props.pageSize ?? 10;
    const allEntries = this.props.entries.slice(0, max);

    if (allEntries.length === 0) {
      return null;
    }

    const totalPages = Math.max(1, Math.ceil(allEntries.length / pageSize));
    const page = Math.min(this.state.page, totalPages - 1);
    const pageStart = page * pageSize;
    const top = allEntries.slice(pageStart, pageStart + pageSize);

    const containerStyle: React.CSSProperties = {
      position: 'fixed',
      top: 'calc(8px + env(safe-area-inset-top, 0px))',
      right: 'calc(8px + env(safe-area-inset-right, 0px))',
      left: 'calc(8px + env(safe-area-inset-left, 0px))',
      maxWidth: '290px',
      marginLeft: 'auto',
      maxHeight: 'calc(100vh - 220px)',
      backgroundColor: 'rgba(8, 10, 12, 0.82)',
      border: '1px solid rgba(201, 170, 113, 0.38)',
      borderRadius: '4px',
      padding: '9px 10px',
      zIndex: 50,
      pointerEvents: 'auto',
      boxSizing: 'border-box',
      display: 'flex',
      flexDirection: 'column',
      overflowY: 'auto',
    };

    const titleStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Verdana',
      fontSize: '11px',
      fontWeight: 'bold',
      marginBottom: '6px',
      textTransform: 'uppercase',
    };

    const entryStyle: React.CSSProperties = {
      padding: '5px 0',
      borderTop: '1px solid rgba(255,255,255,0.08)',
      fontFamily: 'Verdana',
    };

    const topRowStyle: React.CSSProperties = {
      display: 'flex',
      justifyContent: 'space-between',
      gap: '6px',
      fontSize: '10px',
      lineHeight: 1.3,
    };

    const indexStyle: React.CSSProperties = {
      color: '#9aa0a6',
      fontSize: '9px',
      flex: '0 0 auto',
      minWidth: '14px',
    };

    const nameStyle: React.CSSProperties = {
      flex: '1 1 auto',
      whiteSpace: 'nowrap',
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      color: '#f2e7cf',
      fontWeight: 'bold',
    };

    const scoreStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontWeight: 'bold',
      flex: '0 0 auto',
    };

    const metaRowStyle: React.CSSProperties = {
      display: 'flex',
      justifyContent: 'space-between',
      gap: '6px',
      fontSize: '9px',
      lineHeight: 1.3,
      marginTop: '1px',
      paddingLeft: '14px',
      color: '#9aa0a6',
    };

    const rankStyle: React.CSSProperties = {
      color: '#8fb7d9',
    };

    const statsStyle: React.CSSProperties = {
      flex: '0 0 auto',
    };

    const fateStyle: React.CSSProperties = {
      fontSize: '9px',
      lineHeight: 1.3,
      marginTop: '2px',
      paddingLeft: '14px',
      color: '#b89270',
      fontStyle: 'italic',
      whiteSpace: 'normal',
      wordBreak: 'break-word',
    };

    const paginationStyle: React.CSSProperties = {
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'center',
      marginTop: '8px',
      paddingTop: '6px',
      borderTop: '1px solid rgba(255,255,255,0.14)',
      fontFamily: 'Verdana',
      fontSize: '10px',
      color: '#9aa0a6',
    };

    const pageButtonStyle: React.CSSProperties = {
      background: 'transparent',
      border: '1px solid rgba(201, 170, 113, 0.38)',
      borderRadius: '3px',
      color: '#c9aa71',
      cursor: 'pointer',
      fontFamily: 'Verdana',
      fontSize: '10px',
      padding: '2px 8px',
    };

    const pageButtonDisabledStyle: React.CSSProperties = {
      ...pageButtonStyle,
      borderColor: 'rgba(255,255,255,0.1)',
      color: '#555',
      cursor: 'default',
    };

    return (
      <div style={containerStyle}>
        <div style={titleStyle}>Hall of Heroes</div>
        <div style={{ flex: '1 1 auto' }}>
          {top.map((entry, idx) => (
            <div key={entry.id} style={entryStyle}>
              <div style={topRowStyle}>
                <span style={indexStyle}>{pageStart + idx + 1}.</span>
                <span style={nameStyle}>{entry.heroName}</span>
                <span style={scoreStyle}>{entry.totalScore}</span>
              </div>
              <div style={metaRowStyle}>
                <span style={rankStyle}>{entry.heroRank}</span>
                <span style={statsStyle}>
                  Day {entry.daysSurvived}
                  {entry.legendaryKills > 0 && ` · ${entry.legendaryKills} legend${entry.legendaryKills === 1 ? '' : 's'}`}
                </span>
              </div>
              {entry.fate && <div style={fateStyle}>{entry.fate}</div>}
            </div>
          ))}
        </div>
        {totalPages > 1 && (
          <div style={paginationStyle}>
            <button
              type="button"
              style={page === 0 ? pageButtonDisabledStyle : pageButtonStyle}
              onClick={this.handlePrev}
              disabled={page === 0}
            >
              ‹ Prev
            </button>
            <span>Page {page + 1} / {totalPages}</span>
            <button
              type="button"
              style={page >= totalPages - 1 ? pageButtonDisabledStyle : pageButtonStyle}
              onClick={this.handleNext(totalPages)}
              disabled={page >= totalPages - 1}
            >
              Next ›
            </button>
          </div>
        )}
      </div>
    );
  }
}
