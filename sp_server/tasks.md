#### Nov 2025 ####
- [x] Converted Items from a Resource to Inventory Components on each Entity
- [x] Reworked Structure Crafting and Structure Refining 

#### Oct 2025 ####
- [ ] Cancel previous actions/event when starting a new one
- [x] Bug, different types of copper ore are merging with any existing Copper Ore items 
- [ ] Adding cooking for raw meat
- [x] Currently need a recipe for each subtype of cooked meat  
- [ ] Move Leaderboard link as it is making the login screen's height too large for Mobile
- [ ] FindDrinkEvent appears to get stuck not completing


#### Apr 2025 ####
- [ ] NPC pathfinding cannot handle unreachable destinations when using hunter behavior
- [ ] Tax Collector should not be able to enter the stockade
- [ ] Tax Collector boarded any nearby ship including the merchant ship which caused the villager loaded on the merchant ship to be floating in the water

#### March 2025 ####
- [ ] Hero somehow refined a resource while not on the structure (can't reproduce)
- [ ] Villager's activity not updating for mining and refining


#### Feb 2025 ####
- [x] Structures not visible are not removed during re-render
- [x] BUG => Refine event should be removed from the event queue after it is processed
- [ ] villager should drink and eat when thirsty or hungry (difficult to implement)

#### Jan 2025 ####
- [ ] Structures out of LOS should stay visibile until they re-enter LOS
- [ ] Villager keeps just planting on the farm as the crop is growing, farming action do not seem to end ever
- [ ] Villager capacity should be independent from the template capacity, based off strength attributes etc...
- [x] Ordering villager to gather should include position otherwise they will not gather at the same location
- [ ] If villager moves away from crafting structure while crafting, the crafting will continue and the item will be crafted.  Crafting Orders are tied to the villager not the structure currently.  Should be tied to the structure.
- [ ] Villager activity does not always update to crafting  
- [ ] Consider revamping Monolith sanctuary being tied to a player, right now it is proxity based

#### Oct 2024 ####
- [x] Hero Crafting does not work, crafterId is -1 
- [x] Cannot see recipe resource requirement on iOS safari nor iOS chrome or desktop Safari due to a display issue 
- [x] Correct double error message due to Login Panel's Error Handler
- [ ] Move Thirst/Hunger/Tiredness/Heat to the Villager attribute tab
- [ ] Consider adding a thoughts/need villager attribute
- [x] Add cookie for authentication for easy client reconnection 
- [ ] If server disconnects or client disconnects, display an error message then when Ok is pressed refresh the page
- [x] Add Gravestone if die animation doesn't exist for update sprite
- [x] FindDrink should not be instanteously, add a game event 
- [ ] Add portrait support for mobile
- [x] Add Hero Name to class selection
- [ ] Seed and Crop types currently hardcoded
- [ ] Need slight improvement to UI panels when Hero or Villagers start an Order 
- [x] Add crafting queue for OrderCraft
- [x] Bug => Refine / Craft will fail if there is a wall around the crafting structure due to lookup
- [ ] Decide on encounters
- [x] Assign villagers to a structure needs improvement
- [x] Villager is stuck in "Run for your lives" state
- [ ] Villager Farm Tend Order does nothing currently
- [ ] Villager does not have a dialogue for harvesting
- [x] Hero cannot farm currently
- [x] Do not allow spawns on top of hero
- [x] Effect text should be bold
- [x] After resurrection it appears the perception data needs to be resend
- [x] Resurrection should trigger Sanctuary addition
- [x] Add a "you are dead" message after dying
- [x] Wild spawns should be discouraged from moving into sanctuary areas
- [x] Despawn the multiple outside sanctuary spawns after some period
- [x] Hero death animation is no longer playing, nor is the hero turning into a corpse 
- [ ] If vision range decreases while objects are at the further range hex, they will not be removed from the rendered map
- [ ] Goblin pillager are not changing graphics when dying, nor allowing to be looted

#### Aug 2024 ####

Select highlight hex on client side needs to be moved from map scene to object scene as it is hiding behind walls


#### Jan 5, 2022 ####

** Complete structure construction

#### Dec 14, 2022 ####

** A* star pathfinding (completed)
*** Utility AI + Orders (50% completed)



