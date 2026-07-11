"""
batch_run.py — Run the scripted needs/forage bot across N games, pipelined at
`--concurrency` wide (the server has only 5 start locations, recycled on True
Death), and record each run's outcome.

Each game is a fresh fingerprint account that plays needs_run.py --forage until
True Death (then a short grace) or a hard time cap. True Death writes the run to
the postgres `scores` table — the authoritative results sink — so the heavy
aggregation is done from there afterwards. This driver additionally captures
per-game wall-clock survival, whether the bot reached True Death, and the no-slot
retry count, written live to a status file and a final summary JSON.

Usage:
    python -u batch_run.py --total 50 --concurrency 5 --cap 1800 --tag batchA \
        --outdir runs/batch_batchA
"""

import argparse
import asyncio
import json
import os
import re
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))


async def run_one(idx, args, sem, results, lock):
    fp = f"b50{args.tag}{idx:03d}"          # alphanumeric, >=8 chars
    hero = f"B50{args.tag}{idx:03d}"
    log = os.path.join(args.outdir, f"bot_{idx:03d}.jsonl")
    cmd = [
        sys.executable, "-u", os.path.join(HERE, "needs_run.py"),
        "--fingerprint", fp, "--hero-name", hero,
        "--grace", str(args.grace),
        "--duration", str(args.cap), "--log", log,
        "--auth-base", args.auth_base, "--ws-url", args.ws_url,
        "--smart" if args.smart else "--forage",
    ]
    async with sem:
        start = time.time()
        proc = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
        )
        out, _ = await proc.communicate()
        dur = time.time() - start

    text = out.decode("utf-8", "replace")
    true_death = "TRUE DEATH" in text
    spawned = "hero id=" in text
    retries = text.count("no-slot retry")
    name_retries = text.count("name rejected")
    m = re.search(r"player_id=(\d+)", text)
    pid = int(m.group(1)) if m else None
    rec = {
        "idx": idx, "hero": hero, "fp": fp, "player_id": pid, "dur": round(dur, 1),
        "true_death": true_death, "spawned": spawned,
        "no_slot_retries": retries, "name_retries": name_retries,
        "rc": proc.returncode,
    }
    async with lock:
        results.append(rec)
        done = len(results)
        with open(args.status, "a") as f:
            f.write(json.dumps({**rec, "done": done, "total": args.total,
                                "ts": round(time.time(), 1)}) + "\n")
    print(f"[{done}/{args.total}] {hero} dur={dur:.0f}s "
          f"true_death={true_death} spawned={spawned} retries={retries}",
          flush=True)


async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--total", type=int, default=50)
    ap.add_argument("--concurrency", type=int, default=5)
    ap.add_argument("--cap", type=float, default=1800,
                    help="per-game hard cap seconds (bot exits even if alive)")
    ap.add_argument("--grace", type=float, default=8.0)
    ap.add_argument("--smart", action="store_true",
                    help="run the smart bot (heal/retreat/build/hire) instead of plain forage")
    ap.add_argument("--tag", required=True, help="alphanumeric batch tag")
    ap.add_argument("--outdir", required=True)
    ap.add_argument("--auth-base", default="https://192.168.1.28:3030")
    ap.add_argument("--ws-url", default="wss://192.168.1.28:8443")
    args = ap.parse_args()

    assert args.tag.isalnum(), "tag must be alphanumeric"
    os.makedirs(args.outdir, exist_ok=True)
    args.status = os.path.join(args.outdir, "status.jsonl")
    args.summary = os.path.join(args.outdir, "summary.json")

    print(f"batch_run: total={args.total} concurrency={args.concurrency} "
          f"cap={args.cap}s tag={args.tag} outdir={args.outdir}", flush=True)

    sem = asyncio.Semaphore(args.concurrency)
    lock = asyncio.Lock()
    results = []
    t0 = time.time()
    await asyncio.gather(*[
        run_one(i, args, sem, results, lock)
        for i in range(1, args.total + 1)
    ])
    wall = time.time() - t0

    results.sort(key=lambda r: r["idx"])
    n_death = sum(1 for r in results if r["true_death"])
    n_spawn = sum(1 for r in results if r["spawned"])
    n_capped = sum(1 for r in results if r["spawned"] and not r["true_death"])
    n_noslot = sum(1 for r in results if not r["spawned"])
    player_ids = sorted(r["player_id"] for r in results if r.get("player_id"))
    summary = {
        "tag": args.tag, "total": args.total, "concurrency": args.concurrency,
        "cap": args.cap, "wall_seconds": round(wall, 1),
        "reached_true_death": n_death, "spawned": n_spawn,
        "capped_alive": n_capped, "never_spawned": n_noslot,
        "hero_like": f"B50{args.tag}%", "player_ids": player_ids, "runs": results,
    }
    with open(args.summary, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nDONE in {wall:.0f}s — true_death={n_death}/{args.total} "
          f"spawned={n_spawn} capped_alive={n_capped} never_spawned={n_noslot}",
          flush=True)
    print(f"summary: {args.summary}", flush=True)
    print(f"scores query: hero_name LIKE 'B50{args.tag}%'", flush=True)


if __name__ == "__main__":
    import urllib3
    urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)
    asyncio.run(main())
