from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parent
ORIGINAL = Path("sp_frontend/priv/static/art/novicewarrior.png")
FRAME = 144

SKIN_ORANGE = (218, 91, 11, 255)
SKIN_LIGHT = (255, 153, 23, 255)
SKIN_DARK = (121, 45, 5, 255)
OUTLINE = (20, 15, 11, 255)
BLADE_DARK = (104, 111, 111, 255)
BLADE_MID = (205, 209, 201, 255)
BLADE_LIGHT = (247, 241, 220, 255)
BRONZE_DARK = (105, 45, 6, 255)
BRONZE = (197, 88, 8, 255)
BRONZE_LIGHT = (247, 145, 20, 255)


def draw_vertical_sword(frame: Image.Image, hand_x: int, hand_y: int) -> None:
    draw = ImageDraw.Draw(frame)
    # Hand/pommel: repaint the grip area after erasing the spear shaft.
    draw.rectangle((hand_x - 3, hand_y - 2, hand_x + 3, hand_y + 6), fill=OUTLINE)
    draw.rectangle((hand_x - 2, hand_y - 1, hand_x + 2, hand_y + 5), fill=BRONZE_DARK)
    draw.point((hand_x, hand_y), fill=BRONZE_LIGHT)
    draw.rectangle((hand_x - 5, hand_y - 4, hand_x + 5, hand_y - 2), fill=OUTLINE)
    draw.rectangle((hand_x - 4, hand_y - 3, hand_x + 4, hand_y - 3), fill=BRONZE_LIGHT)

    blade_bottom = hand_y - 5
    blade_top = hand_y - 34
    draw.polygon(
        [(hand_x, blade_top - 5), (hand_x + 4, blade_top), (hand_x + 4, blade_bottom),
         (hand_x - 4, blade_bottom), (hand_x - 4, blade_top)],
        fill=OUTLINE,
    )
    draw.polygon(
        [(hand_x, blade_top - 3), (hand_x + 2, blade_top + 1), (hand_x + 2, blade_bottom - 1),
         (hand_x - 2, blade_bottom - 1), (hand_x - 2, blade_top + 1)],
        fill=BLADE_MID,
    )
    draw.line((hand_x, blade_top - 2, hand_x, blade_bottom - 2), fill=BLADE_LIGHT, width=1)
    draw.line((hand_x + 2, blade_top + 1, hand_x + 2, blade_bottom - 1), fill=BLADE_DARK, width=1)


def clear_vertical_spear(frame: Image.Image, x: int, hand_y: int) -> None:
    pixels = frame.load()
    # Everything above the hand is weapon-only in these frames.
    for y in range(0, hand_y - 3):
        for px in range(max(0, x - 6), min(FRAME, x + 7)):
            pixels[px, y] = (0, 0, 0, 0)
    # Below the hand, clear only the narrow wooden shaft. Preserve boots/body.
    wood_predicate = lambda p: p[3] > 0 and p[0] > 65 and p[0] > p[1] * 1.2 and p[1] > p[2] * 1.4
    for y in range(hand_y + 6, FRAME):
        for px in range(max(0, x - 3), min(FRAME, x + 4)):
            if wood_predicate(pixels[px, y]):
                pixels[px, y] = (0, 0, 0, 0)


def clear_attack_weapon(frame: Image.Image, polygon: list[tuple[int, int]]) -> None:
    mask = Image.new("L", (FRAME, FRAME), 0)
    ImageDraw.Draw(mask).polygon(polygon, fill=255)
    frame.paste(Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0)), (0, 0), mask)


def clear_weapon_line(frame: Image.Image, polygon: list[tuple[int, int]]) -> None:
    mask = Image.new("L", (FRAME, FRAME), 0)
    ImageDraw.Draw(mask).polygon(polygon, fill=255)
    pixels = frame.load()
    mask_pixels = mask.load()
    # Remove the wooden shaft and metallic spearhead but avoid skin/clothing.
    for y in range(FRAME):
        for x in range(FRAME):
            if not mask_pixels[x, y]:
                continue
            p = pixels[x, y]
            is_wood = p[3] > 0 and p[0] > 60 and p[0] > p[1] * 1.12 and p[1] > p[2] * 1.25
            is_metal = p[3] > 0 and p[0] > 125 and abs(p[0] - p[1]) < 70 and p[2] > 70
            if is_wood or is_metal:
                pixels[x, y] = (0, 0, 0, 0)


def draw_sword_between(frame: Image.Image, pommel: tuple[int, int], tip: tuple[int, int]) -> None:
    draw = ImageDraw.Draw(frame)
    px, py = pommel
    tx, ty = tip
    dx, dy = tx - px, ty - py
    length = max(abs(dx), abs(dy))
    ux, uy = dx / length, dy / length
    nx, ny = -uy, ux

    guard_center = (px + ux * 8, py + uy * 8)
    blade_start = (guard_center[0] + ux * 3, guard_center[1] + uy * 3)
    # Grip, pommel, and crossguard.
    draw.line((px, py, guard_center[0], guard_center[1]), fill=OUTLINE, width=5)
    draw.line((px, py, guard_center[0], guard_center[1]), fill=BRONZE_DARK, width=3)
    draw.ellipse((px - 2, py - 2, px + 2, py + 2), fill=BRONZE_LIGHT, outline=OUTLINE)
    gx1, gy1 = guard_center[0] + nx * 6, guard_center[1] + ny * 6
    gx2, gy2 = guard_center[0] - nx * 6, guard_center[1] - ny * 6
    draw.line((gx1, gy1, gx2, gy2), fill=OUTLINE, width=5)
    draw.line((gx1, gy1, gx2, gy2), fill=BRONZE_LIGHT, width=2)

    # Tapered blade with dark outline and light center ridge.
    bx, by = blade_start
    points = [
        (round(bx + nx * 4), round(by + ny * 4)),
        (round(tx), round(ty)),
        (round(bx - nx * 4), round(by - ny * 4)),
    ]
    draw.polygon(points, fill=OUTLINE)
    inset_points = [
        (round(bx + nx * 2), round(by + ny * 2)),
        (round(tx - ux * 2), round(ty - uy * 2)),
        (round(bx - nx * 2), round(by - ny * 2)),
    ]
    draw.polygon(inset_points, fill=BLADE_MID)
    draw.line((round(bx), round(by), round(tx - ux * 2), round(ty - uy * 2)), fill=BLADE_LIGHT, width=1)


def main() -> None:
    original = Image.open(ORIGINAL).convert("RGBA")
    generated = Image.open(ROOT / "sailor-sword-generated-normalized-9frame.png").convert("RGBA")
    output = Image.new("RGBA", original.size, (0, 0, 0, 0))
    upright = {
        0: (54, 61),
        4: (54, 60),
        5: (55, 62),
        6: (55, 61),
        7: (55, 62),
        8: (55, 61),
    }

    for index in range(9):
        frame = original.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
        if index in upright:
            x, hand_y = upright[index]
            clear_vertical_spear(frame, x, hand_y)
            draw_vertical_sword(frame, x, hand_y)
        elif index == 1:
            clear_attack_weapon(frame, [(24, 24), (47, 18), (82, 69), (61, 84)])
            draw_sword_between(frame, (69, 72), (35, 29))
        elif index == 2:
            # The shaft crosses both hands and torso here, so use the clean
            # image-edited attack frame rather than cutting holes through them.
            frame = generated.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
        elif index == 3:
            frame = generated.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
        output.alpha_composite(frame, (index * FRAME, 0))

    output.save(ROOT / "sailor-sword-exact-9frame.png")

    hex_tile = Image.open(ROOT / "sailor-on-144-hex.png").convert("RGBA")
    for name, indices, duration in (
        ("attack-exact", [0, 1, 2, 3, 4, 0], 140),
        ("walk-exact", [5, 6, 7, 8], 180),
    ):
        previews = []
        for index in indices:
            preview = hex_tile.copy()
            preview.alpha_composite(output.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME)))
            previews.append(preview)
        previews[0].save(
            ROOT / f"sailor-sword-{name}-on-hex.gif",
            save_all=True,
            append_images=previews[1:],
            duration=duration,
            loop=0,
            disposal=2,
        )


if __name__ == "__main__":
    main()
