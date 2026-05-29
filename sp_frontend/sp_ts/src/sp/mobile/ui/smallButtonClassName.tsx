
import * as React from "react";

interface SmallButtonProps {
  handler: any,
  imageName: string,
  className: any,
  title?: string,
}

export default class SmallButtonClassName extends React.Component<SmallButtonProps, any> {

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

    return (
      <div onClick={this.handleClick}>
        <img
          src={'/static/art/ui/' + this.props.imageName + '.png'}
          className={this.props.className}
          title={this.props.title}
          alt={this.props.title}
          aria-label={this.props.title} />
        {this.state.showClicked &&
          <img
            src={'/static/art/ui/' + this.props.imageName + '_click.png'}
            className={this.props.className}
            title={this.props.title}
            alt={this.props.title}
            aria-label={this.props.title} /> }
      </div>
    );
  }
}
