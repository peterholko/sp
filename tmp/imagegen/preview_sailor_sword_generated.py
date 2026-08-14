from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parent
SHEET = ROOT / "sailor-sword-generated-normalized-9frame.png"
HEX = ROOT / "sailor-on-144-hex.png"
FRAME = 144


def save_preview(name: str, indices: list[int], duration: int) -> None:
    sheet = Image.open(SHEET).convert("RGBA")
    hex_tile = Image.open(HEX).convert("RGBA")
    frames = []
    for index in indices:
        frame = hex_tile.copy()
        frame.alpha_composite(sheet.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME)))
        frames.append(frame)
    frames[0].save(
        ROOT / f"sailor-sword-generated-{name}-on-hex.gif",
        save_all=True,
        append_images=frames[1:],
        duration=duration,
        loop=0,
        disposal=2,
    )


def main() -> None:
    save_preview("attack", [0, 1, 2, 3, 4, 0], 140)
    save_preview("walk", [5, 6, 7, 8], 180)


if __name__ == "__main__":
    main()
