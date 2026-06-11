"""
passive_run.py — Passive/neglectful player harness for game-state analysis.

Creates (or reuses) a passwordless account via /fingerprint-auth, selects a class,
then does NOTHING except observe: every server packet is appended to a JSONL log
with a wall-clock timestamp, and the hero's needs (hunger/thirst/hp) are polled
every POLL_INTERVAL seconds via info_obj. Runs until true death + grace period,
or until --duration seconds elapse.

Usage:
    python passive_run.py --fingerprint passive-bot-001 --hero-name PassiveBot \
        --log runs/passive.jsonl --duration 5400
"""

import argparse
import asyncio
import json
import ssl
import time

import requests
import urllib3
import websockets

urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)

POLL_INTERVAL = 20.0


def fingerprint_auth(base_url: str, fingerprint: str) -> tuple[str, int]:
    """POST /fingerprint-auth, return (session_cookie, player_id)."""
    resp = requests.post(
        base_url.rstrip("/") + "/fingerprint-auth",
        json={"fingerprint": fingerprint, "device_token": None},
        verify=False,
        timeout=10,
    )
    resp.raise_for_status()
    data = resp.json()
    player_id = data.get("playerId", data.get("player_id"))

    session = None
    raw_cookie = resp.headers.get("Set-Cookie", "")
    for part in raw_cookie.split(";"):
        part = part.strip()
        if part.startswith("session="):
            session = part.split("=", 1)[1]
            break
    if session is None:
        raise RuntimeError(f"No session cookie in fingerprint-auth response: {data}")
    print(f"[auth] fingerprint auth ok player_id={player_id} new_player={data.get('newPlayer')}")
    return session, player_id


class PassiveRun:
    def __init__(self, ws_url: str, session: str, player_id: int, hero_name: str, log_path: str):
        self.ws_url = ws_url
        self.session = session
        self.player_id = player_id
        self.hero_name = hero_name
        self.log_path = log_path
        self.log_file = open(log_path, "a", buffering=1)
        self.start_ts = time.time()
        self.hero_id: int | None = None
        self.dead = False
        self.true_death_ts: float | None = None
        self.ws = None

    def log(self, direction: str, pkt: dict):
        rec = {"t": round(time.time() - self.start_ts, 1), "dir": direction, "pkt": pkt}
        self.log_file.write(json.dumps(rec) + "\n")

    async def send(self, cmd: dict):
        self.log("out", cmd)
        await self.ws.send(json.dumps(cmd))

    def handle(self, pkt: dict):
        ptype = pkt.get("packet")
        self.log("in", pkt)

        if ptype == "select_class":
            # handled in run loop (needs await); flag it
            self._needs_class_select = True

        elif ptype in ("init_perception", "new_perception"):
            data = pkt.get("data", {})
            for obs in data.get("observers", []):
                if obs.get("player") == self.player_id:
                    self.hero_id = obs["id"]
                    print(f"[run] hero id={self.hero_id} at ({obs.get('x')},{obs.get('y')})")

        elif ptype == "hero_death_state":
            print(f"[run] t={time.time()-self.start_ts:.0f}s hero_death_state: {pkt.get('phase')} — {pkt.get('message','')}")

        elif ptype == "info_true_death":
            print(f"[run] t={time.time()-self.start_ts:.0f}s TRUE DEATH: {json.dumps(pkt)[:300]}")
            self.true_death_ts = time.time()

        elif ptype == "dmg":
            if pkt.get("target_id") == self.hero_id and pkt.get("state") == "Dead":
                self.dead = True
                print(f"[run] t={time.time()-self.start_ts:.0f}s hero died (dmg packet)")

    async def run(self, duration: float):
        ssl_ctx = ssl.create_default_context()
        ssl_ctx.check_hostname = False
        ssl_ctx.verify_mode = ssl.CERT_NONE
        self._needs_class_select = False

        self.ws = await websockets.connect(
            self.ws_url, ssl=ssl_ctx,
            additional_headers={"Cookie": f"session={self.session}"},
        )
        print(f"[ws] connected to {self.ws_url}")

        last_poll = 0.0
        deadline = self.start_ts + duration

        while time.time() < deadline:
            # End 120s after true death so we capture trailing packets/score.
            if self.true_death_ts and time.time() - self.true_death_ts > 120:
                print("[run] true death + grace elapsed, exiting")
                break
            try:
                raw = await asyncio.wait_for(self.ws.recv(), timeout=1.0)
            except asyncio.TimeoutError:
                raw = None
            except websockets.exceptions.ConnectionClosed as e:
                print(f"[ws] closed: {e}")
                break

            if raw is not None:
                try:
                    pkt = json.loads(raw)
                except json.JSONDecodeError:
                    continue
                self.handle(pkt)

            if self._needs_class_select:
                self._needs_class_select = False
                print("[run] selecting Warrior class...")
                await self.send({"cmd": "select_class", "class_name": "Warrior", "hero_name": self.hero_name})

            now = time.time()
            if self.hero_id is not None and now - last_poll >= POLL_INTERVAL:
                last_poll = now
                await self.send({"cmd": "info_obj", "id": self.hero_id})

        self.log_file.close()
        await self.ws.close()
        print(f"[run] finished after {time.time()-self.start_ts:.0f}s, log: {self.log_path}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--auth-base", default="https://192.168.1.28:3030")
    ap.add_argument("--ws-url", default="wss://192.168.1.28:8443")
    ap.add_argument("--fingerprint", required=True)
    ap.add_argument("--hero-name", default="PassiveBot")
    ap.add_argument("--log", required=True)
    ap.add_argument("--duration", type=float, default=5400)
    args = ap.parse_args()

    session, player_id = fingerprint_auth(args.auth_base, args.fingerprint)
    run = PassiveRun(args.ws_url, session, player_id, args.hero_name, args.log)
    asyncio.run(run.run(args.duration))


if __name__ == "__main__":
    main()
