
import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";

interface SkillsPanelProps {
  skillsData,
}

export default class SkillsPanel extends React.Component<SkillsPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
   
  }

  render() {
    var objId = this.props.skillsData.id;
    var imageName = Global.objectStates[objId].image;
    imageName = imageName.replace(/ /g, '') + '_single.png';
    var name = Global.objectStates[objId].name;

    const skills = [];

    const imageStyle = {
      position: 'absolute',
      top: '25px',
      left: '126px',
      width: '72px',
      height: '72px',
      objectFit: 'contain'
    } as React.CSSProperties

    const spanNameStyle = {
      position: 'absolute',
      top: '96px',
      left: '12px',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '299px',
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap'
    } as React.CSSProperties

    const tableContainerStyle = {
      position: 'absolute',
      top: '122px',
      right: '12px',
      bottom: '18px',
      left: '12px',
      overflowY: 'auto',
      overflowX: 'hidden'
    } as React.CSSProperties

    const tableStyle = {
      width: '100%',
      tableLayout: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '11px',
      lineHeight: '18px',
      borderCollapse: 'collapse'
    } as React.CSSProperties

    const cellStyle = {
      padding: '2px 3px',
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap'
    } as React.CSSProperties

    const headingStyle = {
      ...cellStyle,
      position: 'sticky',
      top: 0,
      background: 'rgba(23, 27, 25, 0.94)',
      textAlign: 'left',
      zIndex: 1
    } as React.CSSProperties


    var key = 0;

    for(var skill in this.props.skillsData.skills) {
      skills.push(<tr key={key}>
                    <td style={cellStyle} title={skill}>{skill}</td>
                    <td style={cellStyle}>{this.props.skillsData.skills[skill].level}</td>
                    <td style={cellStyle}>{this.props.skillsData.skills[skill].xp}</td>
                    <td style={cellStyle}>{this.props.skillsData.skills[skill].next}</td>
                  </tr>);

      key++;
    }

    return (
      <HalfPanel left={false} 
                 panelType={'skills'} 
                 hideExitButton={false}>
        <img src={'/static/art/' + imageName} style={imageStyle} />
        <span style={spanNameStyle}>{name}</span>
        <div style={tableContainerStyle}>
          <table style={tableStyle}>
            <colgroup>
              <col style={{ width: '43%' }} />
              <col style={{ width: '16%' }} />
              <col style={{ width: '16%' }} />
              <col style={{ width: '25%' }} />
            </colgroup>
            <thead>
              <tr>
                <th style={headingStyle}>Name</th>
                <th style={headingStyle}>Level</th>
                <th style={headingStyle}>XP</th>
                <th style={headingStyle}>Next Level</th>
              </tr>
            </thead>
            <tbody>{skills}</tbody>
          </table>
        </div>
      </HalfPanel>
    );
  }
}
