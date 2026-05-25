"""
tools.py — Claude tool definitions and execution for Siege Perilous.
"""

from game_state import GameState
from game_client import GameClient

# ------------------------------------------------------------------ #
# Tool schema definitions (passed to Anthropic API)
# ------------------------------------------------------------------ #

TOOL_DEFINITIONS = [
    {
        "name": "get_inventory",
        "description": "Request the hero's current inventory from the server. Always call this at the start and after picking up or using items.",
        "input_schema": {
            "type": "object",
            "properties": {},
            "required": [],
        },
    },
    {
        "name": "move",
        "description": "Move the hero to a tile at coordinates (x, y). The hero will pathfind automatically. Use for navigating to resources, structures, or enemies.",
        "input_schema": {
            "type": "object",
            "properties": {
                "x": {"type": "integer", "description": "Target X coordinate"},
                "y": {"type": "integer", "description": "Target Y coordinate"},
            },
            "required": ["x", "y"],
        },
    },
    {
        "name": "attack",
        "description": "Attack a target. Use attack_type 'melee' for standard attack. source_id is the hero's id, target_id is the enemy id.",
        "input_schema": {
            "type": "object",
            "properties": {
                "target_id": {"type": "integer", "description": "ID of the target to attack"},
                "attack_type": {
                    "type": "string",
                    "description": "Attack type: 'melee', 'quick', or 'precise'",
                    "enum": ["melee", "quick", "precise"],
                },
            },
            "required": ["target_id", "attack_type"],
        },
    },
    {
        "name": "gather",
        "description": "Begin gathering resources. The hero must be adjacent to or on top of a resource tile. No arguments needed — just be in position first.",
        "input_schema": {
            "type": "object",
            "properties": {},
            "required": [],
        },
    },
    {
        "name": "pick_up_item",
        "description": "Transfer an item from a nearby object (chest, structure, ground) into the hero's inventory.",
        "input_schema": {
            "type": "object",
            "properties": {
                "item_id": {"type": "integer", "description": "ID of the item to pick up"},
                "source_id": {"type": "integer", "description": "ID of the object that holds the item"},
            },
            "required": ["item_id", "source_id"],
        },
    },
    {
        "name": "use_item",
        "description": "Use an item from the hero's inventory (e.g., Health Potion to heal).",
        "input_schema": {
            "type": "object",
            "properties": {
                "item_id": {"type": "integer", "description": "ID of the item to use"},
            },
            "required": ["item_id"],
        },
    },
    {
        "name": "create_foundation",
        "description": "Place a structure foundation at the given coordinates. Must have the required materials in inventory. Common structures: 'Campfire', 'Shelter', 'Storage Box', 'Crafting Tent'.",
        "input_schema": {
            "type": "object",
            "properties": {
                "structure_name": {"type": "string", "description": "Name of the structure to create"},
                "x": {"type": "integer", "description": "X coordinate to place it"},
                "y": {"type": "integer", "description": "Y coordinate to place it"},
            },
            "required": ["structure_name", "x", "y"],
        },
    },
    {
        "name": "build",
        "description": "Build (complete construction of) a foundation. The hero must be adjacent to the structure. source_id is the hero id, structure_id is the foundation object id.",
        "input_schema": {
            "type": "object",
            "properties": {
                "structure_id": {"type": "integer", "description": "ID of the foundation to build"},
            },
            "required": ["structure_id"],
        },
    },
    {
        "name": "craft",
        "description": "Craft an item using a recipe. The hero must have the required materials. Example recipes: 'Campfire', 'Torch', 'Wood Plank'.",
        "input_schema": {
            "type": "object",
            "properties": {
                "recipe": {"type": "string", "description": "Name of the recipe to craft"},
            },
            "required": ["recipe"],
        },
    },
    {
        "name": "order_follow",
        "description": "Order a villager to follow the hero.",
        "input_schema": {
            "type": "object",
            "properties": {
                "villager_id": {"type": "integer", "description": "ID of the villager"},
            },
            "required": ["villager_id"],
        },
    },
    {
        "name": "order_gather",
        "description": "Order a villager to gather a specific resource type.",
        "input_schema": {
            "type": "object",
            "properties": {
                "villager_id": {"type": "integer", "description": "ID of the villager"},
                "res_type": {"type": "string", "description": "Resource type: 'Wood', 'Stone', 'Food', etc."},
            },
            "required": ["villager_id", "res_type"],
        },
    },
    {
        "name": "explore_poi",
        "description": "Investigate a Point of Interest (burned house, graveyard, etc.) for information, loot, or events.",
        "input_schema": {
            "type": "object",
            "properties": {
                "poi_id": {"type": "integer", "description": "ID of the POI object"},
            },
            "required": ["poi_id"],
        },
    },
    {
        "name": "info_obj",
        "description": "Get information about any object by its ID. Use this to learn about unknown objects.",
        "input_schema": {
            "type": "object",
            "properties": {
                "obj_id": {"type": "integer", "description": "ID of the object"},
            },
            "required": ["obj_id"],
        },
    },
    {
        "name": "wait",
        "description": "Wait and collect perception updates for a number of seconds. Use when hero is busy (gathering, building, crafting) or to observe what happens next.",
        "input_schema": {
            "type": "object",
            "properties": {
                "seconds": {"type": "number", "description": "How many seconds to wait (0.5 to 10)"},
            },
            "required": ["seconds"],
        },
    },
    {
        "name": "get_state_summary",
        "description": "Return the current game state as a text summary without sending any commands. Use this to refresh your understanding of the situation.",
        "input_schema": {
            "type": "object",
            "properties": {},
            "required": [],
        },
    },
    {
        "name": "transfer_item_to_structure",
        "description": (
            "Transfer an item from the hero's inventory into a structure (e.g., a foundation that needs construction materials). "
            "Use this in Step 3 of building: after create_foundation, transfer ALL required items into the foundation before calling build. "
            "Example: Campfire foundation needs 1x Stick and 1x Resin — transfer each one with this tool using the item id from get_inventory and the foundation id from nearby objects."
        ),
        "input_schema": {
            "type": "object",
            "properties": {
                "item_id": {"type": "integer", "description": "ID of the item in the hero's inventory to transfer"},
                "structure_id": {"type": "integer", "description": "ID of the foundation/structure to transfer the item into"},
            },
            "required": ["item_id", "structure_id"],
        },
    },
    {
        "name": "hire_villager",
        "description": "Hire a villager from a nearby hireable unit. source_id is the hero id, target_id is the hireable villager id.",
        "input_schema": {
            "type": "object",
            "properties": {
                "target_id": {"type": "integer", "description": "ID of the villager to hire"},
            },
            "required": ["target_id"],
        },
    },
]


# ------------------------------------------------------------------ #
# Tool execution
# ------------------------------------------------------------------ #

async def execute_tool(tool_name: str, tool_input: dict, client: GameClient, state: GameState) -> str:
    """Execute a tool call and return a result string."""
    hero_id = state.hero_id()

    if tool_name == "get_inventory":
        await client.send({"cmd": "info_inventory", "id": hero_id})
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        items = state.inventory
        if items:
            return "Inventory: " + ", ".join(f"{i['name']} x{i.get('quantity',1)} [id:{i['id']}]" for i in items)
        return "Inventory is empty."

    elif tool_name == "move":
        x, y = tool_input["x"], tool_input["y"]
        await client.send({"cmd": "move_unit", "x": x, "y": y})
        pkts = await client.recv_all(duration=2.0)
        for p in pkts:
            state.process(p)
        hx, hy = state.hero_pos()
        return f"Move command sent. Hero now at ({hx},{hy})."

    elif tool_name == "attack":
        target_id = tool_input["target_id"]
        attack_type = tool_input.get("attack_type", "melee")
        await client.send({
            "cmd": "attack",
            "attack_type": attack_type,
            "source_id": hero_id,
            "target_id": target_id,
        })
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        return f"Attacked target {target_id} with {attack_type}. HP={state.hp}/{state.max_hp}."

    elif tool_name == "gather":
        await client.send({"cmd": "gather"})
        pkts = await client.recv_all(duration=1.0)
        for p in pkts:
            state.process(p)
        return "Gather command sent."

    elif tool_name == "pick_up_item":
        item_id = tool_input["item_id"]
        source_id = tool_input["source_id"]
        await client.send({
            "cmd": "item_transfer",
            "item": item_id,
            "source_id": source_id,
            "target_id": hero_id,
        })
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        return f"Item transfer requested: item {item_id} from {source_id}."

    elif tool_name == "use_item":
        item_id = tool_input["item_id"]
        await client.send({"cmd": "use", "obj_id": hero_id, "item_id": item_id})
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        return f"Used item {item_id}. HP now {state.hp}/{state.max_hp}."

    elif tool_name == "create_foundation":
        name = tool_input["structure_name"]
        x, y = tool_input["x"], tool_input["y"]
        await client.send({"cmd": "create_foundation", "source_id": hero_id, "structure": name})
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        return f"Foundation creation requested for '{name}'."

    elif tool_name == "build":
        structure_id = tool_input["structure_id"]
        await client.send({"cmd": "build", "source_id": hero_id, "structure_id": structure_id})
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        return f"Build command sent for structure {structure_id}."

    elif tool_name == "craft":
        recipe = tool_input["recipe"]
        await client.send({"cmd": "craft", "recipe": recipe})
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        return f"Crafting '{recipe}'."

    elif tool_name == "order_follow":
        villager_id = tool_input["villager_id"]
        await client.send({"cmd": "order_follow", "source_id": villager_id})
        pkts = await client.recv_all(duration=1.0)
        for p in pkts:
            state.process(p)
        return f"Villager {villager_id} ordered to follow."

    elif tool_name == "order_gather":
        villager_id = tool_input["villager_id"]
        res_type = tool_input["res_type"]
        await client.send({"cmd": "order_gather", "source_id": villager_id, "res_type": res_type})
        pkts = await client.recv_all(duration=1.0)
        for p in pkts:
            state.process(p)
        return f"Villager {villager_id} ordered to gather {res_type}."

    elif tool_name == "explore_poi":
        poi_id = tool_input["poi_id"]
        await client.send({"cmd": "investigate", "target_id": poi_id})
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        return f"Investigating POI {poi_id}."

    elif tool_name == "info_obj":
        obj_id = tool_input["obj_id"]
        await client.send({"cmd": "info_obj", "id": obj_id})
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        # Return info packet contents if any
        for p in pkts:
            if p.get("packet", "").startswith("info_"):
                return str(p)
        return f"Info requested for object {obj_id}."

    elif tool_name == "transfer_item_to_structure":
        item_id = tool_input["item_id"]
        structure_id = tool_input["structure_id"]
        await client.send({
            "cmd": "item_transfer",
            "item": item_id,
            "source_id": hero_id,
            "target_id": structure_id,
        })
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        return f"Transferred item {item_id} from hero to structure {structure_id}."

    elif tool_name == "hire_villager":
        target_id = tool_input["target_id"]
        await client.send({"cmd": "hire", "source_id": hero_id, "target_id": target_id})
        pkts = await client.recv_all(duration=1.5)
        for p in pkts:
            state.process(p)
        return f"Hire command sent for {target_id}."

    elif tool_name == "wait":
        seconds = min(max(float(tool_input.get("seconds", 1.0)), 0.5), 10.0)
        pkts = await client.recv_all(duration=seconds)
        for p in pkts:
            state.process(p)
        return f"Waited {seconds:.1f}s, received {len(pkts)} packets. " + state.summary()

    elif tool_name == "get_state_summary":
        return state.summary()

    else:
        return f"Unknown tool: {tool_name}"
