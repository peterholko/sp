"""
competent_run.py — Competent-player harness for game-state analysis.

Runs the existing LLM agent (agent.py) unmodified except:
  - MAX_TURNS raised so a full run can reach death/win
  - every packet processed by GameState is also appended to a timestamped JSONL

Usage:
    AUTH_URL=https://192.168.1.28:3030/auth WS_URL=wss://192.168.1.28:8443 \
        python -u competent_run.py --log runs/competent.jsonl --max-turns 600
"""

import argparse
import asyncio
import json
import time

ap = argparse.ArgumentParser()
ap.add_argument("--log", required=True)
ap.add_argument("--max-turns", type=int, default=600)
args = ap.parse_args()

import agent
from game_state import GameState

agent.MAX_TURNS = args.max_turns

_log_file = open(args.log, "a", buffering=1)
_start_ts = time.time()
_orig_process = GameState.process


def _logged_process(self, pkt):
    try:
        _log_file.write(json.dumps({"t": round(time.time() - _start_ts, 1), "pkt": pkt}) + "\n")
    except Exception:
        pass
    return _orig_process(self, pkt)


GameState.process = _logged_process

if __name__ == "__main__":
    import urllib3
    urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)
    asyncio.run(agent.main())
