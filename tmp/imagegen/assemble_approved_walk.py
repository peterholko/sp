from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / "sailor-walk-approved-transparent.png"
ATTACK = ROOT / "sailor-attack-5frame.png"
HEX = ROOT / "sailor-on-144-hex.png"

FRAME_SIZE = 144
BASELINE = 119
TARGET_MAX_HEIGHT = 97


def alpha_bbox(image: Image.Image, threshold: int = 24) -> tuple[int, int, int, int]:
    alpha = image.getchannel("A").point(lambda value: 255 if value >= threshold else 0)
    bbox = alpha.getbbox()
    if bbox is None:
        raise ValueError("frame lane contains no opaque sprite pixels")
    return bbox


def main() -> None:
    source = Image.open(SOURCE).convert("RGBA")
    lane_width = source.width // 4
    crops: list[Image.Image] = []

    for index in range(4):
        left = index * lane_width
        right = source.width if index == 3 else (index + 1) * lane_width
        lane = source.crop((left, 0, right, source.height))
        crops.append(lane.crop(alpha_bbox(lane)))

    # Use one scale for the complete approved sheet so its deliberately smaller
    # passing poses are not enlarged relative to the two contact poses.
    scale = TARGET_MAX_HEIGHT / max(crop.height for crop in crops)
    frames: list[Image.Image] = []
    for crop in crops:
        width = max(1, round(crop.width * scale))
        height = max(1, round(crop.height * scale))
        sprite = crop.resize((width, height), Image.Resampling.NEAREST)
        frame = Image.new("RGBA", (FRAME_SIZE, FRAME_SIZE), (0, 0, 0, 0))
        frame.alpha_composite(sprite, ((FRAME_SIZE - width) // 2, BASELINE - height))
        frames.append(frame)

    walk_sheet = Image.new("RGBA", (FRAME_SIZE * 4, FRAME_SIZE), (0, 0, 0, 0))
    for index, frame in enumerate(frames):
        walk_sheet.alpha_composite(frame, (index * FRAME_SIZE, 0))
    walk_sheet.save(ROOT / "sailor-walk-approved-4frame.png")

    attack = Image.open(ATTACK).convert("RGBA")
    if attack.size != (FRAME_SIZE * 5, FRAME_SIZE):
        raise ValueError(f"unexpected attack sheet dimensions: {attack.size}")
    combined = Image.new("RGBA", (FRAME_SIZE * 9, FRAME_SIZE), (0, 0, 0, 0))
    combined.alpha_composite(attack, (0, 0))
    combined.alpha_composite(walk_sheet, (FRAME_SIZE * 5, 0))
    combined.save(ROOT / "sailor-attack-walk-approved-9frame.png")

    hex_tile = Image.open(HEX).convert("RGBA")
    preview_frames: list[Image.Image] = []
    for frame in frames:
        preview = hex_tile.copy()
        preview.alpha_composite(frame)
        preview_frames.append(preview)
    preview_frames[0].save(
        ROOT / "sailor-walk-approved-on-hex.gif",
        save_all=True,
        append_images=preview_frames[1:],
        duration=180,
        loop=0,
        disposal=2,
    )

    print("source crops:", [crop.size for crop in crops])
    print("frame bboxes:", [frame.getchannel("A").getbbox() for frame in frames])
    print("combined:", combined.size)


if __name__ == "__main__":
    main()
