
import * as React from "react";

interface SmallButtonProps {
  handler: any,
  imageName: string,
  style: any,
}

export default class SmallButton extends React.Component<SmallButtonProps, any> {

  constructor(props) {
    super(props);

    this.state = {
      showClicked: false
    };

    this.startTimer = this.startTimer.bind(this);
    this.handleClick = this.handleClick.bind(this);
    this.hideImage = this.hideImage.bind(this);
  }

  handleClick = () => {
    this.props.handler();
    this.setState({showClicked: true});

    this.startTimer();
  } 

  startTimer() {
    setTimeout(this.hideImage, 100);
  }

  hideImage() {
    this.setState({showClicked: false});
  }

  render() {

    const divStyle = {
      display: "inline"
    } as React.CSSProperties

    return (
      <div style={divStyle} onClick={this.handleClick}>
        <img src={'/static/art/ui/' + this.props.imageName + '.png'} style={this.props.style} />
        {this.state.showClicked && <img src={'/static/art/ui/' + this.props.imageName + '_click.png'} style={this.props.style} /> }
      </div>
    );
  }
}
