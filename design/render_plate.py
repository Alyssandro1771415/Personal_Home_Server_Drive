#!/usr/bin/env python3
"""Umbral Steel identity plate — museum plate, not a poster."""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

FONTS = Path("/home/alyssandro/.cursor/skills/canvas-design/canvas-fonts")
OUT = Path(__file__).resolve().parent / "umbral-steel.png"

W, H = 1400, 1866  # 3:4
VOID = (7, 8, 11)
INK = (11, 13, 18)
HAIR = (32, 38, 48)
HAIR_DIM = (22, 26, 34)
STEEL = (109, 138, 168)
PEWTER = (139, 149, 167)
ICE = (214, 220, 228)
GHOST = (18, 22, 29)
FILL = (28, 36, 48)
FILL_HI = (42, 54, 70)


def font(name: str, size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(str(FONTS / name), size)


def draw_hairline(draw: ImageDraw.ImageDraw, box, fill=HAIR, width=1):
    x0, y0, x1, y1 = box
    draw.rectangle([x0, y0, x1, y1], outline=fill, width=width)


def main() -> None:
    img = Image.new("RGB", (W, H), VOID)
    draw = ImageDraw.Draw(img)

    # faint field grain — vertical brushed metal, not noise spray
    grain = Image.new("RGB", (W, H), VOID)
    g = ImageDraw.Draw(grain)
    for x in range(0, W, 3):
        shade = 8 + (x * 17) % 5
        g.line([(x, 0), (x, H)], fill=(shade, shade + 1, shade + 3))
    img = Image.blend(img, grain, 0.18)
    draw = ImageDraw.Draw(img)

    m = 88
    # outer plate
    draw_hairline(draw, (m, m, W - m, H - m), HAIR, 1)
    draw_hairline(draw, (m + 10, m + 10, W - m - 10, H - m - 10), HAIR_DIM, 1)

    jura = font("Jura-Light.ttf", 22)
    jura_sm = font("Jura-Medium.ttf", 13)
    mono = font("IBMPlexMono-Regular.ttf", 13)
    mono_sm = font("IBMPlexMono-Regular.ttf", 11)
    mono_lg = font("IBMPlexMono-Bold.ttf", 13)
    title = font("Outfit-Regular.ttf", 92)
    title2 = font("Outfit-Regular.ttf", 92)

    top = m + 36
    draw.text((m + 36, top), "PLATE  07", font=mono_sm, fill=PEWTER)
    draw.text((W - m - 36 - 168, top), "SPEC. LOCAL", font=mono_sm, fill=PEWTER)
    draw.line([(m + 36, top + 28), (W - m - 36, top + 28)], fill=HAIR, width=1)

    # title block
    ty = top + 56
    draw.text((m + 36, ty), "UMBRAL", font=title, fill=ICE)
    draw.text((m + 36, ty + 96), "STEEL", font=title2, fill=STEEL)

    footer_y = H - m - 168
    spine_x = m + 36
    draw.line([(spine_x, ty + 214), (spine_x, footer_y)], fill=STEEL, width=2)

    captions = [
        (ty + 236, "01  VOID"),
        (ty + 264, "02  INK"),
        (ty + 292, "03  HAIRLINE"),
        (ty + 320, "04  PEWTER"),
        (ty + 348, "05  STEEL"),
    ]
    for y, label in captions:
        fill = STEEL if "STEEL" in label else (PEWTER if "PEWTER" in label else HAIR)
        draw.rectangle([spine_x + 16, y + 5, spine_x + 24, y + 13], fill=fill)
        draw.rectangle([spine_x + 16, y + 5, spine_x + 24, y + 13], outline=HAIR, width=1)
        draw.text((spine_x + 34, y), label, font=mono_sm, fill=PEWTER)

    origin_x = m + 250
    cell = 36
    gap = 3
    block_gap = 16
    lattice_h = 4 * (4 * cell + 3 * gap) + 3 * block_gap
    title_bottom = ty + 214
    origin_y = title_bottom + max(36, ((footer_y - 20) - title_bottom - lattice_h) // 2)
    # deterministic occupancy: suggests a wrapped key, not random clutter
    seed = [
        1, 0, 1, 1,  0, 1, 0, 0,  1, 1, 0, 1,  0, 0, 1, 0,
        0, 1, 1, 0,  1, 0, 1, 1,  0, 1, 0, 0,  1, 0, 1, 1,
        1, 1, 0, 0,  0, 1, 1, 0,  1, 0, 1, 1,  0, 1, 0, 0,
        0, 0, 1, 1,  1, 0, 0, 1,  0, 1, 1, 0,  1, 1, 0, 1,
        1, 0, 0, 1,  1, 1, 0, 0,  0, 0, 1, 1,  1, 0, 1, 0,
        0, 1, 1, 0,  0, 0, 1, 1,  1, 1, 0, 0,  0, 1, 0, 1,
        1, 1, 1, 0,  1, 0, 0, 1,  0, 1, 0, 1,  1, 0, 0, 1,
        0, 0, 1, 1,  0, 1, 1, 0,  1, 0, 1, 0,  0, 1, 1, 0,
        1, 0, 1, 0,  1, 1, 0, 1,  0, 0, 1, 1,  1, 0, 0, 1,
        0, 1, 0, 1,  0, 0, 1, 0,  1, 1, 0, 0,  0, 1, 1, 0,
        1, 1, 0, 1,  1, 0, 0, 1,  0, 1, 1, 0,  1, 0, 1, 1,
        0, 0, 1, 0,  0, 1, 1, 0,  1, 0, 0, 1,  0, 1, 0, 0,
        1, 0, 0, 1,  1, 1, 0, 0,  0, 1, 1, 1,  1, 0, 1, 0,
        0, 1, 1, 1,  0, 0, 1, 1,  1, 0, 0, 0,  0, 1, 0, 1,
        1, 0, 1, 0,  1, 1, 0, 1,  0, 1, 1, 0,  1, 0, 1, 0,
        0, 1, 0, 1,  0, 1, 1, 0,  1, 0, 0, 1,  0, 1, 1, 1,
    ]

    # 16×16 cells grouped as 4×4 blocks
    idx = 0
    for by in range(4):
        for bx in range(4):
            bx0 = origin_x + bx * (4 * (cell + gap) - gap + block_gap)
            by0 = origin_y + by * (4 * (cell + gap) - gap + block_gap)
            # block frame
            bw = 4 * cell + 3 * gap
            bh = bw
            draw.rectangle([bx0 - 4, by0 - 4, bx0 + bw + 4, by0 + bh + 4], outline=HAIR_DIM, width=1)
            for cy in range(4):
                for cx in range(4):
                    x = bx0 + cx * (cell + gap)
                    y = by0 + cy * (cell + gap)
                    on = seed[idx]
                    idx += 1
                    if on:
                        col = FILL_HI if (bx + by) % 2 == 0 else FILL
                        draw.rectangle([x, y, x + cell - 1, y + cell - 1], fill=col)
                    else:
                        draw.rectangle([x, y, x + cell - 1, y + cell - 1], outline=HAIR, width=1)

    # coordinate ticks on lattice
    lattice_w = 4 * (4 * cell + 3 * gap) + 3 * block_gap
    draw.text((origin_x, origin_y - 30), "FIG. 03  —  STATE MATRIX", font=mono_sm, fill=PEWTER)
    tw = draw.textlength("16 × 16", font=mono_sm)
    draw.text((origin_x + lattice_w - tw, origin_y - 30), "16 × 16", font=mono_sm, fill=HAIR)

    sw_x = W - m - 168
    sw_y = ty + 6
    swatches = [
        ("VOID", VOID),
        ("INK", INK),
        ("GHOST", GHOST),
        ("HAIR", HAIR),
        ("PEWTER", PEWTER),
        ("STEEL", STEEL),
        ("ICE", ICE),
    ]
    for i, (name, color) in enumerate(swatches):
        y = sw_y + i * 34
        draw.rectangle([sw_x, y, sw_x + 72, y + 20], fill=color)
        draw.rectangle([sw_x, y, sw_x + 72, y + 20], outline=PEWTER if name == "VOID" else HAIR, width=1)
        draw.text((sw_x + 82, y + 3), name, font=mono_sm, fill=PEWTER)

    by = footer_y
    draw.line([(m + 36, by), (W - m - 36, by)], fill=HAIR, width=1)
    samples = [("256", "BIT"), ("GCM", "MODE"), ("DEK", "SEALED"), ("TTL", "30D")]
    slot = (W - 2 * m - 72) // 4
    for i, (big, small) in enumerate(samples):
        x = m + 36 + i * slot
        draw.text((x, by + 22), big, font=font("Outfit-Regular.ttf", 36), fill=ICE)
        draw.text((x, by + 68), small, font=mono_sm, fill=PEWTER)
        if i < 3:
            draw.line([(x + slot - 18, by + 28), (x + slot - 18, by + 86)], fill=HAIR_DIM, width=1)

    draw.text((m + 36, H - m - 36), "UMBRAL STEEL  ·  NO GOLD", font=mono_sm, fill=HAIR)
    draw.text((W - m - 220, H - m - 36), "MASTER PLATE  /  01", font=mono_sm, fill=HAIR)

    # slight unsharp to keep edges crisp after blend
    img = img.filter(ImageFilter.UnsharpMask(radius=0.6, percent=80, threshold=2))
    img.save(OUT, "PNG", optimize=True)
    print(f"wrote {OUT} {img.size}")


if __name__ == "__main__":
    main()
