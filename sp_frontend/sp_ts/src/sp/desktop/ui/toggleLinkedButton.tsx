
import * as React from "react";
import smalliconborder from "ui_comp/selectbordersmall.png";

interface ToggleLinkedButtonProps {
  handler: any,
  imageName: string,
  style?: any,
  className?: any,
  displayInline?: boolean,
  toggleIconBorder: boolean
}

export default class ToggleLinkedButton extends React.Component<ToggleLinkedButtonProps, any> {

  constructor(props) {
    super(props);

    this.state = {
      toggleIconBorder: false
    };

    this.handleClick = this.handleClick.bind(this);
  }

  handleClick = () => {
    this.props.handler();
  }

  render() {

    return (
      <div onClick={this.handleClick} style={this.props.displayInline ? {display: 'inline'} : {}}>
        {this.props.style &&
          <img src={'/static/art/ui/' + this.props.imageName + '.png'} style={this.props.style} />}
        {this.props.toggleIconBorder && this.props.style && <img src={smalliconborder} style={this.props.style} />}

        {!this.props.style &&
          <img src={'/static/art/ui/' + this.props.imageName + '.png'} className={this.props.className} />}
        {this.props.toggleIconBorder && !this.props.style && <img src={smalliconborder} className={this.props.className} />}
      </div>
    );
  }
}
