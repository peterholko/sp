from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parent
SOURCE = Path("sp_frontend/priv/static/art/novicewarrior.png")
CELL = 512
SCALE = 3
FRAME = 144


def main() -> None:
    sheet = Image.open(SOURCE).convert("RGBA")
    if sheet.size != (FRAME * 9, FRAME):
        raise ValueError(f"unexpected source dimensions: {sheet.size}")

    grid = Image.new("RGB", (CELL * 3, CELL * 3), (0, 255, 0))
    for index in range(9):
        frame = sheet.crop((index * FRAME, 0, (index + 1) * FRAME, FRAME))
        frame = frame.resize((FRAME * SCALE, FRAME * SCALE), Image.Resampling.NEAREST)
        row, column = divmod(index, 3)
        x = column * CELL + (CELL - frame.width) // 2
        y = row * CELL + (CELL - frame.height) // 2
        grid.paste(frame.convert("RGB"), (x, y), frame.getchannel("A"))

    output = ROOT / "sailor-spear-edit-grid.png"
    grid.save(output)
    print(output, grid.size)


if __name__ == "__main__":
    main()
