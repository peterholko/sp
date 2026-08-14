from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parent
GENERATED_GRID = ROOT / "sailor-copper-helm-grid-transparent.png"
BASE_SHEET = Path("sp_frontend/priv/static/art/novicewarrior.png")
FRAME = 144
CELL = 418
BASELINE = 119


def alpha_bbox(image: Image.Image, threshold: int = 24) -> tuple[int, int, int, int]:
    alpha = image.getchannel("A").point(lambda value: 255 if value >= threshold else 0)
    bbox = alpha.getbbox()
    if bbox is None:
        raise ValueError("empty image")
    return bbox


def connected_components(alpha: Image.Image, threshold: int = 24):
    pixels = alpha.load()
    width, height = alpha.size
    seen = set()
    components = []
    for y in range(height):
        for x in range(width):
            if (x, y) in seen or pixels[x, y] < threshold:
                continue
            stack = [(x, y)]
            seen.add((x, y))
            points = []
            while stack:
                px, py = stack.pop()
                points.append((px, py))
                for nx, ny in ((px - 1, py), (px + 1, py), (px, py - 1), (px, py + 1)):
                    if 0 <= nx < width and 0 <= ny < height and (nx, ny) not in seen and pixels[nx, ny] >= threshold:
                        seen.add((nx, ny))
                        stack.append((nx, ny))
            xs = [point[0] for point in points]
            ys = [point[1] for point in points]
            components.append((len(points), (min(xs), min(ys), max(xs) + 1, max(ys) + 1)))
    return sorted(components, reverse=True)


def main() -> None:
    generated = Image.open(GENERATED_GRID).convert("RGBA")
    base = Image.open(BASE_SHEET).convert("RGBA")
    normalized_generated = []

    for index in range(9):
        row, column = divmod(index, 3)
        cell = generated.crop((column * CELL, row * CELL, (column + 1) * CELL, (row + 1) * CELL))
        crop = cell.crop(alpha_bbox(cell))
        base_frame = base.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
        base_box = alpha_bbox(base_frame)
        target_height = base_box[3] - base_box[1]
        target_width = max(1, round(crop.width * target_height / crop.height))
        sprite = crop.resize((target_width, target_height), Image.Resampling.NEAREST)
        frame = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
        frame.alpha_composite(sprite, ((FRAME - target_width) // 2, BASELINE - target_height))
        normalized_generated.append(frame)

    normalized_sheet = Image.new("RGBA", (FRAME * 9, FRAME), (0, 0, 0, 0))
    for index, frame in enumerate(normalized_generated):
        normalized_sheet.alpha_composite(frame, (index * FRAME, 0))
    normalized_sheet.save(ROOT / "sailor-copper-helm-generated-normalized-9frame.png")

    # Generated-head region and destination anchor for each fixed rig frame.
    # Separating source and destination is what makes the paper doll robust to
    # slight recentering in a generated edit while leaving the body unchanged.
    head_regions = [
        ((59, 25, 94, 59), (59, 25)),
        ((67, 37, 100, 69), (67, 37)),
        ((58, 38, 91, 70), (67, 38)),
        ((44, 43, 77, 75), (64, 43)),
        ((58, 24, 93, 58), (59, 24)),
        ((58, 26, 94, 60), (59, 26)),
        ((59, 27, 95, 61), (59, 27)),
        ((58, 26, 94, 60), (59, 26)),
        ((59, 27, 95, 61), (59, 27)),
    ]

    helmet_sheet = Image.new("RGBA", (FRAME * 9, FRAME), (0, 0, 0, 0))
    occlusion_sheet = Image.new("L", (FRAME * 9, FRAME), 0)
    composite_sheet = Image.new("RGBA", (FRAME * 9, FRAME), (0, 0, 0, 0))

    for index, (generated_frame, head_data) in enumerate(zip(normalized_generated, head_regions)):
        region, destination = head_data
        x1, y1, x2, y2 = region
        dest_x, dest_y = destination
        helmet = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
        helmet.alpha_composite(generated_frame.crop(region), (dest_x, dest_y))

        # The helmet layer is restricted to the head window; alpha inside that
        # window supplies its exact silhouette. The occlusion mask erases the
        # upper crown of the base hair before the equipment is composited.
        helmet_sheet.alpha_composite(helmet, (index * FRAME, 0))
        mask = Image.new("L", (FRAME, FRAME), 0)
        draw = ImageDraw.Draw(mask)
        width = x2 - x1
        height = y2 - y1
        crown_bottom = dest_y + round(height * 0.72)
        draw.ellipse((dest_x + 2, dest_y, dest_x + width - 2, crown_bottom + 6), fill=255)
        draw.rectangle((dest_x + 5, crown_bottom - 2, dest_x + width - 5, crown_bottom + 5), fill=255)
        occlusion_sheet.paste(mask, (index * FRAME, 0))

        base_frame = base.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
        base_frame.paste(Image.new("RGBA", (FRAME, FRAME)), (0, 0), mask)
        base_frame.alpha_composite(helmet)
        composite_sheet.alpha_composite(base_frame, (index * FRAME, 0))

    helmet_sheet.save(ROOT / "sailor-copper-helm-paperdoll-9frame.png")
    occlusion_sheet.save(ROOT / "sailor-copper-helm-occlusion-9frame.png")
    composite_sheet.save(ROOT / "sailor-copper-helm-composite-9frame.png")

    hex_tile = Image.open(ROOT / "sailor-on-144-hex.png").convert("RGBA")
    for name, indices, duration in (
        ("attack", [0, 1, 2, 3, 4, 0], 140),
        ("walk", [5, 6, 7, 8], 180),
    ):
        frames = []
        for index in indices:
            preview = hex_tile.copy()
            preview.alpha_composite(composite_sheet.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME)))
            frames.append(preview)
        frames[0].save(
            ROOT / f"sailor-copper-helm-{name}-on-hex.gif",
            save_all=True,
            append_images=frames[1:],
            duration=duration,
            loop=0,
            disposal=2,
        )

    print("helmet components:", connected_components(helmet_sheet.getchannel("A"))[:15])
    print("helmet bbox:", helmet_sheet.getchannel("A").getbbox())


if __name__ == "__main__":
    main()
