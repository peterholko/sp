
import * as React from "react";
import smalliconborder from "ui_comp/selectbordersmall.png";

interface ToggleButtonProps {
  handler: any,
  imageName: string,
  className: any,
  title?: string,
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
    this.setState({toggleIconBorder: !this.state.toggleIconBorder});
  }

  render() {

    return (
      <div onClick={this.handleClick}>
        <img
          src={'/static/art/ui/' + this.props.imageName + '.png'}
          className={this.props.className}
          title={this.props.title}
          alt={this.props.title}
          aria-label={this.props.title} />
        {this.state.toggleIconBorder && <img src={smalliconborder} className={this.props.className} />}
      </div>
    );
  }
}
