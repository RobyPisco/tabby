"""Icone del pacchetto MSIX e del Microsoft Store, ricavate da src-tauri/icons/icon.png.

Uso: python scripts/msix/make-assets.py <cartella di destinazione>
"""
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "src-tauri" / "icons" / "icon.png"
DARK = (40, 45, 58, 255)  # lo stesso blu-grigio dello sfondo dell'icona


def square(icon: Image.Image, size: int, padding: float = 0.0) -> Image.Image:
    """L'icona centrata in un quadrato trasparente, con un margine in proporzione."""
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    inner = max(1, round(size * (1 - 2 * padding)))
    scaled = icon.resize((inner, inner), Image.LANCZOS)
    canvas.alpha_composite(scaled, ((size - inner) // 2, (size - inner) // 2))
    return canvas


def wide(icon: Image.Image, width: int, height: int) -> Image.Image:
    """Riquadro largo del menu Start: icona al centro su sfondo scuro."""
    canvas = Image.new("RGBA", (width, height), DARK)
    inner = round(height * 0.7)
    scaled = icon.resize((inner, inner), Image.LANCZOS)
    canvas.alpha_composite(scaled, ((width - inner) // 2, (height - inner) // 2))
    return canvas


def main(dest: Path) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    icon = Image.open(SOURCE).convert("RGBA")
    files = {
        "StoreLogo.png": square(icon, 50),
        "Square44x44Logo.png": square(icon, 44),
        "Square150x150Logo.png": square(icon, 150, 0.12),
        "Wide310x150Logo.png": wide(icon, 310, 150),
    }
    # Icone per barra delle applicazioni ed Esplora risorse, nitide a ogni dimensione.
    for size in (16, 24, 32, 48, 256):
        files[f"Square44x44Logo.targetsize-{size}.png"] = square(icon, size)
        files[f"Square44x44Logo.targetsize-{size}_altform-unplated.png"] = square(icon, size)
    for name, image in files.items():
        image.save(dest / name)
    print(f"{len(files)} icone in {dest}")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
