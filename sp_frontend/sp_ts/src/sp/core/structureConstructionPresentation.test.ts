import { BUILDING, FOUNDED, NONE, STRUCTURE, UNIT, UPGRADING } from './config';
import { usesFoundationGraphic } from './structureConstructionPresentation';

describe('structure construction presentation', () => {
  test('keeps the foundation graphic through active construction', () => {
    expect(usesFoundationGraphic({ class: STRUCTURE, state: FOUNDED })).toBe(true);
    expect(usesFoundationGraphic({ class: STRUCTURE, state: BUILDING })).toBe(true);
  });

  test('uses the completed graphic only outside initial construction', () => {
    expect(usesFoundationGraphic({ class: STRUCTURE, state: NONE })).toBe(false);
    expect(usesFoundationGraphic({ class: STRUCTURE, state: UPGRADING })).toBe(false);
    expect(usesFoundationGraphic({ class: UNIT, state: BUILDING })).toBe(false);
  });
});
