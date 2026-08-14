from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parent
FRAME = 144
SCALE = 4


def main() -> None:
    sheets = [
        Image.open("sp_frontend/priv/static/art/novicewarrior.png").convert("RGBA"),
        Image.open(ROOT / "sailor-sword-generated-normalized-9frame.png").convert("RGBA"),
        Image.open(ROOT / "sailor-sword-composite-9frame.png").convert("RGBA"),
    ]
    canvas = Image.new("RGBA", (FRAME * SCALE * 9, FRAME * SCALE * 3), (0, 0, 0, 0))
    for row, sheet in enumerate(sheets):
        for index in range(9):
            frame = sheet.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
            frame = frame.resize((FRAME * SCALE, FRAME * SCALE), Image.Resampling.NEAREST)
            canvas.alpha_composite(frame, (index * FRAME * SCALE, row * FRAME * SCALE))
    canvas.save(ROOT / "sailor-weapon-frame-comparison-4x.png")


if __name__ == "__main__":
    main()
