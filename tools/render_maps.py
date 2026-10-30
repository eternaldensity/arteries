#!/usr/bin/env python3
"""Render map data produced by tools/gen-maps.sh into SVG assets."""
import pathlib
import sys

W, H = 10, 6

COLORS = {
    "1": ("#263b30", "url(#forest)"),
    "2": ("#1d3828", "url(#forest)"),
    "3": ("#4a4130", "url(#hills)"),
    "4": ("#665f52", "url(#mountain)"),
    "5": ("#123149", "url(#water)"),
    "6": ("#1a4252", "url(#coast)"),
    "7": ("#39462d", "url(#marsh)"),
}


def main():
    source, out_dir = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)
    for line in source.read_text().splitlines():
        if not line.strip():
            continue
        seed_text, rows_text = line.split("|", 1)
        seed = int(seed_text)
        rows = rows_text.split(";")
        if len(rows) != H or any(len(row.split(",")) != W for row in rows):
            raise SystemExit(f"bad map data for seed {seed}")
        rects = []
        for y, row in enumerate(rows):
            for x, code in enumerate(row.split(",")):
                fill, pattern = COLORS[code]
                rects.append(
                    f'<rect x="{x}" y="{y}" width="1.01" height="1.01" fill="{pattern}"/>'
                )
        svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" preserveAspectRatio="none">
<defs>
  <pattern id="forest" width="2" height="2" patternUnits="userSpaceOnUse">
    <rect width="2" height="2" fill="#1d3828"/>
    <circle cx=".55" cy=".55" r=".18" fill="#3d7d4d" opacity=".8"/>
    <circle cx="1.45" cy="1.35" r=".14" fill="#2c6040" opacity=".75"/>
  </pattern>
  <pattern id="hills" width="2" height="1.4" patternUnits="userSpaceOnUse">
    <rect width="2" height="1.4" fill="#4a4130"/>
    <path d="M0 1.1 Q.5 .55 1 1.1 T2 1.1" fill="none" stroke="#75664a" stroke-width=".07" opacity=".6"/>
  </pattern>
  <pattern id="mountain" width="2" height="1.5" patternUnits="userSpaceOnUse">
    <rect width="2" height="1.5" fill="#665f52"/>
    <path d="M0 1.3 L.8 .2 L1.4 1.3Z" fill="#837a69" opacity=".65"/>
    <path d="M.9 1.3 L1.55 .45 L2 1.3Z" fill="#514a40" opacity=".6"/>
  </pattern>
  <pattern id="water" width="2" height="1" patternUnits="userSpaceOnUse">
    <rect width="2" height="1" fill="#123149"/>
    <path d="M0 .45 Q.5 .2 1 .45 T2 .45" fill="none" stroke="#2f6d90" stroke-width=".05" opacity=".55"/>
  </pattern>
  <pattern id="coast" width="2" height="1" patternUnits="userSpaceOnUse">
    <rect width="2" height="1" fill="#1a4252"/>
    <path d="M0 .7 Q.5 .45 1 .7 T2 .7" fill="none" stroke="#3e7f91" stroke-width=".05" opacity=".5"/>
  </pattern>
  <pattern id="marsh" width="2" height="1.2" patternUnits="userSpaceOnUse">
    <rect width="2" height="1.2" fill="#39462d"/>
    <ellipse cx=".6" cy=".65" rx=".3" ry=".13" fill="#536840" opacity=".7"/>
    <ellipse cx="1.5" cy=".4" rx=".22" ry=".1" fill="#4a6038" opacity=".6"/>
  </pattern>
</defs>
<rect width="{W}" height="{H}" fill="#263b30"/>
{''.join(rects)}
<path d="M0 0H{W}V{H}H0Z" fill="none" stroke="#0b1220" stroke-width=".04"/>
</svg>
'''
        (out_dir / f"map-{seed}.svg").write_text(svg)
        print(f"wrote {out_dir / f'map-{seed}.svg'}")


if __name__ == "__main__":
    main()
