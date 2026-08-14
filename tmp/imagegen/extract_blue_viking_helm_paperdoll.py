from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parent
GENERATED_GRID = ROOT / "sailor-blue-viking-helm-grid-transparent.png"
# Keep normalization tied to the approved unhelmeted rig. The live
# novicewarrior file is intentionally swapped between experiments, so using it
# here made repeated extraction depend on whichever test sprite was active.
BASE_SHEET = ROOT / "sailor-sword-generated-normalized-9frame.png"
FRAME = 144
CELL = 418
BASELINE = 119


def alpha_bbox(image: Image.Image, threshold: int = 24) -> tuple[int, int, int, int]:
    alpha = image.getchannel("A").point(lambda value: 255 if value >= threshold else 0)
    bbox = alpha.getbbox()
    if bbox is None:
        raise ValueError("empty image")
    return bbox


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

    full_character_sheet = Image.new("RGBA", (FRAME * 9, FRAME), (0, 0, 0, 0))
    for index, frame in enumerate(normalized_generated):
        full_character_sheet.alpha_composite(frame, (index * FRAME, 0))
    full_character_sheet.save(ROOT / "sailor-blue-viking-helm-full-9frame.png")

    # Wider than the Copper Helm so each source region includes both horns.
    # Destination anchors place the equipment over the fixed rig's head.
    head_regions = [
        ((51, 20, 101, 60), (51, 20)),
        ((57, 32, 108, 70), (57, 32)),
        ((48, 33, 99, 71), (57, 33)),
        ((34, 38, 85, 76), (54, 38)),
        ((50, 19, 100, 59), (51, 19)),
        ((50, 21, 101, 61), (51, 21)),
        ((51, 22, 102, 62), (51, 22)),
        ((50, 21, 101, 61), (51, 21)),
        ((51, 22, 102, 62), (51, 22)),
    ]

    helmet_sheet = Image.new("RGBA", (FRAME * 9, FRAME), (0, 0, 0, 0))
    occlusion_sheet = Image.new("L", (FRAME * 9, FRAME), 0)
    composite_sheet = Image.new("RGBA", (FRAME * 9, FRAME), (0, 0, 0, 0))

    for index, (generated_frame, head_data) in enumerate(zip(normalized_generated, head_regions)):
        region, destination = head_data
        x1, y1, x2, y2 = region
        dest_x, dest_y = destination
        width, height = x2 - x1, y2 - y1

        helmet = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
        helmet.alpha_composite(generated_frame.crop(region), destination)

        # Retain only the helmet/head island within the crop. The generated
        # reference places the upright sword close to the left horn and some
        # attack frames place a shoulder under the cap; both must remain part
        # of the base sheet, not the equipment layer.
        helmet_pixels = helmet.load()
        keep_bottom = dest_y + round(height * 0.78)
        center_x = dest_x + width / 2
        for py in range(FRAME):
            for px in range(FRAME):
                if helmet_pixels[px, py][3] == 0:
                    continue
                outside_vertical = py > keep_bottom
                far_left_weapon = px < dest_x + round(width * 0.13)
                if outside_vertical or far_left_weapon:
                    helmet_pixels[px, py] = (0, 0, 0, 0)
        helmet_sheet.alpha_composite(helmet, (index * FRAME, 0))

        # Horns remain visible, but only the central cap hides the base hair.
        mask = Image.new("L", (FRAME, FRAME), 0)
        draw = ImageDraw.Draw(mask)
        cap_left = dest_x + round(width * 0.22)
        cap_right = dest_x + round(width * 0.78)
        cap_top = dest_y + round(height * 0.13)
        cap_bottom = dest_y + round(height * 0.80)
        draw.ellipse((cap_left, cap_top, cap_right, cap_bottom), fill=255)
        draw.rectangle((cap_left + 2, cap_bottom - 5, cap_right - 2, cap_bottom + 3), fill=255)
        occlusion_sheet.paste(mask, (index * FRAME, 0))

        base_frame = base.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
        base_frame.paste(Image.new("RGBA", (FRAME, FRAME)), (0, 0), mask)
        base_frame.alpha_composite(helmet)
        composite_sheet.alpha_composite(base_frame, (index * FRAME, 0))

    helmet_sheet.save(ROOT / "sailor-blue-viking-helm-paperdoll-9frame.png")
    occlusion_sheet.save(ROOT / "sailor-blue-viking-helm-occlusion-9frame.png")
    composite_sheet.save(ROOT / "sailor-blue-viking-helm-composite-9frame.png")

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
            ROOT / f"sailor-blue-viking-helm-{name}-on-hex.gif",
            save_all=True,
            append_images=frames[1:],
            duration=duration,
            loop=0,
            disposal=2,
        )

        full_frames = []
        for index in indices:
            preview = hex_tile.copy()
            preview.alpha_composite(
                full_character_sheet.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
            )
            full_frames.append(preview)
        full_frames[0].save(
            ROOT / f"sailor-blue-viking-helm-full-{name}-on-hex.gif",
            save_all=True,
            append_images=full_frames[1:],
            duration=duration,
            loop=0,
            disposal=2,
        )

    print("helmet bbox:", helmet_sheet.getchannel("A").getbbox())
    print("mask bbox:", occlusion_sheet.getbbox())


if __name__ == "__main__":
    main()
