"""
game_state.py — Tracks game world state from server packets.
"""

import math
from dataclasses import dataclass, field
from typing import Optional


HOSTILE_SUBCLASSES = {"npc", "undead", "demon", "bandit"}

TICK_PHASE = {
    0: "Night",
    400: "First Light",
    500: "Dawn",
    600: "Morning",
    1200: "Afternoon",
    1800: "Evening",
    2000: "Dusk",
    2200: "Night",
}


def tick_phase(tick: int) -> str:
    phase = "Night"
    for t, name in sorted(TICK_PHASE.items()):
        if tick >= t:
            phase = name
    return phase


class GameState:
    def __init__(self, player_id: int):
        self.player_id = player_id
        self.hero: dict = {}
        self.objects: dict[int, dict] = {}   # id -> MapObj attrs
        self.map_tiles: dict[tuple, dict] = {}  # (x,y) -> tile
        self.current_tick: int = 0
        self.current_day: int = 1
        self.time_of_day: str = "Morning"
        self.hp: int = 100
        self.max_hp: int = 100
        self.stamina: int = 100
        self.inventory: list[dict] = []
        self.events: list[str] = []       # recent notices, capped at 10
        self.recent_damage: list[dict] = []  # recent dmg packets, capped at 5
        self.objectives: dict = {
            "build_campfire": False,
            "build_3_structures": False,
            "recruit_villager": False,
            "explore_poi": False,
            "survive_5_nights": False,
        }
        self._initialized = False

    # ------------------------------------------------------------------ #
    # Packet processing
    # ------------------------------------------------------------------ #

    def process(self, pkt: dict):
        packet_type = pkt.get("packet")

        if packet_type == "init_perception":
            self._process_init_perception(pkt["data"])
            self._initialized = True

        elif packet_type == "new_perception":
            self._process_init_perception(pkt["data"])

        elif packet_type == "perception_changes":
            self._process_changes(pkt.get("events", []))

        elif packet_type == "stats":
            d = pkt.get("data", {})
            self.hp = d.get("hp", self.hp)
            self.max_hp = d.get("base_hp", self.max_hp)
            self.stamina = d.get("stamina", self.stamina)

        elif packet_type == "world":
            self.time_of_day = pkt.get("time_of_day", self.time_of_day)
            self.current_day = pkt.get("day", self.current_day)

        elif packet_type == "info_inventory":
            if pkt.get("id") == self.hero.get("id"):
                self.inventory = pkt.get("items", [])

        elif packet_type == "objectives":
            self.objectives = {
                "build_campfire": pkt.get("build_campfire", False),
                "build_3_structures": pkt.get("build_3_structures", False),
                "recruit_villager": pkt.get("recruit_villager", False),
                "explore_poi": pkt.get("explore_poi", False),
                "survive_5_nights": pkt.get("survive_5_nights", False),
            }

        elif packet_type == "Notice":
            msg = pkt.get("noticemsg", "")
            if msg:
                self.events.append(msg)
                self.events = self.events[-10:]  # keep last 10

        elif packet_type == "dmg":
            target_id = pkt.get("target_id")
            hero_id = self.hero.get("id")
            if target_id == hero_id:
                dmg = pkt.get("dmg", 0)
                source_id = pkt.get("source_id")
                attacker = self.objects.get(source_id, {})
                attacker_name = attacker.get("name", f"entity {source_id}")
                self.recent_damage.append({
                    "dmg": dmg,
                    "source_id": source_id,
                    "source_name": attacker_name,
                    "state": pkt.get("state", ""),
                })
                self.recent_damage = self.recent_damage[-5:]
                # Update HP from state if dead
                if pkt.get("state") == "Dead":
                    self.hp = 0

    def _process_init_perception(self, data: dict):
        # Map tiles
        for tile in data.get("map", []):
            self.map_tiles[(tile["x"], tile["y"])] = tile

        # All visible objects (includes hero, villagers, NPCs, structures, resources)
        self.objects.clear()
        for obj in data.get("visible_objs", []):
            self.objects[obj["id"]] = obj

        # Our hero is in observers
        for obs in data.get("observers", []):
            if obs["player"] == self.player_id:
                self.hero = obs
                self.objects[obs["id"]] = obs
                break

    def _process_changes(self, events: list):
        for ev in events:
            event_type = ev.get("event")
            if event_type == "obj_create":
                obj = ev.get("obj", {})
                self.objects[obj["id"]] = obj
            elif event_type == "obj_move":
                obj = ev.get("obj", {})
                self.objects[obj["id"]] = obj
                # Update hero position if it's us
                if obj["id"] == self.hero.get("id"):
                    self.hero["x"] = obj["x"]
                    self.hero["y"] = obj["y"]
                    self.hero["state"] = obj.get("state", self.hero.get("state"))
            elif event_type == "obj_update":
                obj_id = ev.get("obj_id")
                if obj_id in self.objects:
                    for attr in ev.get("attrs", []):
                        self.objects[obj_id][attr["attr"]] = attr["value"]
                    if obj_id == self.hero.get("id"):
                        for attr in ev.get("attrs", []):
                            self.hero[attr["attr"]] = attr["value"]
            elif event_type == "obj_delete":
                obj_id = ev.get("obj_id")
                self.objects.pop(obj_id, None)

    # ------------------------------------------------------------------ #
    # Queries
    # ------------------------------------------------------------------ #

    def hero_pos(self) -> tuple[int, int]:
        return self.hero.get("x", 0), self.hero.get("y", 0)

    def hero_id(self) -> int:
        return self.hero.get("id", -1)

    def nearby_objects(self, radius: int = 10) -> list[dict]:
        hx, hy = self.hero_pos()
        result = []
        for obj in self.objects.values():
            if obj["id"] == self.hero.get("id"):
                continue
            dx = obj["x"] - hx
            dy = obj["y"] - hy
            dist = math.sqrt(dx * dx + dy * dy)
            if dist <= radius:
                result.append({**obj, "_dist": round(dist, 1)})
        result.sort(key=lambda o: o["_dist"])
        return result

    def hostile_npcs(self, radius: int = 15) -> list[dict]:
        return [
            o for o in self.nearby_objects(radius)
            if o.get("subclass", "").lower() in HOSTILE_SUBCLASSES
            or o.get("class", "").lower() == "npc"
        ]

    def villagers(self) -> list[dict]:
        return [
            o for o in self.objects.values()
            if o.get("player") == self.player_id
            and o.get("subclass", "").lower() == "villager"
        ]

    def structures(self) -> list[dict]:
        return [
            o for o in self.objects.values()
            if o.get("player") == self.player_id
            and o.get("class", "").lower() == "structure"
        ]

    def resources_nearby(self, radius: int = 8) -> list[dict]:
        return [
            o for o in self.nearby_objects(radius)
            if o.get("class", "").lower() == "resource"
        ]

    def inv_item(self, name: str) -> dict | None:
        name_lower = name.lower()
        for item in self.inventory:
            if item.get("name", "").lower() == name_lower:
                return item
        return None

    def inv_quantity(self, name: str) -> int:
        item = self.inv_item(name)
        return item.get("quantity", 0) if item else 0

    # ------------------------------------------------------------------ #
    # Summary for Claude
    # ------------------------------------------------------------------ #

    def summary(self) -> str:
        hx, hy = self.hero_pos()
        phase = tick_phase(self.current_tick)

        # Damage alert — shown prominently if hit since last summary, then cleared
        damage_alert = ""
        if self.recent_damage:
            hits = self.recent_damage
            total = sum(h["dmg"] for h in hits)
            attacker = hits[-1]["source_name"]
            damage_alert = f"\n*** UNDER ATTACK by {attacker}! Took {total} damage recently. HP={self.hp}/{self.max_hp} ***"
            self.recent_damage = []  # clear so it doesn't repeat next turn

        lines = [
            f"Day {self.current_day}, {phase} (tick {self.current_tick}){damage_alert}",
            f"Hero: HP {self.hp}/{self.max_hp}, Stamina {self.stamina}, at ({hx},{hy}), state={self.hero.get('state','?')}",
        ]

        # Inventory
        if self.inventory:
            inv_str = ", ".join(
                f"{i['name']} x{i.get('quantity',1)}" for i in self.inventory[:15]
            )
            lines.append(f"Inventory: {inv_str}")
        else:
            lines.append("Inventory: empty (run get_inventory to refresh)")

        # Nearby objects
        nearby = self.nearby_objects(12)
        if nearby:
            lines.append(f"Nearby objects (within 12 tiles):")
            for o in nearby[:20]:
                tag = ""
                cls = o.get("class", "").lower()
                sub = o.get("subclass", "").lower()
                if sub in HOSTILE_SUBCLASSES or cls == "npc":
                    tag = " [HOSTILE]"
                elif cls == "resource":
                    tag = " [resource]"
                elif cls == "structure":
                    tag = " [structure]"
                elif sub == "villager":
                    tag = " [villager]"
                lines.append(
                    f"  - {o['name']} [id:{o['id']}] at ({o['x']},{o['y']}) dist={o['_dist']}{tag} state={o.get('state','?')}"
                )
        else:
            lines.append("Nearby objects: none visible")

        # Villagers
        vils = self.villagers()
        if vils:
            lines.append(f"Villagers ({len(vils)}):")
            for v in vils:
                lines.append(f"  - {v['name']} [id:{v['id']}] at ({v['x']},{v['y']}) state={v.get('state','?')}")

        # Structures
        structs = self.structures()
        if structs:
            lines.append(f"Structures ({len(structs)}):")
            for s in structs[:8]:
                lines.append(f"  - {s['name']} [id:{s['id']}] at ({s['x']},{s['y']}) state={s.get('state','?')}")

        # Objectives
        obj_parts = []
        for k, v in self.objectives.items():
            label = k.replace("_", " ").title()
            obj_parts.append(f"{label} {'✓' if v else '✗'}")
        lines.append("Objectives: " + ", ".join(obj_parts))

        # Recent events
        if self.events:
            lines.append("Recent notices:")
            for e in self.events[-5:]:
                lines.append(f"  - {e}")

        return "\n".join(lines)
