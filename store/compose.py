"""Immagini per la scheda del Microsoft Store: sfondo, titolo e schermata con ombra.
Uso: python compose.py <cartella delle foto> <cartella di uscita>"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter, ImageFont

SRC, OUT = Path(sys.argv[1]), Path(sys.argv[2])
W, H = 2560, 1440
FONTS = Path("C:/Windows/Fonts")
BOLD = str(FONTS / "segoeuib.ttf")
SEMI = str(FONTS / "seguisb.ttf")
REG = str(FONTS / "segoeui.ttf")
BG = Image.open(SRC / "bg-dark.png").convert("RGBA").resize((W, H))

TEXT = {
    "it": [
        ("deck", "Le tue note, sul bordo\ndello schermo", "Avvicina il mouse al bordo e le note si aprono a ventaglio.\nAllontanalo e spariscono. Non ingombrano mai."),
        ("all", "Tutte le note,\nin ordine", "Cartelle, tag, caselle da spuntare, immagini e promemoria.\nRicerca istantanea, anche senza accenti."),
        ("search", "Trova tutto con\nCtrl+Alt+F", "Una barra al centro dello schermo, da qualsiasi programma.\nScrivi, Invio, e la nota è aperta."),
        ("sync", "Le stesse note\nsu tutti i tuoi PC", "Con Google Drive, OneDrive o Dropbox: niente account,\nniente server. Anche cifrate con una frase segreta."),
    ],
    "en": [
        ("deck", "Your notes, on the edge\nof your screen", "Move the mouse to the edge and your notes fan out.\nMove it away and they disappear. Never in the way."),
        ("all", "All your notes,\nneatly organised", "Folders, tags, checklists, images and reminders.\nInstant search across everything."),
        ("search", "Find anything with\nCtrl+Alt+F", "A search bar in the middle of the screen, from any app.\nType, press Enter, and the note is open."),
        ("sync", "The same notes\non all your PCs", "Through Google Drive, OneDrive or Dropbox: no account,\nno server. Optionally encrypted with a passphrase."),
    ],
}
FILES = {
    "deck": "deck-pw-{l}.png",
    "all": "all-{l}.png",
    "search": "search-{l}.png",
    "sync": "sync-{l}.png",
}
FEATURES = {
    "it": ("Gratis, per davvero", ["Niente pubblicità", "Niente account", "Le note restano tue", "Italiano e inglese"],
           "Promemoria con notifiche · Timer pomodoro · Note protette da PIN · Backup automatico\nLink tra note · Cattura dagli appunti · Liste condivise · Copie da leggere sul telefono"),
    "en": ("Free, for real", ["No ads", "No account", "Your notes stay yours", "English and Italian"],
           "Reminders with notifications · Pomodoro timer · PIN-protected notes · Automatic backup\nLinks between notes · Clipboard capture · Shared lists · Copies to read on your phone"),
}


def key_black(im: Image.Image, threshold: int = 6) -> Image.Image:
    """Il nero puro intorno alle finestre trasparenti diventa trasparente."""
    im = im.convert("RGBA")
    px = im.load()
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, _ = px[x, y]
            if r <= threshold and g <= threshold and b <= threshold:
                px[x, y] = (0, 0, 0, 0)
    return im


def trim(im: Image.Image) -> Image.Image:
    box = im.getchannel("A").getbbox()
    return im.crop(box) if box else im


def rounded(im: Image.Image, radius: int) -> Image.Image:
    im = im.convert("RGBA")
    mask = Image.new("L", im.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, im.width - 1, im.height - 1), radius, fill=255)
    alpha = Image.composite(im.getchannel("A"), Image.new("L", im.size, 0), mask)
    im.putalpha(alpha)
    return im


def with_shadow(canvas: Image.Image, im: Image.Image, pos, blur=40, offset=24, strength=150):
    shadow = Image.new("RGBA", (im.width + blur * 4, im.height + blur * 4), (0, 0, 0, 0))
    alpha = im.getchannel("A").point(lambda a: strength if a > 0 else 0)
    shadow.paste((0, 0, 0, 255), (blur * 2, blur * 2), alpha)
    shadow = shadow.filter(ImageFilter.GaussianBlur(blur))
    canvas.alpha_composite(shadow, (pos[0] - blur * 2, pos[1] - blur * 2 + offset))
    canvas.alpha_composite(im, pos)


def title(draw, xy, text, size, font=BOLD, fill=(255, 255, 255, 255), spacing=12):
    draw.multiline_text(xy, text, font=ImageFont.truetype(font, size), fill=fill, spacing=spacing)


def shot_image(kind, lang, head, sub):
    canvas = BG.copy()
    draw = ImageDraw.Draw(canvas)
    im = Image.open(SRC / FILES[kind].format(l=lang)).convert("RGBA")
    if kind == "deck":
        im = trim(key_black(im))
        im = im.resize((int(im.width * 1.35), int(im.height * 1.35)), Image.LANCZOS)
        pos = (W - im.width - 40, (H - im.height) // 2 + 60)
        with_shadow(canvas, im, pos, blur=30, offset=16, strength=120)
        title(draw, (150, 430), head, 104)
        title(draw, (154, 730), sub, 44, REG, (225, 230, 245, 255))
    else:
        # Titolo e sottotitolo in alto, al centro; la finestra sotto.
        head1 = " ".join(head.splitlines())
        f = ImageFont.truetype(BOLD, 86)
        tw = draw.textlength(head1, font=f)
        draw.text(((W - tw) / 2, 90), head1, font=f, fill=(255, 255, 255, 255))
        sf = ImageFont.truetype(REG, 38)
        for i, line in enumerate(sub.splitlines()):
            lw = draw.textlength(line, font=sf)
            draw.text(((W - lw) / 2, 220 + i * 54), line, font=sf, fill=(225, 230, 245, 255))
        if kind == "search":
            im = rounded(trim(key_black(im)), 26)
            box_w, box_h = 1250, 860
        else:
            im = rounded(im, 16)
            box_w, box_h = 2000, 1000
        ratio = min(box_w / im.width, box_h / im.height)
        im = im.resize((int(im.width * ratio), int(im.height * ratio)), Image.LANCZOS)
        pos = ((W - im.width) // 2, 360 + (box_h - im.height) // 2)
        with_shadow(canvas, im, pos)
    return canvas


def features_image(lang):
    canvas = BG.copy()
    draw = ImageDraw.Draw(canvas)
    head, chips, line = FEATURES[lang]
    icon = Image.open(Path(__file__).resolve().parents[0] / "icon.png").convert("RGBA").resize((300, 300), Image.LANCZOS)
    with_shadow(canvas, icon, ((W - 300) // 2, 230), blur=30, offset=14, strength=110)
    f = ImageFont.truetype(BOLD, 120)
    tw = draw.textlength(head, font=f)
    draw.text(((W - tw) / 2, 590), head, font=f, fill=(255, 255, 255, 255))
    cf = ImageFont.truetype(SEMI, 46)
    widths = [draw.textlength(c, font=cf) + 90 for c in chips]
    x = (W - (sum(widths) + 30 * (len(chips) - 1))) / 2
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ld = ImageDraw.Draw(layer)
    for w in widths:
        ld.rounded_rectangle((x, 790, x + w, 880), 45, fill=(255, 255, 255, 38), outline=(255, 255, 255, 110), width=2)
        x += w + 30
    canvas.alpha_composite(layer)
    draw = ImageDraw.Draw(canvas)
    x = (W - (sum(widths) + 30 * (len(chips) - 1))) / 2
    for c, w in zip(chips, widths):
        draw.text((x + 45, 805), c, font=cf, fill=(255, 255, 255, 255))
        x += w + 30
    lf = ImageFont.truetype(REG, 40)
    for i, l in enumerate(line.split("\n")):
        lw = draw.textlength(l, font=lf)
        draw.text(((W - lw) / 2, 1000 + i * 64), l, font=lf, fill=(215, 222, 240, 255))
    return canvas


for lang in ("it", "en"):
    out = OUT / lang
    out.mkdir(parents=True, exist_ok=True)
    for n, (kind, head, sub) in enumerate(TEXT[lang], start=1):
        shot_image(kind, lang, head, sub).convert("RGB").save(out / f"{n:02d}-{kind}.png", optimize=True)
    features_image(lang).convert("RGB").save(out / "05-gratis.png" if lang == "it" else out / "05-free.png", optimize=True)
    print("fatto", lang)
