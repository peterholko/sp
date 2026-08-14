from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parent
ORIGINAL = Path("sp_frontend/priv/static/art/novicewarrior.png")
GENERATED = ROOT / "sailor-sword-generated-normalized-9frame.png"
FRAME = 144


def mask_for(index: int) -> Image.Image:
    mask = Image.new("L", (FRAME, FRAME), 0)
    draw = ImageDraw.Draw(mask)
    if index in (0, 4, 5, 6, 7, 8):
        # Sword, gripping hand, and the old shaft down to the feet. The lower
        # corridor is narrow so the approved walk/body pixels stay untouched.
        draw.rectangle((43, 7, 68, 78), fill=255)
        if index == 0:
            draw.rectangle((49, 75, 61, 126), fill=255)
        else:
            draw.rectangle((49, 75, 61, 91), fill=255)
    elif index == 1:
        draw.polygon([(22, 20), (48, 16), (86, 69), (62, 87)], fill=255)
    elif index == 2:
        draw.rectangle((5, 47, 143, 74), fill=255)
    elif index == 3:
        draw.rectangle((64, 46, 143, 75), fill=255)
    else:
        raise ValueError(index)
    return mask


def main() -> None:
    original = Image.open(ORIGINAL).convert("RGBA")
    generated = Image.open(GENERATED).convert("RGBA")
    output = Image.new("RGBA", original.size, (0, 0, 0, 0))
    masks = []
    for index in range(9):
        box = (index * FRAME, 0, (index + 1) * FRAME, FRAME)
        base = original.crop(box)
        donor = generated.crop(box)
        mask = mask_for(index)
        frame = Image.composite(donor, base, mask)
        output.alpha_composite(frame, (index * FRAME, 0))
        masks.append(mask)

    output.save(ROOT / "sailor-sword-masked-9frame.png")
    mask_sheet = Image.new("L", output.size, 0)
    for index, mask in enumerate(masks):
        mask_sheet.paste(mask, (index * FRAME, 0))
    mask_sheet.save(ROOT / "sailor-sword-edit-mask-9frame.png")

    hex_tile = Image.open(ROOT / "sailor-on-144-hex.png").convert("RGBA")
    for name, indices, duration in (
        ("attack-masked", [0, 1, 2, 3, 4, 0], 140),
        ("walk-masked", [5, 6, 7, 8], 180),
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
