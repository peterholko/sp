import assert from "node:assert/strict";
import { canUseInventoryItem } from "./itemUsePolicy";

assert.equal(
  canUseInventoryItem({ class: "Medical", subclass: "Bandage" }),
  true,
  "Crude Bandages must expose the Use action",
);
assert.equal(
  canUseInventoryItem({ class: "Medical", subclass: "Splint" }),
  false,
  "unknown medical items must not expose a nonfunctional Use action",
);

for (const itemClass of ["Potion", "Deed", "Food", "Drink"]) {
  assert.equal(
    canUseInventoryItem({ class: itemClass, subclass: "Test" }),
    true,
    `${itemClass} remains usable`,
  );
}

for (const subclass of ["Bucket", "Fishing Rod", "Waterskin", "Bedroll"]) {
  assert.equal(
    canUseInventoryItem({ class: "Test", subclass }),
    true,
    `${subclass} remains usable`,
  );
}

assert.equal(canUseInventoryItem({ class: "Weapon", subclass: "Spear" }), false);
assert.equal(canUseInventoryItem(undefined), false);

console.log("Item use policy checks passed");
