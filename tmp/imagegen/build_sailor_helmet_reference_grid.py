from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parent
SOURCE = Path("sp_frontend/priv/static/art/novicewarrior.png")
CELL = 512
FRAME = 144
SCALE = 3


def main() -> None:
    sheet = Image.open(SOURCE).convert("RGBA")
    if sheet.size != (FRAME * 9, FRAME):
        raise ValueError(sheet.size)
    grid = Image.new("RGB", (CELL * 3, CELL * 3), (0, 255, 0))
    for index in range(9):
        frame = sheet.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
        enlarged = frame.resize((FRAME * SCALE, FRAME * SCALE), Image.Resampling.NEAREST)
        row, column = divmod(index, 3)
        x = column * CELL + (CELL - enlarged.width) // 2
        y = row * CELL + (CELL - enlarged.height) // 2
        grid.paste(enlarged.convert("RGB"), (x, y), enlarged.getchannel("A"))
    grid.save(ROOT / "sailor-helmet-reference-grid.png")
    print(grid.size)


if __name__ == "__main__":
    main()
