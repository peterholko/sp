from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / "sailor-sword-edit-grid-transparent.png"
ORIGINAL = Path("sp_frontend/priv/static/art/novicewarrior.png")
CELL = 418
FRAME = 144
BASELINE = 119


def alpha_bbox(image: Image.Image, threshold: int = 24) -> tuple[int, int, int, int]:
    alpha = image.getchannel("A").point(lambda value: 255 if value >= threshold else 0)
    bbox = alpha.getbbox()
    if bbox is None:
        raise ValueError("empty image cell")
    return bbox


def main() -> None:
    source = Image.open(SOURCE).convert("RGBA")
    original = Image.open(ORIGINAL).convert("RGBA")
    output = Image.new("RGBA", (FRAME * 9, FRAME), (0, 0, 0, 0))
    normalized: list[Image.Image] = []

    for index in range(9):
        row, column = divmod(index, 3)
        cell = source.crop((column * CELL, row * CELL, (column + 1) * CELL, (row + 1) * CELL))
        crop = cell.crop(alpha_bbox(cell))
        original_frame = original.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
        original_bbox = alpha_bbox(original_frame)
        target_height = original_bbox[3] - original_bbox[1]
        target_width = max(1, round(crop.width * target_height / crop.height))
        sprite = crop.resize((target_width, target_height), Image.Resampling.NEAREST)
        frame = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
        frame.alpha_composite(sprite, ((FRAME - target_width) // 2, BASELINE - target_height))
        normalized.append(frame)
        output.alpha_composite(frame, (index * FRAME, 0))

    output.save(ROOT / "sailor-sword-generated-normalized-9frame.png")
    print("bboxes:", [frame.getchannel("A").getbbox() for frame in normalized])


if __name__ == "__main__":
    main()
