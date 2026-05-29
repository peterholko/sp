import * as React from "react";
import { createPortal } from "react-dom";
import { Global } from "./global";
import { Util } from "./util";
import { NetworkEvent } from "./networkEvent";

// Renders NPC/villager speech as HTML elements in an overlay above the Phaser
// canvas. Because the bubbles live in the DOM rather than in world space, they
// keep a constant, readable size regardless of the camera zoom. Each bubble is
// re-positioned every animation frame from the camera transform so it stays
// anchored over the speaking object as it moves and the camera pans/zooms.

interface Bubble {
  id: number;
  sourceId: string;
  text: string;
  total: number; // total lifetime in ms (opaque hold + fade)
}

interface State {
  bubbles: Bubble[];
}

const SPRITE_HALF = 36; // sprites are 72px with a top-left origin
const ANCHOR_GAP = 6; // px above the sprite top where the bubble sits

export default class SpeechBubbleLayer extends React.Component<{}, State> {
  private nextId = 1;
  private rafId: number | null = null;
  private timers: Map<number, ReturnType<typeof setTimeout>> = new Map();
  private els: Map<number, HTMLDivElement> = new Map();
  private animated: Set<number> = new Set();

  constructor(props) {
    super(props);
    this.state = { bubbles: [] };
    this.handleSpeech = this.handleSpeech.bind(this);
    this.tick = this.tick.bind(this);
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.SPEECH, this.handleSpeech, this);
  }

  componentWillUnmount() {
    Global.gameEmitter.off(NetworkEvent.SPEECH, this.handleSpeech, this);
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
    this.timers.forEach((t) => clearTimeout(t));
    this.timers.clear();
  }

  handleSpeech(message) {
    // "!" alerts (and messages from objects we don't know about) are handled by
    // the Phaser scene; the HTML layer only renders prose speech.
    if (!message || message.speech === "!" || !Global.objectStates[message.source]) {
      return;
    }

    // Match the previous canvas timing: shorter lines linger ~10s, longer ~6s,
    // each spending the first half fully opaque and the second half fading out.
    const total = message.speech.length < 60 ? 10000 : 6000;

    const id = this.nextId++;
    const bubble: Bubble = {
      id,
      sourceId: String(message.source),
      text: message.speech,
      total,
    };

    this.setState((prev) => ({ bubbles: [...prev.bubbles, bubble] }));

    this.timers.set(id, setTimeout(() => this.removeBubble(id), total));

    if (this.rafId === null) {
      this.rafId = requestAnimationFrame(this.tick);
    }
  }

  removeBubble(id: number) {
    const timer = this.timers.get(id);
    if (timer) clearTimeout(timer);
    this.timers.delete(id);
    this.els.delete(id);
    this.animated.delete(id);
    this.setState((prev) => ({
      bubbles: prev.bubbles.filter((b) => b.id !== id),
    }));
  }

  // Inline arrow refs are re-invoked on every re-render, so guard the fade so it
  // starts exactly once per bubble rather than restarting on each render.
  setBubbleRef(id: number, total: number, el: HTMLDivElement | null) {
    if (!el) {
      this.els.delete(id);
      return;
    }

    this.els.set(id, el);

    if (!this.animated.has(id) && typeof el.animate === "function") {
      this.animated.add(id);
      el.animate(
        [
          { opacity: 1, offset: 0 },
          { opacity: 1, offset: 0.5 },
          { opacity: 0, offset: 1 },
        ],
        { duration: total, easing: "linear", fill: "forwards" }
      );
    }
  }

  tick() {
    const scene: any =
      Global.game && Global.game.scene
        ? Global.game.scene.getScene("ObjectScene")
        : null;

    if (scene) {
      this.state.bubbles.forEach((bubble) => {
        const el = this.els.get(bubble.id);
        if (!el) return;

        // Prefer the live rendered object (reflects movement tweens); fall back
        // to the last known hex position from object state.
        let worldX: number;
        let worldY: number;
        const obj = scene.objectList ? scene.objectList[bubble.sourceId] : null;
        if (obj) {
          worldX = obj.x;
          worldY = obj.y;
        } else {
          const st = Global.objectStates[bubble.sourceId];
          if (!st) {
            el.style.display = "none";
            return;
          }
          const p = Util.hex_to_pixel(st.x, st.y);
          worldX = p.x;
          worldY = p.y;
        }

        const page = Util.worldToPage(scene, worldX + SPRITE_HALF, worldY - ANCHOR_GAP);
        el.style.display = "";
        el.style.transform = `translate(${page.x}px, ${page.y}px) translate(-50%, -100%)`;
      });
    }

    if (this.state.bubbles.length > 0) {
      this.rafId = requestAnimationFrame(this.tick);
    } else {
      this.rafId = null;
    }
  }

  render() {
    if (this.state.bubbles.length === 0) {
      return null;
    }

    const layerStyle: React.CSSProperties = {
      position: "fixed",
      left: 0,
      top: 0,
      width: "100%",
      height: "100%",
      pointerEvents: "none",
      zIndex: 5,
    };

    return createPortal(
      <div style={layerStyle}>
        {this.state.bubbles.map((bubble) => {
          const hasBg = bubble.text.length >= 5;
          const bubbleStyle: React.CSSProperties = {
            position: "absolute",
            left: 0,
            top: 0,
            width: 120,
            boxSizing: "border-box",
            padding: hasBg ? "4px 6px" : 0,
            background: hasBg ? "rgba(0, 0, 0, 0.5)" : "transparent",
            borderRadius: 5,
            color: "#FFFFFF",
            fontFamily: "Alegreya, serif",
            fontSize: 14,
            lineHeight: 1.2,
            textAlign: "center",
            overflowWrap: "break-word",
            whiteSpace: "normal",
            willChange: "transform",
            // Start off-screen until the first rAF tick positions the bubble, to
            // avoid a one-frame flash in the top-left corner.
            transform: "translate(-9999px, -9999px)",
          };

          return (
            <div
              key={bubble.id}
              ref={(el) => this.setBubbleRef(bubble.id, bubble.total, el)}
              style={bubbleStyle}
            >
              {bubble.text}
            </div>
          );
        })}
      </div>,
      document.body
    );
  }
}
