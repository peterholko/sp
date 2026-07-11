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
LOOT_CLASSES = ("Food", "Drink", "Bedroll", "Log", "Timber", "Hide", "Potion", "Gold", "Ingot")
# Offsets from home to try when placing the Crafting Tent (the spawn tile holds
# the Campfire, so we can't build there).
BUILD_OFFSETS = [(2, 2), (-2, 2), (2, -2), (-2, -2), (3, 0), (0, 3), (-3, 0), (0, -3)]

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
        self.grace = 120.0           # seconds to linger after true death
        self.spawned = False         # hero ever placed in the world
        self._retry_select_at = None  # resend select_class after this ts (no-slot retry)
        self.no_slot_retries = 0
        self._name_attempt = 0       # fallback-name attempts (rustrict false positives)
        self.ws = None
        self._needs_class_select = False
        self.forage = False
        self.build_stockade = False
        self.smart = False           # use HP-recovery / defense / hire behaviors
        self.max_hp = 100
        self.home = None             # defensive anchor (campfire / spawn tile)
        self._hired = False
        self._heal_notes = 0
        # Crafting goal: build a Crafting Tent, then craft + equip a Copper
        # Training Axe (no longer a starting item after the rebalance).
        self.craft_done = False
        self._tent_id = None
        self._cphase = "gather"      # gather->found_wait->stock->build->built_wait
        self._cphase_ts = 0.0        #   ->loadmats->craft_send->craft_wait->retrieve->equip->done
        self._caxe_id = None         # crafted axe item id once seen in the tent
        self._build_spot_idx = 0     # which BUILD_OFFSETS slot to try for the tent
        self._stockade_phase = "none"  # none -> founded_wait -> stocked -> building -> done
        self._stockade_id = None
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
                    self.spawned = True
                    if self.home is None:
                        self.home = self.hero_pos  # campfire spawns on hero tile
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
                    self._track_stockade(o)
                    if (self._tent_id is None and o.get("name") == "Crafting Tent"
                            and o.get("player") == self.player_id):
                        self._tent_id = o["id"]
                        self.note(f"craft: Crafting Tent foundation seen id={o['id']}")
                elif et == "obj_update":
                    oid = ev.get("obj_id")
                    if oid in self.objects:
                        for a in ev.get("attrs", []):
                            self.objects[oid][a["attr"]] = a["value"]
                        if oid == self.hero_id:
                            self.state = self.objects[oid].get("state", self.state)
                        if oid == self._stockade_id:
                            new_state = self.objects[oid].get("state")
                            self.note(f"stockade {oid} state -> {new_state}")
                            if self._stockade_phase == "building" and new_state in ("none", "None"):
                                self._stockade_phase = "done"
                                self.note("STOCKADE COMPLETE — built from shipwreck logs")
                        if oid == self._tent_id and self._cphase == "built_wait":
                            new_state = self.objects[oid].get("state")
                            if new_state in ("none", "None"):
                                self._cphase = "loadmats"
                                self.note("craft: Crafting Tent BUILT")
                elif et == "obj_delete":
                    self.objects.pop(ev.get("obj_id"), None)

        elif ptype == "info_hero":
            self.thirst = pkt.get("thirst", self.thirst)
            self.hunger = pkt.get("hunger", self.hunger)
            self.tired = pkt.get("tiredness", self.tired)
            self.hp = pkt.get("hp", self.hp)
            for k in ("base_hp", "max_hp", "hp_max", "total_hp"):
                if pkt.get(k):
                    self.max_hp = pkt[k]
                    break
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
                # Detect the crafted axe sitting in our Crafting Tent.
                if (src_id == self._tent_id
                        and "Copper Training Axe" in it.get("name", "")):
                    self._caxe_id = it["id"]
                    continue
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

        elif ptype == "Error":
            emsg = pkt.get("errmsg", "")
            self.note(f"ERROR: {emsg}")
            # 5 start locations are shared; a freed slot may not be recycled
            # the instant we connect. Retry select_class until one opens.
            if "start location" in emsg.lower() and not self.spawned:
                self.no_slot_retries += 1
                self._retry_select_at = time.time() + 6.0
            elif "already exists" in emsg.lower() and not self.craft_done:
                # tent build spot is occupied — try a different offset from home
                self._build_spot_idx += 1
                if self._cphase in ("gather", "found_wait"):
                    self._cphase = "gather"
            elif "inappropriate" in emsg.lower() and not self.spawned:
                # rustrict false-positives on alphanumeric names; fall back to a
                # clean dictionary word + player_id (harvest is keyed on player_id).
                self._name_attempt += 1
                fb = ("Hero", "Wanderer", "Settler", "Ranger", "Scout",
                      "Pilgrim", "Drifter", "Nomad")[(self._name_attempt - 1) % 8]
                self.hero_name = f"{fb}{self.player_id}"
                self.note(f"name rejected, retrying as {self.hero_name}")
                self._retry_select_at = time.time() + 0.5

        elif ptype == "build":
            self.note(f"build accepted, build_time={pkt.get('build_time')}")

    def _track_stockade(self, o):
        if (
            self._stockade_id is None
            and o.get("name") == "Stockade"
            and o.get("player") == self.player_id
        ):
            self._stockade_id = o["id"]
            self.note(f"stockade foundation seen id={o['id']} state={o.get('state')}")

    # ------------------------------------------------------------- smart
    def _gold(self):
        return sum(i.get("quantity", 0) for i in self.inventory
                   if "Gold" in i.get("name", ""))

    async def _retreat(self):
        if not self.home:
            return
        await self.send({"cmd": "move_unit", "x": self.home[0], "y": self.home[1]})
        self.note(f"retreating to home {self.home} hp={self.hp}/{self.max_hp}",
                  key="retreat", interval=6)

    async def _try_hire(self):
        # Merchant-borne villagers cost 25 gold (hero starts with 20). Best-effort.
        if self._hired or self._gold() < 25:
            return False
        hx, hy = self.hero_pos
        for o in self.objects.values():
            if o.get("id") == self.hero_id:
                continue
            sub = (o.get("subclass") or "").lower()
            if sub == "villager" and o.get("player") != self.player_id:
                if math.dist((o.get("x", 0), o.get("y", 0)), (hx, hy)) <= 3:
                    await self.send({"cmd": "hire", "source_id": self.hero_id,
                                     "target_id": o["id"]})
                    await self.send({"cmd": "order_follow", "source_id": o["id"]})
                    self.note(f"HIRING villager {o.get('name')} id={o['id']}")
                    self._hired = True
                    return True
        return False

    def _tile_has_structure(self, pos):
        for o in self.objects.values():
            if o.get("id") == self.hero_id:
                continue
            if str(o.get("class", "")).lower() == "structure" \
                    and (o.get("x"), o.get("y")) == (pos[0], pos[1]):
                return True
        return False

    def _inv_count(self, *keys):
        keys = [k.lower() for k in keys]
        total = 0
        for it in self.inventory:
            fields = {str(it.get(f, "")).lower() for f in ("name", "class", "subclass")}
            if any(k in fields for k in keys):
                total += it.get("quantity", 1)
        return total

    async def _transfer_to_tent(self, *subclasses):
        """Move every inventory stack whose class/subclass matches into the tent."""
        subs = [s.lower() for s in subclasses]
        for it in list(self.inventory):
            fields = {str(it.get(f, "")).lower() for f in ("class", "subclass", "name")}
            if any(s in fields for s in subs):
                await self.send({"cmd": "item_transfer", "item": it["id"],
                                 "source_id": self.hero_id, "target_id": self._tent_id})
        await self.send({"cmd": "info_inventory", "id": self.hero_id})

    async def _craft_step(self, now):
        """Build a Crafting Tent, then craft + equip a Copper Training Axe.
        Returns True if it acted; False if it needs the forager to fetch materials."""
        if self.craft_done:
            return False
        logs, hide = self._inv_count("log"), self._inv_count("hide")
        ingot, timber = self._inv_count("copper ingot"), self._inv_count("maple timber")
        self.note(f"craft phase={self._cphase} tent={self._tent_id} "
                  f"log={logs} hide={hide} ingot={ingot} timber={timber}",
                  key="cdbg", interval=15)

        if self._cphase == "gather":
            if logs >= 5 and hide >= 5:
                hx, hy = self.hero_pos
                if self._tile_has_structure((hx, hy)):
                    # standing on the campfire/another structure — step one tile to
                    # an adjacent structure-free tile (always reachable).
                    ring = [(1, 0), (-1, 0), (0, 1), (0, -1),
                            (1, 1), (-1, -1), (1, -1), (-1, 1)]
                    dx, dy = ring[self._build_spot_idx % len(ring)]
                    if not self._tile_has_structure((hx + dx, hy + dy)):
                        await self.send({"cmd": "move_unit", "x": hx + dx, "y": hy + dy})
                        self.note(f"craft: stepping off structure to ({hx+dx},{hy+dy})",
                                  key="cstep", interval=6)
                        return True
                    self._build_spot_idx += 1
                    return True
                await self.send({"cmd": "create_foundation", "source_id": self.hero_id,
                                 "structure": "Crafting Tent"})
                self._cphase, self._cphase_ts = "found_wait", now
                self.note(f"craft: placing Crafting Tent foundation at ({hx},{hy})")
                return True
            return False  # let the forager fetch logs + hide from the shipwreck

        if self._cphase == "found_wait":
            if self._tent_id is not None:
                self._cphase = "stock"
                return await self._craft_step(now)
            if now - self._cphase_ts > 30:
                self._cphase = "gather"
            return True

        if self._cphase == "stock":
            await self._transfer_to_tent("log", "hide")
            self._cphase, self._cphase_ts = "build", now
            self.note("craft: stocked tent with logs + hide")
            return True

        if self._cphase == "build":
            await self.send({"cmd": "build", "source_id": self.hero_id,
                             "structure_id": self._tent_id})
            self._cphase, self._cphase_ts = "built_wait", now
            self.note(f"craft: building Crafting Tent {self._tent_id}")
            return True

        if self._cphase == "built_wait":
            if now - self._cphase_ts > 120:
                self._cphase = "build"  # build may not have started; retry
            return True

        if self._cphase == "loadmats":
            if ingot >= 1 and timber >= 1:
                await self._transfer_to_tent("copper ingot", "maple timber")
                self._cphase, self._cphase_ts = "craft_send", now
                self.note("craft: loaded ingot + timber into tent")
                return True
            return False  # let the forager fetch ingot + timber from the burrow

        if self._cphase == "craft_send":
            await self.send({"cmd": "structure_craft", "structure_id": self._tent_id,
                             "recipe": "Copper Training Axe"})
            self._cphase, self._cphase_ts = "craft_wait", now
            self.note("craft: structure_craft Copper Training Axe")
            return True

        if self._cphase == "craft_wait":
            if self._caxe_id is not None:
                self._cphase = "retrieve"
                return await self._craft_step(now)
            if now - self._cphase_ts > 45:
                self._cphase, self._cphase_ts = "craft_send", now  # craft lost — re-issue
                return True
            await self.send({"cmd": "info_obj", "id": self._tent_id})  # poll for the axe
            return True

        if self._cphase == "retrieve":
            await self.send({"cmd": "item_transfer", "item": self._caxe_id,
                             "source_id": self._tent_id, "target_id": self.hero_id})
            await self.send({"cmd": "info_inventory", "id": self.hero_id})
            self._cphase, self._cphase_ts = "equip", now
            return True

        if self._cphase == "equip":
            axe = next((i for i in self.inventory
                        if "Copper Training Axe" in i.get("name", "")), None)
            if axe:
                await self.send({"cmd": "equip", "obj_id": self.hero_id,
                                 "item": axe["id"], "status": True})
                self.note("craft: EQUIPPED Copper Training Axe — weapon upgrade complete")
                self.craft_done, self._cphase = True, "done"
                return True
            if now - self._cphase_ts > 6:
                self._cphase = "retrieve"  # transfer lagged; retry
            return True

        return False

    async def _smart_step(self, now):
        """HP-recovery + defensive layer on top of the needs loop.
        Returns True if it issued an action (caller should gate last_action)."""
        hp = self.hp if self.hp is not None else self.max_hp
        hpf = hp / max(1, self.max_hp)
        melee = self.hostiles(1.6)
        near = self.hostiles(4.0)
        around = self.hostiles(6.5)

        # Urgent needs first so a long fight can't kill us via thirst/hunger.
        if self.thirst in THIRST_TRIGGER:
            it = self.find_item(names=DRINK_ITEMS, item_class="Water")
            if it:
                await self.send({"cmd": "use", "obj_id": self.hero_id, "item_id": it["id"]})
                return True
        if self.hunger in HUNGER_TRIGGER:
            it = self.find_item(hints=FOOD_HINTS, item_class="Food")
            if it:
                await self.send({"cmd": "use", "obj_id": self.hero_id, "item_id": it["id"]})
                return True

        # Emergency heal with a consumable (baseline bot never does this).
        if hpf < 0.45:
            it = self.find_item(hints=("Health Potion", "Potion", "Bandage",
                                       "Poultice", "Salve", "Tonic"))
            if it:
                await self.send({"cmd": "use", "obj_id": self.hero_id, "item_id": it["id"]})
                self.note(f"healing with {it['name']} (hp={hp}/{self.max_hp})")
                return True

        # Combat: retreat if low and outnumbered, else fight.
        if melee:
            if hpf < 0.35 and len(near) >= 2:
                await self._retreat()
                return True
            tgt = melee[0][1]
            await self.send({"cmd": "attack", "attack_type": "quick",
                             "source_id": self.hero_id, "target_id": tgt["id"]})
            self.note(f"attacking {tgt.get('name')} hp={hp}/{self.max_hp}", key="atk", interval=6)
            return True
        if near and hpf < 0.3:
            await self._retreat()
            return True

        # Urgent HP recovery: sleep to heal when badly hurt and nothing adjacent.
        if hpf < 0.4 and not near and self.state != "Sleeping":
            await self.send({"cmd": "sleep", "structure_id": 0})
            self.note(f"smart sleep (urgent, hp={hp}/{self.max_hp})", key="hpsleep", interval=10)
            return True

        # Progress the crafting goal whenever nothing is in melee range — a hostile
        # lurking at distance shouldn't block it (combat preempts if it closes in).
        # _craft_step returns False when it needs materials, so the forager fetches them.
        if not self.craft_done and await self._craft_step(now):
            return True

        # Otherwise (safe): top up HP/energy, then try to hire.
        if not around:
            if self.state != "Sleeping" and (hpf < 0.6 or self.tired in TIRED_TRIGGER):
                await self.send({"cmd": "sleep", "structure_id": 0})
                self.note(f"smart sleep to recover (hp={hp}/{self.max_hp}, tired={self.tired})",
                          key="hpsleep", interval=10)
                return True
            if await self._try_hire():
                return True
        return False

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
            if self.true_death_ts and time.time() - self.true_death_ts > self.grace:
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

            if self._retry_select_at and time.time() >= self._retry_select_at \
                    and not self.spawned:
                self._retry_select_at = None
                self._needs_class_select = True
                self.note(f"retrying select_class (no-slot retry #{self.no_slot_retries})")

            if self._needs_class_select:
                self._needs_class_select = False
                self.note("selecting Warrior class...")
                await self.send({"cmd": "select_class", "class_name": "Warrior",
                                 "hero_name": self.hero_name})
                continue

            if self.hero_id is None or self.true_death_ts:
                continue

            # Don't act while dead/awaiting resurrection (avoids "dead cannot use
            # items" spam). hp is restored on respawn via the next info_hero poll.
            if self.hp is not None and self.hp <= 0:
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

            # Smart layer: heal / retreat / HP-recovery sleep / hire run first and
            # preempt the legacy combat block. Falls through to forage/build when
            # healthy and safe.
            if self.smart:
                if await self._smart_step(now):
                    last_action = now
                    continue

            # Priority 0 (forage mode): walk to and loot queued food/drink/bedroll.
            # Defer further looting while a stockade build is pending and we
            # already hold logs — the wall is the higher-value errand.
            stockade_pending = (
                self.build_stockade
                and self._stockade_phase != "done"
                and any(i.get("class") == "Log" for i in self.inventory)
            )
            if self.forage and self._loot_queue and not stockade_pending \
                    and not self.hostiles(1.6):
                item_id, src_id, name = self._loot_queue[0]
                src = self.objects.get(src_id)
                if src is None:
                    self._loot_queue.pop(0)
                    continue
                hx, hy = self.hero_pos
                d = math.dist((src.get("x",0), src.get("y",0)), (hx, hy))
                last_action = now
                if d > 2.4:
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

            # Stockade build test: foundation at hero pos -> transfer logs -> build
            if (
                self.build_stockade
                and self._stockade_phase != "done"
                and not self.hostiles(1.6)
            ):
                logs = next(
                    (i for i in self.inventory if i.get("class") == "Log"), None
                )
                if self._stockade_phase == "none":
                    if logs and logs.get("quantity", 1) >= 3:
                        last_action = now
                        await self.send({
                            "cmd": "create_foundation",
                            "source_id": self.hero_id,
                            "structure": "Stockade",
                        })
                        self._stockade_phase = "founded_wait"
                        self.note("requested Stockade foundation")
                        continue
                elif self._stockade_phase == "founded_wait" and self._stockade_id:
                    if logs:
                        last_action = now
                        await self.send({
                            "cmd": "item_transfer",
                            "item": logs["id"],
                            "source_id": self.hero_id,
                            "target_id": self._stockade_id,
                        })
                        await self.send({"cmd": "info_inventory", "id": self.hero_id})
                        self._stockade_phase = "stocked"
                        self.note(
                            f"transferred {logs['name']} x{logs.get('quantity')} "
                            f"into foundation {self._stockade_id}"
                        )
                        continue
                    else:
                        self.note("foundation up but no logs in inventory!", key="nolog")
                elif self._stockade_phase == "stocked":
                    last_action = now
                    await self.send({
                        "cmd": "build",
                        "source_id": self.hero_id,
                        "structure_id": self._stockade_id,
                    })
                    self._stockade_phase = "building"
                    self.note(f"build command sent for stockade {self._stockade_id}")
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
    ap.add_argument("--grace", type=float, default=120.0,
                    help="seconds to linger after true death before exiting")
    ap.add_argument("--forage", action="store_true",
                    help="also loot food/drink/bedroll from Burrow/Shipwreck")
    ap.add_argument("--build-stockade", action="store_true",
                    help="after salvaging logs, build one Stockade segment")
    ap.add_argument("--smart", action="store_true",
                    help="enable HP-recovery (heal/sleep), retreat, salvage, "
                         "stockade, and best-effort hire (implies --forage)")
    args = ap.parse_args()

    session, player_id = fingerprint_auth(args.auth_base, args.fingerprint)
    run = NeedsRun(args.ws_url, session, player_id, args.hero_name, args.log)
    run.smart = args.smart
    run.forage = args.forage or args.smart
    # NOTE: a Warrior cannot melee through its own walls ("Only ranged attacks can
    # be used from behind a wall"), so smart mode does NOT auto-build stockades —
    # they trap a melee hero. Use --build-stockade explicitly to force one.
    run.build_stockade = args.build_stockade
    run.grace = args.grace
    asyncio.run(run.run(args.duration))


if __name__ == "__main__":
    main()
