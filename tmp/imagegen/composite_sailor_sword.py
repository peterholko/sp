from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parent
ORIGINAL = Path("sp_frontend/priv/static/art/novicewarrior.png")
GENERATED = ROOT / "sailor-sword-generated-normalized-9frame.png"
FRAME = 144


def polygons_for_frame(index: int) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
    # Removal mask for the original spear, then a slightly broader donor mask
    # that includes the replacement sword and gripping hand.
    if index in (0, 4, 5, 6, 7, 8):
        remove = [(45, 10), (65, 10), (65, 122), (45, 122)]
        donor = [(43, 8), (69, 8), (69, 78), (43, 78)]
    elif index == 1:
        remove = [(30, 28), (50, 20), (85, 71), (69, 82)]
        donor = [(32, 26), (53, 20), (80, 71), (62, 78)]
    elif index == 2:
        remove = [(9, 51), (139, 51), (139, 72), (9, 72)]
        donor = [(67, 49), (139, 49), (139, 72), (67, 72)]
    elif index == 3:
        remove = [(62, 50), (143, 50), (143, 73), (62, 73)]
        donor = [(69, 48), (143, 48), (143, 73), (69, 73)]
    else:
        raise ValueError(index)
    return remove, donor


def polygon_mask(points: list[tuple[int, int]]) -> Image.Image:
    mask = Image.new("L", (FRAME, FRAME), 0)
    ImageDraw.Draw(mask).polygon(points, fill=255)
    return mask


def main() -> None:
    original = Image.open(ORIGINAL).convert("RGBA")
    generated = Image.open(GENERATED).convert("RGBA")
    output = Image.new("RGBA", original.size, (0, 0, 0, 0))

    for index in range(9):
        box = (index * FRAME, 0, (index + 1) * FRAME, FRAME)
        base = original.crop(box)
        donor = generated.crop(box)
        remove_points, donor_points = polygons_for_frame(index)

        remove_mask = polygon_mask(remove_points)
        cleared = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
        base.paste(cleared, (0, 0), remove_mask)

        donor_mask = polygon_mask(donor_points)
        donor_mask = Image.composite(donor.getchannel("A"), Image.new("L", (FRAME, FRAME), 0), donor_mask)
        base.alpha_composite(Image.composite(donor, Image.new("RGBA", (FRAME, FRAME)), donor_mask))
        output.alpha_composite(base, (index * FRAME, 0))

    output.save(ROOT / "sailor-sword-composite-9frame.png")

    # Map previews for the idle, attack, and walk animations.
    hex_tile = Image.open(ROOT / "sailor-on-144-hex.png").convert("RGBA")
    for name, indices, duration in (
        ("idle", [0], 300),
        ("attack", [0, 1, 2, 3, 4, 0], 140),
        ("walk", [5, 6, 7, 8], 180),
    ):
        frames = []
        for index in indices:
            preview = hex_tile.copy()
            preview.alpha_composite(output.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME)))
            frames.append(preview)
        frames[0].save(
            ROOT / f"sailor-sword-{name}-on-hex.gif",
            save_all=True,
            append_images=frames[1:],
            duration=duration,
            loop=0,
            disposal=2,
        )


if __name__ == "__main__":
    main()
