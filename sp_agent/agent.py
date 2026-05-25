"""
agent.py — AI agent that plays Siege Perilous via the backend WebSocket API.

Usage:
    pip install -r requirements.txt
    # Edit .env with your credentials
    python agent.py
"""

import asyncio
import os
import sys
import json
import anthropic
from dotenv import load_dotenv

from game_client import GameClient
from game_state import GameState
from tools import TOOL_DEFINITIONS, execute_tool

load_dotenv()

AUTH_URL = os.environ["AUTH_URL"]
WS_URL = os.environ["WS_URL"]
GAME_ACCOUNT = os.environ["GAME_ACCOUNT"]
GAME_PASSWORD = os.environ["GAME_PASSWORD"]
ANTHROPIC_API_KEY = os.environ["ANTHROPIC_API_KEY"]

SYSTEM_PROMPT = """You are an AI agent playing Siege Perilous, a multiplayer turn-based survival game.

Your goals (in priority order):
1. SURVIVAL: Keep HP above 50%. Use Health Potions immediately if HP drops below 40%.
   Retreat from hostiles if HP is low. Campfires provide warmth and safety.
   IMPORTANT: When you see "UNDER ATTACK" in the state summary, respond immediately — attack
   the enemy or flee (move away) before doing anything else.
2. GATHER RESOURCES: Collect Wood, Stone, Food, and other materials.
3. BUILD STRUCTURES: Place foundations and build them. Start with Campfire → Shelter → Storage Box.
4. RECRUIT VILLAGERS: Hire nearby villagers to help gather and build.
5. EXPLORE POIs: Investigate Points of Interest (burned house, graveyard, etc.) for loot.
6. COMPLETE OBJECTIVES: Work through the displayed objectives list.

HOW TO BUILD A STRUCTURE (exact sequence — do not skip steps):
Step 1. Call get_inventory to confirm you have the required materials.
        Campfire requires: 1x Stick, 1x Resin.
        Shelter requires: Wood and Stone. Storage Box requires Wood and Stone.
Step 2. Call create_foundation with the structure name and a nearby tile coordinate.
        Wait for the foundation object to appear in nearby objects (it will show as a structure
        with state "foundation"). Note its id.
Step 3. Transfer the required construction items from hero to the foundation using pick_up_item
        in reverse — use item_transfer tool with source_id=hero_id, target_id=foundation_id.
        You must transfer ALL required items before building.
Step 4. Call build with structure_id = the foundation's id.
        Wait a few seconds — the hero will work and the structure will complete.
        Call wait(5) to let construction finish.

If you see a foundation in nearby objects but haven't transferred items yet, do that now.
If you have already transferred items to the foundation, send the build command.
Never call create_foundation again if a foundation for that structure already exists nearby.

Tactical notes:
- Always call get_inventory at the start so you know what you have.
- Move to a resource tile before gathering.
- Build foundations near the campfire / central camp area.
- When a hostile NPC is nearby and your HP is good, attack it; otherwise flee.
- After moving, use wait(2) to collect perception updates before deciding next action.
- You cannot craft without materials — check inventory first.
- Villagers must be hired before they can be ordered.

Each turn you will receive a state summary. Choose 1-3 tool calls that make progress toward your goals.
Do not repeat the same failed action. If something isn't working, try a different approach.
"""

MAX_TURNS = 200      # safety limit
DECISION_INTERVAL = 2.0  # seconds between decision cycles


async def collect_initial_state(client: GameClient, state: GameState):
    """Wait for init_perception, handling class selection if this is a new hero."""
    print("[agent] waiting for init_perception...")
    deadline = asyncio.get_event_loop().time() + 30.0
    while not state._initialized:
        pkt = await client.recv(timeout=5.0)
        if pkt is None:
            if asyncio.get_event_loop().time() > deadline:
                raise RuntimeError("Timed out waiting for init_perception")
            continue

        packet_type = pkt.get("packet")

        # New account needs to select a hero class before the game starts
        if packet_type == "select_class":
            print("[agent] new hero — selecting Warrior class...")
            await client.send({"cmd": "select_class", "class_name": "Warrior", "hero_name": "Agent"})
            # Wait for the server to spawn the hero and send init_perception
            await asyncio.sleep(2.0)
            continue

        state.process(pkt)
        if state._initialized:
            hx, hy = state.hero_pos()
            print(f"[agent] init_perception received — hero spawned at ({hx}, {hy})")

    # Drain a few more seconds of initial packets (stats, world, objectives)
    pkts = await client.recv_all(duration=2.0)
    for p in pkts:
        state.process(p)

    # Explicitly request inventory
    await client.send({"cmd": "info_inventory", "id": state.hero_id()})
    pkts = await client.recv_all(duration=1.5)
    for p in pkts:
        state.process(p)

    hx, hy = state.hero_pos()
    print(f"[agent] initialized. Hero '{state.hero.get('name', '?')}' id={state.hero_id()} spawned at ({hx}, {hy})")
    print(state.summary())


async def run_decision_loop(client: GameClient, state: GameState):
    """Main AI decision loop."""
    anth = anthropic.Anthropic(api_key=ANTHROPIC_API_KEY)
    conversation: list[dict] = []

    for turn in range(MAX_TURNS):
        # Collect any pending perception updates before deciding
        pkts = await client.recv_all(duration=0.3)
        for p in pkts:
            state.process(p)

        # Build the current state message
        summary = state.summary()
        print(f"\n[turn {turn+1}] {summary[:200]}...")

        # Append current state to conversation (keep last 6 exchanges to limit tokens)
        conversation.append({"role": "user", "content": summary})
        if len(conversation) > 12:
            conversation = conversation[-12:]

        # Ask Claude what to do
        try:
            response = anth.messages.create(
                model="claude-opus-4-6",
                max_tokens=1024,
                system=SYSTEM_PROMPT,
                messages=conversation,
                tools=TOOL_DEFINITIONS,
            )
        except Exception as e:
            print(f"[agent] Anthropic API error: {e}")
            await asyncio.sleep(5.0)
            continue

        # Collect tool calls from response
        tool_calls = [b for b in response.content if b.type == "tool_use"]
        text_blocks = [b for b in response.content if b.type == "text"]

        if text_blocks:
            for t in text_blocks:
                print(f"[claude] {t.text[:200]}")

        if not tool_calls:
            print("[agent] no tool calls — waiting")
            await asyncio.sleep(DECISION_INTERVAL)
            continue

        # Execute each tool call
        tool_results = []
        for tc in tool_calls:
            print(f"[tool] {tc.name}({json.dumps(tc.input)})")
            result = await execute_tool(tc.name, tc.input, client, state)
            print(f"[result] {result[:200]}")
            tool_results.append({
                "type": "tool_result",
                "tool_use_id": tc.id,
                "content": result,
            })

        # Append assistant turn + tool results to conversation
        conversation.append({"role": "assistant", "content": response.content})
        conversation.append({"role": "user", "content": tool_results})

        await asyncio.sleep(DECISION_INTERVAL)

    print("[agent] reached max turns, exiting")


async def main():
    client = GameClient(
        auth_url=AUTH_URL,
        ws_url=WS_URL,
        account=GAME_ACCOUNT,
        password=GAME_PASSWORD,
    )

    print(f"[agent] authenticating as '{GAME_ACCOUNT}'...")
    client.authenticate()

    await client.connect()

    state = GameState(player_id=client.player_id)

    try:
        await collect_initial_state(client, state)
        await run_decision_loop(client, state)
    except KeyboardInterrupt:
        print("[agent] interrupted by user")
    finally:
        await client.close()


if __name__ == "__main__":
    # Suppress the InsecureRequestWarning from requests (self-signed cert)
    import urllib3
    urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)

    asyncio.run(main())
