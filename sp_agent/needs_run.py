"""
needs_run.py — Scripted "needs-maintenance only" player for game-state analysis.

Plays the absolute minimum viable survival loop with NO LLM: drink when thirsty,
eat when hungry, sleep when tired (if no hostiles close), fight back any hostile
within melee range. Builds nothing, gathers nothing, recruits nobody.
Purpose: test whether basic needs upkeep alone is enough to reach the day-8
survival director / nightly waves / legendary arc.

Logs every packet to JSONL like passive_run.py.

Usage:
    python needs_run.py --fingerprint needsbot001 --hero-name NeedsBot \
        --log runs/needs.jsonl --duration 5400
"""

import argparse
import asyncio
import json
import math
import ssl
import time

import requests
import urllib3
import websockets

from passive_run import fingerprint_auth

urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)

POLL_INTERVAL = 10.0

THIRST_TRIGGER = {"Slightly Thirsty", "Thirsty", "Parched", "Dehydrated"}
HUNGER_TRIGGER = {"Hungry", "Peckish", "Famished", "Ravenous"}
TIRED_TRIGGER = {"Weary", "Tired", "Exhausted", "Depleted"}
HOSTILE_SUBCLASSES = {"npc", "undead", "demon", "bandit"}
LOOT_SOURCES = ("Burrow", "Shipwreck", "Supply Cache", "Washed Ashore Materials", "Cache")
LOOT_CLASSES = ("Food", "Drink", "Bedroll")

DRINK_ITEMS = ("Waterskin (Filled)", "Waterskin")
FOOD_HINTS = ("Meat", "Berries", "Fish", "Bread", "Stew", "Ration", "Apple", "Egg")


class NeedsRun:
    def __init__(self, ws_url, session, player_id, hero_name, log_path):
        self.ws_url = ws_url
        self.session = session
        self.player_id = player_id
        self.hero_name = hero_name
        self.log_file = open(log_path, "a", buffering=1)
        self.start_ts = time.time()
        self.hero_id = None
        self.hero_pos = (0, 0)
        self.objects = {}
        self.inventory = []
        self.thirst = "Hydrated"
        self.hunger = "Satiated"
        self.tired = "Energized"
        self.hp = None
        self.state = "none"
        self.true_death_ts = None
        self.ws = None
        self._needs_class_select = False
        self.forage = False
        self._loot_queue = []      # (item_id, source_id, name)
        self._looted_ids = set()
        self._storage_poll = 0.0
        self._note_ts = {}

    # ------------------------------------------------------------- helpers
    def log(self, direction, pkt):
        rec = {"t": round(time.time() - self.start_ts, 1), "dir": direction, "pkt": pkt}
        self.log_file.write(json.dumps(rec) + "\n")

    def note(self, msg, key=None, interval=20.0):
        if key is not None:
            now = time.time()
            if now - self._note_ts.get(key, 0) < interval:
                return
            self._note_ts[key] = now
        print(f"[needs t={time.time()-self.start_ts:5.0f}s] {msg}")

    async def send(self, cmd):
        self.log("out", cmd)
        await self.ws.send(json.dumps(cmd))

    def hostiles(self, radius):
        hx, hy = self.hero_pos
        out = []
        for o in self.objects.values():
            if o.get("id") == self.hero_id:
                continue
            sub = (o.get("subclass") or "").lower()
            cls = (o.get("class") or "").lower()
            if sub in HOSTILE_SUBCLASSES or cls == "npc":
                if o.get("state") in ("Dead", "dead"):
                    continue
                d = math.dist((o.get("x", 0), o.get("y", 0)), (hx, hy))
                if d <= radius:
                    out.append((d, o))
        out.sort(key=lambda x: x[0])
        return out

    def find_item(self, names=None, hints=None, item_class=None):
        for it in self.inventory:
            name = it.get("name", "")
            if names and name in names:
                return it
            if hints and any(h.lower() in name.lower() for h in hints):
                return it
            if item_class and it.get("class") == item_class:
                return it
        return None

    # ------------------------------------------------------------- packets
    def handle(self, pkt):
        self.log("in", pkt)
        ptype = pkt.get("packet")

        if ptype == "select_class":
            self._needs_class_select = True

        elif ptype in ("init_perception", "new_perception"):
            data = pkt.get("data", {})
            for obj in data.get("visible_objs", []):
                self.objects[obj["id"]] = obj
            for obs in data.get("observers", []):
                if obs.get("player") == self.player_id:
                    self.hero_id = obs["id"]
                    self.hero_pos = (obs.get("x", 0), obs.get("y", 0))
                    self.objects[obs["id"]] = obs
                    self.note(f"hero id={self.hero_id} at {self.hero_pos}")

        elif ptype == "perception_changes":
            for ev in pkt.get("events", []):
                et = ev.get("event")
                if et in ("obj_create", "obj_move"):
                    o = ev.get("obj", {})
                    self.objects[o["id"]] = o
                    if o["id"] == self.hero_id:
                        self.hero_pos = (o.get("x", 0), o.get("y", 0))
                        self.state = o.get("state", self.state)
                elif et == "obj_update":
                    oid = ev.get("obj_id")
                    if oid in self.objects:
                        for a in ev.get("attrs", []):
                            self.objects[oid][a["attr"]] = a["value"]
                        if oid == self.hero_id:
                            self.state = self.objects[oid].get("state", self.state)
                elif et == "obj_delete":
                    self.objects.pop(ev.get("obj_id"), None)

        elif ptype == "info_hero":
            self.thirst = pkt.get("thirst", self.thirst)
            self.hunger = pkt.get("hunger", self.hunger)
            self.tired = pkt.get("tiredness", self.tired)
            self.hp = pkt.get("hp", self.hp)
            if pkt.get("items") is not None:
                self.inventory = pkt["items"]

        elif ptype == "info_needs_update":
            if pkt.get("id") == self.hero_id:
                self.thirst = pkt.get("thirst", self.thirst)
                self.hunger = pkt.get("hunger", self.hunger)
                self.tired = pkt.get("tiredness", self.tired)

        elif ptype == "info_inventory":
            if pkt.get("id") == self.hero_id:
                self.inventory = pkt.get("items", [])

        elif self.forage and ptype in ("info_structure", "info_poi", "info_npc", "info_obj"):
            src_id = pkt.get("id")
            for it in pkt.get("items") or []:
                if it.get("id") in self._looted_ids:
                    continue
                cls = it.get("class", "")
                if cls in LOOT_CLASSES or "Berries" in it.get("name", ""):
                    self._loot_queue.append((it["id"], src_id, it.get("name", "?")))
                    self._looted_ids.add(it["id"])
                    self.note(f"queued loot: {it.get('name')} x{it.get('quantity',1)} from obj {src_id}")

        elif ptype == "hero_death_state":
            self.note(f"hero_death_state: {pkt.get('phase')} — {pkt.get('message','')}")

        elif ptype == "info_true_death":
            self.note(f"TRUE DEATH: {json.dumps(pkt)[:300]}")
            self.true_death_ts = time.time()

        elif ptype == "Notice":
            self.note(f"NOTICE: {pkt.get('noticemsg','')[:110]}")

    # ------------------------------------------------------------- main
    async def run(self, duration):
        ssl_ctx = ssl.create_default_context()
        ssl_ctx.check_hostname = False
        ssl_ctx.verify_mode = ssl.CERT_NONE
        self.ws = await websockets.connect(
            self.ws_url, ssl=ssl_ctx,
            additional_headers={"Cookie": f"session={self.session}"},
        )
        self.note(f"connected to {self.ws_url}")

        last_poll = 0.0
        last_action = 0.0
        deadline = self.start_ts + duration

        while time.time() < deadline:
            if self.true_death_ts and time.time() - self.true_death_ts > 120:
                self.note("true death + grace elapsed, exiting")
                break
            try:
                raw = await asyncio.wait_for(self.ws.recv(), timeout=0.5)
                self.handle(json.loads(raw))
                continue  # drain packets before acting
            except asyncio.TimeoutError:
                pass
            except (websockets.exceptions.ConnectionClosed, json.JSONDecodeError) as e:
                if isinstance(e, websockets.exceptions.ConnectionClosed):
                    self.note(f"ws closed: {e}")
                    break
                continue

            if self._needs_class_select:
                self._needs_class_select = False
                self.note("selecting Warrior class...")
                await self.send({"cmd": "select_class", "class_name": "Warrior",
                                 "hero_name": self.hero_name})
                continue

            if self.hero_id is None or self.true_death_ts:
                continue

            now = time.time()
            if now - last_poll >= POLL_INTERVAL:
                last_poll = now
                await self.send({"cmd": "info_obj", "id": self.hero_id})
                await self.send({"cmd": "info_inventory", "id": self.hero_id})

            if self.forage and now - self._storage_poll >= 25.0:
                self._storage_poll = now
                hx, hy = self.hero_pos
                for o in self.objects.values():
                    if o.get("name") in LOOT_SOURCES \
                            and math.dist((o.get("x",0), o.get("y",0)), (hx, hy)) <= 6:
                        await self.send({"cmd": "info_obj", "id": o["id"]})

            if now - last_action < 4.0:
                continue

            # Priority 0 (forage mode): walk to and loot queued food/drink/bedroll
            if self.forage and self._loot_queue and not self.hostiles(1.6):
                item_id, src_id, name = self._loot_queue[0]
                src = self.objects.get(src_id)
                if src is None:
                    self._loot_queue.pop(0)
                    continue
                hx, hy = self.hero_pos
                d = math.dist((src.get("x",0), src.get("y",0)), (hx, hy))
                last_action = now
                if d > 1.6:
                    await self.send({"cmd": "move_unit",
                                     "x": src.get("x"), "y": src.get("y")})
                    self.note(f"moving to {src.get('name')} for {name} (d={d:.1f})",
                              key=f"mv{src_id}")
                else:
                    self._loot_queue.pop(0)
                    await self.send({"cmd": "item_transfer", "item": item_id,
                                     "source_id": src_id, "target_id": self.hero_id})
                    self.note(f"looting {name} from {src.get('name')}")
                continue

            # Priority 1: fight back hostiles in melee range
            close = self.hostiles(1.6)
            if close:
                target = close[0][1]
                last_action = now
                await self.send({"cmd": "attack", "attack_type": "quick",
                                 "source_id": self.hero_id, "target_id": target["id"]})
                self.note(f"attacking {target.get('name')} id={target['id']} "
                          f"d={close[0][0]:.1f} hp={self.hp}")
                continue

            # Priority 2: drink
            if self.thirst in THIRST_TRIGGER:
                item = self.find_item(names=DRINK_ITEMS, item_class="Water")
                if item:
                    last_action = now
                    await self.send({"cmd": "use", "obj_id": self.hero_id,
                                     "item_id": item["id"]})
                    self.note(f"drinking {item['name']} (thirst={self.thirst})")
                    continue
                else:
                    self.note(f"THIRSTY ({self.thirst}) but no drink item!", key="nodrink")

            # Priority 3: eat
            if self.hunger in HUNGER_TRIGGER:
                item = self.find_item(hints=FOOD_HINTS, item_class="Food")
                if item:
                    last_action = now
                    await self.send({"cmd": "use", "obj_id": self.hero_id,
                                     "item_id": item["id"]})
                    self.note(f"eating {item['name']} (hunger={self.hunger})")
                    continue
                else:
                    self.note(f"HUNGRY ({self.hunger}) but no food item!", key="nofood")

            # Priority 4: sleep if tired and no hostile adjacent — or emergency
            # sleep regardless once the lethal Exhausted timer is close/active.
            if self.tired in TIRED_TRIGGER and self.state != "Sleeping":
                emergency = self.tired in ("Exhausted", "Depleted")
                if emergency or not self.hostiles(2.0):
                    last_action = now
                    await self.send({"cmd": "sleep", "structure_id": 0})
                    self.note(f"sleeping (tired={self.tired}, emergency={emergency})")
                    continue
                elif now - getattr(self, "_last_tired_note", 0) > 15:
                    self._last_tired_note = now
                    self.note(f"tired ({self.tired}) but hostiles adjacent, waiting")

        self.log_file.close()
        await self.ws.close()
        self.note(f"finished after {time.time()-self.start_ts:.0f}s")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--auth-base", default="https://192.168.1.28:3030")
    ap.add_argument("--ws-url", default="wss://192.168.1.28:8443")
    ap.add_argument("--fingerprint", required=True)
    ap.add_argument("--hero-name", default="NeedsBot")
    ap.add_argument("--log", required=True)
    ap.add_argument("--duration", type=float, default=5400)
    ap.add_argument("--forage", action="store_true",
                    help="also loot food/drink/bedroll from Burrow/Shipwreck")
    args = ap.parse_args()

    session, player_id = fingerprint_auth(args.auth_base, args.fingerprint)
    run = NeedsRun(args.ws_url, session, player_id, args.hero_name, args.log)
    run.forage = args.forage
    asyncio.run(run.run(args.duration))


if __name__ == "__main__":
    main()
