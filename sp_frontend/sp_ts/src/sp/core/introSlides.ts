export interface IntroSlide {
  image: string;
  alt: string;
  text: string;
}

export const INTRO_SLIDE_HOLD_MS = 7000;
export const INTRO_SLIDE_FADE_MS = 1000;

export const INTRO_SLIDES: readonly IntroSlide[] = [
  {
    image: "/static/art/ui/intro_01_new_lands.png",
    alt: "Three crewmates approaching an unexplored coast",
    text: `We sailed beyond the charted sea in search of new lands—three crewmates chasing the promise of an untouched coast.
At dawn, mountains rose from the mist.`,
  },
  {
    image: "/static/art/ui/intro_02_shipwreck.png",
    alt: "The expedition ship breaking apart on coastal reefs",
    text: `Before we could find safe anchorage, the weather turned. Wind and black water drove the ship onto hidden reefs.
The hull split beneath us.`,
  },
  {
    image: "/static/art/ui/intro_03_ashore.png",
    alt: "A lone survivor crawling ashore beside the wreck",
    text: `You crawled ashore alone. Your two crewmates appear to have perished aboard the wreck.
A campfire still burns. Search the wreck, face what stirs inside, and survive.
Welcome to Perilous.`,
  },
];
