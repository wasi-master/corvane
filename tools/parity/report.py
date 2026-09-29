"""HTML + JSON report for a parity run (rewritten after every scenario)."""

from __future__ import annotations

import html
import json
from pathlib import Path

_CSS = """
:root{--bg:#f6f8fa;--fg:#1f2328;--muted:#59636e;--card:#fff;--line:#d1d9e0;--ok:#1a7f37;--bad:#cf222e;--warn:#9a6700}
@media (prefers-color-scheme:dark){:root{--bg:#0d1117;--fg:#e6edf3;--muted:#9198a1;--card:#151b23;--line:#30363d;--ok:#3fb950;--bad:#f85149;--warn:#d29922}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.45 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
main{max-width:1500px;margin:0 auto;padding:24px 16px 80px}h1{font-size:22px;margin:0 0 4px}h2{font-size:17px;margin:28px 0 6px}
.muted{color:var(--muted)}table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid var(--line);padding:5px 8px;text-align:left;vertical-align:top}
th{font-weight:600;color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.03em}
.b{display:inline-block;padding:1px 8px;border-radius:10px;font-size:12px;font-weight:600;color:#fff}.ok{background:var(--ok)}.bad{background:var(--bad)}.warn{background:var(--warn)}
.card{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:14px;margin:12px 0}
.snaphead{display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin-bottom:8px}.snaphead h3{margin:0;font-size:15px}
.modes button{background:none;border:1px solid var(--line);color:var(--fg);border-radius:6px;padding:3px 10px;cursor:pointer;font:inherit;font-size:12px}
.modes button.on{background:var(--fg);color:var(--bg)}
.view{position:relative;overflow:auto}.view img{max-width:100%;display:block}
.pair{display:grid;grid-template-columns:1fr 1fr;gap:8px}.pair figure{margin:0}.pair figcaption{font-size:12px;color:var(--muted)}
.swipe{position:relative;display:inline-block;max-width:100%}.swipe img{display:block;max-width:100%}.swipe .top{position:absolute;inset:0;clip-path:inset(0 50% 0 0)}
.swipe input{width:100%}.crop img{max-width:100%;border-radius:4px}code{font:12px ui-monospace,SFMono-Regular,Menlo,monospace}
.sw{display:inline-block;width:11px;height:11px;border-radius:2px;border:1px solid var(--line);vertical-align:-1px;margin-right:3px}
pre{white-space:pre-wrap;font-size:12px}a{color:inherit}
"""

_JS = """
document.querySelectorAll('.snap').forEach(card=>{
  const views=card.querySelectorAll('.view'), btns=card.querySelectorAll('.modes button');
  let blink=null;
  btns.forEach(b=>b.onclick=()=>{
    btns.forEach(x=>x.classList.toggle('on',x===b));
    views.forEach(v=>v.hidden=v.dataset.mode!==b.dataset.mode);
    clearInterval(blink);
    if(b.dataset.mode==='blink'){const img=card.querySelector('.view[data-mode=blink] img');let f=0;
      blink=setInterval(()=>{f^=1;img.src=f?img.dataset.b:img.dataset.a;img.nextElementSibling.textContent=f?'Corvane':'GitHub Desktop'},650)}
  });
  const r=card.querySelector('.swipe input');
  if(r) r.oninput=()=>card.querySelector('.swipe .top').style.clipPath=`inset(0 ${100-r.value}% 0 0)`;
});
"""


def _e(s) -> str:
    return html.escape(str(s))


def write(out: Path, results: list[dict], defaults: dict):
    (out / "results.json").write_text(json.dumps({"defaults": defaults, "results": results}, indent=1))
    snaps = [s for r in results for s in r["snaps"]]
    bad = [s for s in snaps if not s["pass"]]
    rows = []
    for r in results:
        worst = max((s["percent"] for s in r["snaps"]), default=0)
        badge = '<span class="b bad">error</span>' if r["error"] else ('<span class="b ok">pass</span>' if r["pass"] else '<span class="b bad">fail</span>')
        rows.append(
            f'<tr><td><a href="#{_e(r["name"])}-{_e(r["theme"])}">{_e(r["name"])}</a></td><td>{_e(r["theme"])}</td><td>{badge}</td>'
            f'<td>{sum(s["pass"] for s in r["snaps"])}/{len(r["snaps"])}</td><td>{worst:.2f}%</td><td class="muted">{_e(r["description"])}</td></tr>'
        )
    body = [
        "<main><h1>GitHub Desktop ⇄ Corvane parity</h1>",
        f'<p class="muted">{len(snaps) - len(bad)}/{len(snaps)} snaps within threshold · default threshold {defaults["threshold"]}% · '
        f'tolerance Δ{defaults["tolerance"]} flat / Δ{defaults["edge_tolerance"]} edges · {defaults["radius"]}pt positional slack · window {defaults["width"]}×{defaults["height"]}</p>',
        "<table><tr><th>Scenario</th><th>Theme</th><th>Status</th><th>Snaps</th><th>Worst</th><th>About</th></tr>",
        *rows,
        "</table>",
    ]
    for r in results:
        body.append(f'<h2 id="{_e(r["name"])}-{_e(r["theme"])}">{_e(r["name"])} <span class="muted">· {_e(r["theme"])} · {_e(r["file"])} · {r.get("seconds", 0)}s</span></h2>')
        if r["description"]:
            body.append(f'<p class="muted">{_e(r["description"])}</p>')
        for n in r.get("notes", []):
            body.append(f'<p><span class="b warn">note</span> {_e(n)}</p>')
        if r["error"]:
            body.append(f'<div class="card"><span class="b bad">error</span> {_e(r["error"])}<pre>{_e(r.get("traceback", ""))}</pre></div>')
        base = f'shots/{r["name"]}-{r["theme"]}/'
        for m in r.get("menus", []):
            if m["pass"]:
                body.append(f'<p><span class="b ok">menu</span> {_e(m["name"])}: <code>{_e(" | ".join(m["ghd"]))}</code></p>')
        for dump in r.get("dumps", []):
            body.append(f'<p class="muted">GHD DOM dump: <a href="{base + dump}">{_e(dump)}</a></p>')
        for s in r["snaps"]:
            if s.get("menu"):
                g, c = s["menu"]["ghd"], s["menu"]["corvane"]
                body.append(f'<div class="card"><div class="snaphead"><h3>{_e(s["name"])}</h3><span class="b bad">items differ</span></div>'
                            f'<div class="pair"><figure><pre>{_e(chr(10).join(g))}</pre><figcaption>GitHub Desktop</figcaption></figure>'
                            f'<figure><pre>{_e(chr(10).join(c))}</pre><figcaption>Corvane</figcaption></figure></div></div>')
                continue
            if s.get("ghd_only"):
                body.append(f'<div class="card"><h3>{_e(s["name"])}</h3><img loading="lazy" style="max-width:100%" src="{base + s["ghd"]}"></div>')
                continue
            badge = (f'<span class="b {"ok" if s["pass"] else "bad"}">{s["percent"]:.3f}% px / ≤{s["threshold"]}%</span>'
                     f'<span class="muted">{s.get("coverage", 0):.1f}% of 4pt blocks differ</span>')
            g, c, d = base + s["ghd"], base + s["corvane"], base + s["diff"]
            regions = "".join(
                f'<tr><td>{i}</td><td><code>{x["rect"]}</code></td><td>{x["share"]:.3f}%</td><td>{_e(x["hint"])}'
                + (f'<br><span class="sw" style="background:{x["ghd_color"]}"></span><code>{x["ghd_color"]}</code> → '
                   f'<span class="sw" style="background:{x["corvane_color"]}"></span><code>{x["corvane_color"]}</code>' if x["ghd_color"] else "")
                + f'</td><td><code>{_e(x["element"])}</code></td><td class="crop"><img loading="lazy" src="{base + x["crop"]}"></td></tr>'
                for i, x in enumerate(s["regions"], 1)
            )
            body.append(
                f'<div class="card snap"><div class="snaphead"><h3>{_e(s["name"])}</h3>{badge}'
                + (f'<span class="b warn">{_e(s["size_mismatch"])}</span>' if s["size_mismatch"] else "")
                + f'<span class="muted">{_e(s["note"])}</span><span class="modes">'
                '<button data-mode="diff" class="on">diff</button><button data-mode="pair">side by side</button>'
                '<button data-mode="swipe">swipe</button><button data-mode="blink">blink</button></span></div>'
                f'<div class="view" data-mode="diff"><img loading="lazy" src="{d}"></div>'
                f'<div class="view pair" data-mode="pair" hidden><figure><img loading="lazy" src="{g}"><figcaption>GitHub Desktop</figcaption></figure>'
                f'<figure><img loading="lazy" src="{c}"><figcaption>Corvane</figcaption></figure></div>'
                f'<div class="view" data-mode="swipe" hidden><div class="swipe"><img loading="lazy" src="{c}"><img class="top" loading="lazy" src="{g}"></div>'
                '<input type="range" min="0" max="100" value="50" aria-label="GitHub Desktop share"><div class="muted">left: GitHub Desktop · right: Corvane</div></div>'
                f'<div class="view" data-mode="blink" hidden><img loading="lazy" src="{g}" data-a="{g}" data-b="{c}"><div class="muted">GitHub Desktop</div></div>'
                + (f'<table><tr><th>#</th><th>Rect (pt)</th><th>Share</th><th>Hint</th><th>GHD element</th><th>GHD · Corvane · diff</th></tr>{regions}</table>' if regions else "")
                + "</div>"
            )
    body.append("</main>")
    page = (
        '<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">'
        f"<title>Parity report</title><style>{_CSS}</style></head><body>{''.join(body)}<script>{_JS}</script></body></html>"
    )
    (out / "index.html").write_text(page)
