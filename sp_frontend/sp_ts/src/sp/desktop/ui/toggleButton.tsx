
import * as React from "react";
import smalliconborder from "ui_comp/selectbordersmall.png";

interface ToggleButtonProps {
  handler: any,
  imageName: string,
  className: any,
  title?: string,
  active?: boolean,
}

export default class ToggleButton extends React.Component<ToggleButtonProps, any> {

  constructor(props) {
    super(props);

    this.state = {
      toggleIconBorder: false
    };

    this.handleClick = this.handleClick.bind(this);
  }

  handleClick = () => {
    this.props.handler();
    if (this.props.active === undefined) {
      this.setState({toggleIconBorder: !this.state.toggleIconBorder});
    }
  }

  render() {
    const active = this.props.active ?? this.state.toggleIconBorder;

    return (
      <div onClick={this.handleClick}>
        <img
          src={'/static/art/ui/' + this.props.imageName + '.png'}
          className={this.props.className}
          title={this.props.title}
          alt={this.props.title}
          aria-label={this.props.title}
          aria-pressed={active} />
        {active && <img src={smalliconborder} className={this.props.className} alt="" aria-hidden="true" />}
      </div>
    );
  }
}
