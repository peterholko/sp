from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / "sailor-opposite-contact-transparent.png"
HEX = ROOT / "sailor-on-144-hex.png"


def opposite_contact(source: Image.Image, cutoff: int, axis: float) -> Image.Image:
    result = source.copy()
    pixels = source.load()
    out = result.load()

    # Clear the legs while retaining the upright spear shaft.
    for y in range(cutoff, source.height):
        for x in range(source.width):
            is_spear = x < 500 and y < 920
            if not is_spear:
                out[x, y] = (0, 0, 0, 0)

    # Reflect only the lower body around the hips. The spear is excluded from
    # the reflection, and its original pixels remain untouched above.
    for y in range(cutoff, source.height):
        for x in range(source.width):
            if x < 500 and y < 920:
                continue
            pixel = pixels[x, y]
            if pixel[3] == 0:
                continue
            target_x = round(2 * axis - x)
            if 0 <= target_x < source.width:
                out[target_x, y] = pixel

    return result


def fit_to_frame(image: Image.Image) -> Image.Image:
    bbox = image.getchannel("A").getbbox()
    assert bbox is not None
    crop = image.crop(bbox)
    target_height = 98
    target_width = max(1, round(crop.width * target_height / crop.height))
    crop = crop.resize((target_width, target_height), Image.Resampling.NEAREST)
    frame = Image.new("RGBA", (144, 144), (0, 0, 0, 0))
    frame.alpha_composite(crop, ((144 - target_width) // 2, 120 - target_height))
    return frame


def graft_legs(base: Image.Image, donor: Image.Image, cutoff: int, shift_x: int) -> Image.Image:
    result = base.copy()
    base_pixels = base.load()
    donor_pixels = donor.load()
    out = result.load()

    def is_spear(x: int, y: int) -> bool:
        return 49 <= x <= 59 and y < 116

    for y in range(cutoff, 144):
        for x in range(144):
            if not is_spear(x, y):
                out[x, y] = (0, 0, 0, 0)

    for y in range(cutoff, 144):
        for x in range(144):
            # Remove the donor's shaft, but retain its boot pixels at ground level.
            if x <= 59 and y < 108:
                continue
            pixel = donor_pixels[x, y]
            if pixel[3] == 0:
                continue
            target_x = x + shift_x
            if 0 <= target_x < 144:
                out[target_x, y] = pixel

    # Restore the exact spear from the standing source over the graft.
    for y in range(cutoff, 116):
        for x in range(49, 60):
            if base_pixels[x, y][3] != 0:
                out[x, y] = base_pixels[x, y]
    return result


def on_hex(frame: Image.Image) -> Image.Image:
    background = Image.open(HEX).convert("RGBA")
    background.alpha_composite(frame)
    return background


def main() -> None:
    source = Image.open(SOURCE).convert("RGBA")
    combinations = [(650, 630), (650, 645), (670, 630), (670, 645), (690, 630), (690, 645)]
    strip = Image.new("RGBA", (144 * len(combinations), 144), (0, 0, 0, 0))
    for index, (cutoff, axis) in enumerate(combinations):
        pose = opposite_contact(source, cutoff, axis)
        pose.save(ROOT / f"sailor-opposite-contact-c{cutoff}-a{axis}.png")
        frame = fit_to_frame(pose)
        frame.save(ROOT / f"sailor-opposite-contact-c{cutoff}-a{axis}-144.png")
        strip.alpha_composite(on_hex(frame), (144 * index, 0))
    strip.resize((strip.width * 3, strip.height * 3), Image.Resampling.NEAREST).save(
        ROOT / "sailor-opposite-contact-options-3x.png"
    )

    original_contact = fit_to_frame(source)
    opposite = fit_to_frame(opposite_contact(source, 670, 645))
    base_sheet = Image.open(ROOT / "sailor-attack-5frame.png").convert("RGBA")
    base = base_sheet.crop((0, 0, 144, 144))
    graft_options = []
    labels = []
    for cutoff in (68, 72, 76):
        for shift_x in (-2, 0, 2):
            left = graft_legs(base, original_contact, cutoff, shift_x)
            right = graft_legs(base, opposite, cutoff, shift_x)
            graft_options.extend((left, right))
            labels.extend(((cutoff, shift_x, "left"), (cutoff, shift_x, "right")))
    graft_strip = Image.new("RGBA", (144 * len(graft_options), 144), (0, 0, 0, 0))
    for index, frame in enumerate(graft_options):
        graft_strip.alpha_composite(on_hex(frame), (index * 144, 0))
        cutoff, shift_x, side = labels[index]
        frame.save(ROOT / f"sailor-graft-c{cutoff}-x{shift_x}-{side}.png")
    graft_strip.resize((graft_strip.width * 2, graft_strip.height * 2), Image.Resampling.NEAREST).save(
        ROOT / "sailor-graft-options-2x.png"
    )

    # Final walk: neutral -> left contact -> neutral -> right contact.
    left_contact = graft_legs(base, original_contact, 68, 0)
    right_contact = graft_legs(base, opposite, 68, 0)
    final_walk = (base, left_contact, base, right_contact)
    walk_sheet = Image.new("RGBA", (144 * len(final_walk), 144), (0, 0, 0, 0))
    for index, frame in enumerate(final_walk):
        walk_sheet.alpha_composite(frame, (index * 144, 0))
    walk_sheet.save(ROOT / "sailor-walk-alternating-final-4frame.png")

    attack_sheet = Image.open(ROOT / "sailor-attack-5frame.png").convert("RGBA")
    combined = Image.new("RGBA", (144 * 9, 144), (0, 0, 0, 0))
    combined.alpha_composite(attack_sheet, (0, 0))
    combined.alpha_composite(walk_sheet, (144 * 5, 0))
    combined.save(ROOT / "sailor-attack-walk-final-9frame.png")

    hex_frames = [on_hex(frame) for frame in final_walk]
    hex_frames[0].save(
        ROOT / "sailor-walk-alternating-final-on-hex.gif",
        save_all=True,
        append_images=hex_frames[1:],
        duration=180,
        loop=0,
        disposal=2,
    )


if __name__ == "__main__":
    main()
