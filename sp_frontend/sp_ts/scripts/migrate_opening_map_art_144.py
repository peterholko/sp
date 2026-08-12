#!/usr/bin/env python3
"""Nearest-neighbour migration for the bounded opening-game map-art set."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from PIL import Image


REPO_ROOT = Path(__file__).resolve().parents[3]
AXUM_STATIC = REPO_ROOT / "sp_axum/root/static"
ART_ROOTS = (
    AXUM_STATIC / "art",
    REPO_ROOT / "sp_frontend/priv/static/art",
)
DEFINITION_ROOTS = (
    REPO_ROOT / "sp_server/tileset",
    AXUM_STATIC / "art",
    REPO_ROOT / "sp_frontend/priv/static/art",
)
TILESET_MANIFESTS = (
    AXUM_STATIC / "tileset.json",
    REPO_ROOT / "sp_frontend/priv/static/tileset.json",
)

TARGET_DEFINITIONS = (
    "novicewarrior",
    "noviceranger",
    "novicemage",
    "giantrat",
    "humanvillager",
    "giantcrab",
    "wildboar",
    "spider",
    "cavebat",
    "thornbeetle",
    "ashviper",
    "mossmite",
    "reefskitter",
    "meagermerchant",
    "necromancer",
    "zombie",
    "sailor",
    "shipwreck",
    "campfire",
    "campfirelit",
    "burrow",
    "lumbercamp",
    "craftingtent",
    "tent",
    "tentlit",
    "monolith",
    "mausoleum",
    "burnedhouse",
    "graveyard",
    "sealedcavern",
    "mine-abandoned",
    "foundation",
    "stockade",
)

# Only full map sprites/sheets are included. The separate *_single.png files
# are React UI previews and intentionally remain outside this map-art pass.
MAP_ART_SOURCE_SIZES = {
    "novicewarrior.png": (648, 72),
    "noviceranger.png": (1728, 72),
    "novicemage.png": (432, 72),
    "giantrat.png": (936, 72),
    "humanvillager.png": (72, 72),
    "humanvillager1.png": (864, 72),
    "humanvillager2.png": (864, 72),
    "humanvillager3.png": (864, 72),
    "humanvillager4.png": (864, 72),
    "giantcrab.png": (72, 72),
    "wildboar.png": (72, 72),
    "spider.png": (72, 72),
    "cavebat.png": (720, 72),
    "thornbeetle.png": (72, 72),
    "ashviper.png": (72, 72),
    "mossmite.png": (72, 72),
    "reefskitter.png": (72, 72),
    "meagermerchant.png": (72, 72),
    "necromancer.png": (504, 72),
    "zombie.png": (432, 72),
    "sailor.png": (648, 72),
    "shipwreck.png": (72, 72),
    "campfire.png": (72, 72),
    "campfirelit.png": (72, 72),
    "burrow.png": (72, 72),
    "lumbercamp.png": (72, 72),
    "craftingtent.png": (72, 72),
    "tent.png": (72, 72),
    "tentlit.png": (72, 72),
    "monolith.png": (72, 72),
    "mausoleum.png": (72, 72),
    "burnedhouse.png": (72, 72),
    "graveyard.png": (72, 72),
    "sealedcavern.png": (72, 72),
    "mine-abandoned.png": (72, 72),
    "foundation.png": (72, 72),
    "gravestone.png": (72, 72),
    "regular-concave-bl.png": (126, 180),
    "regular-concave-br.png": (126, 144),
    "regular-concave-l.png": (126, 144),
    "regular-concave-r.png": (126, 180),
    "regular-concave-tl.png": (126, 180),
    "regular-concave-tr.png": (126, 144),
}

OPENING_TILE_SOURCE_SIZES = {
    "tileset/grass/green.png": (72, 72),
    "tileset/frozen/snow.png": (72, 72),
    "tileset/water/coast-grey-tile.png": (72, 72),
    "tileset/water/coast-tile.png": (72, 72),
    "tileset/water/ocean-A01.png": (72, 72),
    "tileset/grass/dry.png": (72, 72),
    "tileset/hills/dry.png": (72, 72),
    "tileset/hills/dry2.png": (72, 72),
    "tileset/grass/semi-dry.png": (72, 72),
    "tileset/sand/desert.png": (72, 72),
    "tileset/sand/desert-oasis.png": (72, 72),
    "tileset/hills/desert.png": (72, 72),
    "tileset/hills/regular.png": (72, 72),
    "tileset/swamp/water.png": (72, 72),
    "tileset/swamp/water-plant.png": (72, 72),
    "tileset/hills/snow.png": (72, 72),
    "tileset/water/reef.png": (72, 72),
    "tileset/swamp/reed.png": (120, 112),
    "tileset/forest/deciduous-summer4.png": (144, 144),
    "tileset/forest/tropical/rainforest.png": (144, 144),
    "tileset/forest/tropical/jungle.png": (144, 144),
    "tileset/forest/tropical/savanna.png": (144, 144),
    "tileset/forest/deciduous-winter.png": (144, 144),
    "tileset/forest/deciduous-winter-snow.png": (144, 144),
    "tileset/forest/mixed-winter-snow.png": (144, 144),
    "tileset/forest/pine.png": (144, 144),
    "tileset/forest/snow-forest.png": (144, 144),
    "tileset/forest/snow-forest-sparse4.png": (144, 144),
    "tileset/forest/tropical/savanna-small.png": (144, 144),
    "tileset/forest/tropical/palm-desert.png": (144, 144),
    "tileset/mountains/basic.png": (180, 216),
    "tileset/mountains/snow.png": (180, 216),
    "tileset/mountains/basic2.png": (180, 216),
    "tileset/mountains/basic3.png": (180, 216),
    "tileset/mountains/dry.png": (180, 216),
    "tileset/mountains/snow2.png": (180, 216),
    "tileset/mountains/snow3.png": (180, 216),
    "tileset/mountains/volcano.png": (180, 216),
    "burnedhouse.png": (72, 72),
    "mine-abandoned.png": (72, 72),
    "monolith.png": (72, 72),
    "sealedcavern.png": (72, 72),
    "mausoleum.png": (72, 72),
}

LEGACY_STOCKADE_FRAMES = [
    [0, 0, 126, 144, 0, 0, 72],
    [0, 0, 126, 180, 1, 54, 108],
    [0, 0, 126, 180, 2, 54, 108],
    [0, 0, 126, 144, 3, 0, 72],
    [0, 0, 126, 180, 4, 54, 108],
    [0, 0, 126, 144, 5, 0, 72],
    [0, 0, 126, 144, 6, 54, 36],
    [0, 0, 126, 180, 7, 0, 72],
    [0, 0, 126, 144, 8, 0, 0],
    [0, 0, 126, 180, 9, 54, 36],
]


def doubled_stockade_frames() -> list[list[int]]:
    frames = [frame.copy() for frame in LEGACY_STOCKADE_FRAMES]
    for frame in frames:
        for index in (0, 1, 2, 3, 5, 6):
            frame[index] *= 2
    return frames


def doubled(size: tuple[int, int]) -> tuple[int, int]:
    return size[0] * 2, size[1] * 2


def resize_nearest(path: Path, legacy_size: tuple[int, int], apply: bool) -> str:
    with Image.open(path) as source:
        current_size = source.size
        target_size = doubled(legacy_size)
        if current_size == target_size:
            return "ready"
        if current_size != legacy_size:
            raise ValueError(
                f"{path}: expected {legacy_size} or {target_size}, found {current_size}"
            )
        if not apply:
            raise ValueError(f"{path}: still uses legacy dimensions {current_size}")

        converted = source.resize(target_size, Image.Resampling.NEAREST)
        converted.save(path, format="PNG", optimize=False)
        return "resized"


def write_json(path: Path, data: Any) -> None:
    path.write_text(json.dumps(data, indent=4) + "\n")


def migrate_definition(path: Path, apply: bool) -> str:
    data = json.loads(path.read_text())
    frames = data.get("frames")
    scale = data.get("map_scale")

    native_frames = (
        isinstance(frames, dict)
        and (frames.get("width"), frames.get("height")) == (144, 144)
    ) or (
        path.stem == "stockade"
        and frames == doubled_stockade_frames()
    )

    if native_frames:
        if scale == 1:
            return "ready"
        if scale != 0.5:
            raise ValueError(f"{path}: native definition has unexpected map_scale {scale!r}")
        if not apply:
            raise ValueError(f"{path}: native definition still uses half-scale map art")

        data["map_scale"] = 1
        write_json(path, data)
        return "updated"

    if isinstance(frames, dict):
        if (frames.get("width"), frames.get("height")) != (72, 72):
            raise ValueError(f"{path}: expected 72x72 legacy frames, found {frames}")
        if scale not in (None, 1):
            raise ValueError(f"{path}: legacy definition has unexpected map_scale {scale!r}")
        if not apply:
            raise ValueError(f"{path}: still uses legacy 72x72 frames")
        frames["width"] = 144
        frames["height"] = 144
    elif path.stem == "stockade" and isinstance(frames, list):
        if frames != LEGACY_STOCKADE_FRAMES:
            raise ValueError(f"{path}: legacy stockade frames are misaligned")
        if not apply:
            raise ValueError(f"{path}: still uses legacy stockade frames")
        data["frames"] = doubled_stockade_frames()
    else:
        raise ValueError(f"{path}: unsupported frame definition")

    data["map_scale"] = 1
    write_json(path, data)
    return "updated"


def doubled_offset(value: Any) -> Any:
    parsed = float(value)
    converted = parsed * 2
    if converted.is_integer():
        converted = int(converted)
    return str(converted) if isinstance(value, str) else converted


def migrate_tileset_manifest(path: Path, apply: bool) -> str:
    data = json.loads(path.read_text())
    target_entries = [entry for entry in data if 1 <= int(entry["tile"]) <= 44]
    if len(target_entries) != 44:
        raise ValueError(f"{path}: expected 44 opening-map tile definitions")

    changed = False
    for entry in target_entries:
        if entry.get("map_scale") == 1:
            continue
        if entry.get("map_scale") not in (None, 0.5):
            raise ValueError(f"{path}: tile {entry['tile']} has unexpected map_scale")
        if not apply:
            raise ValueError(f"{path}: tile {entry['tile']} is not at native map scale")
        entry["offsetx"] = doubled_offset(entry.get("offsetx", 0))
        entry["offsety"] = doubled_offset(entry.get("offsety", 0))
        entry["map_scale"] = 1
        changed = True

    if changed:
        write_json(path, data)
        return "updated"
    return "ready"


def run(apply: bool) -> None:
    results: dict[str, int] = {}

    def record(result: str) -> None:
        results[result] = results.get(result, 0) + 1

    for relative_path, legacy_size in MAP_ART_SOURCE_SIZES.items():
        matches = [root / relative_path for root in ART_ROOTS if (root / relative_path).is_file()]
        if not matches:
            raise FileNotFoundError(f"No copy of map art {relative_path} exists")
        for path in matches:
            record(resize_nearest(path, legacy_size, apply))

    for relative_path, legacy_size in OPENING_TILE_SOURCE_SIZES.items():
        matches = [root / relative_path for root in ART_ROOTS if (root / relative_path).is_file()]
        if not matches:
            raise FileNotFoundError(f"No copy of tile art {relative_path} exists")
        for path in matches:
            # POI files overlap MAP_ART_SOURCE_SIZES and may already be converted.
            record(resize_nearest(path, legacy_size, apply))

    for definition_name in TARGET_DEFINITIONS:
        matches = [
            root / f"{definition_name}.json"
            for root in DEFINITION_ROOTS
            if (root / f"{definition_name}.json").is_file()
        ]
        if not matches:
            raise FileNotFoundError(f"No definition for {definition_name} exists")
        for path in matches:
            record(migrate_definition(path, apply))

    for path in TILESET_MANIFESTS:
        record(migrate_tileset_manifest(path, apply))

    mode = "migration" if apply else "validation"
    summary = ", ".join(f"{key}={value}" for key, value in sorted(results.items()))
    print(f"Opening map-art {mode} complete: {summary}")


def main() -> None:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--apply", action="store_true", help="perform the conversion")
    mode.add_argument("--check", action="store_true", help="validate the converted set")
    args = parser.parse_args()
    run(apply=args.apply)


if __name__ == "__main__":
    main()
