#!/usr/bin/env python3
"""One-off migration: split the 573 KB single-file handbook into a served page.

- moves the 12 base64 guitar samples to web/assets/guitar-samples.js
- makes sample decoding lazy (first guitar switch), with visible status
- adds links to the new practice surfaces and a cloud section
- keeps the original inline CSS/JS immutable otherwise

Run from the repo root: python scripts/migrate-handbook.py
"""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "调律TEMPER.html"
OUT = ROOT / "web" / "index.html"
SAMPLES = ROOT / "web" / "assets" / "guitar-samples.js"

html = SRC.read_text(encoding="utf-8")

# 1. Extract the sample payload ------------------------------------------------
match = re.search(r"<script>\nconst GUITAR_SAMPLE_DATA = \{.*?\n</script>\n", html, re.S)
if not match:
    raise SystemExit("sample block not found")
block = match.group(0)
payload = block[len("<script>\n"):-len("\n</script>\n")]
SAMPLES.write_text(
    "/* FreePats Spanish Classical Guitar, CC0 1.0 — see README. */\n"
    "window.GUITAR_SAMPLE_DATA = " + payload.split("=", 1)[1].strip().rstrip(";") + ";\n",
    encoding="utf-8",
)
html = html.replace(block, "")

# 2. Replace eager sample loading with lazy loading + status -------------------
start = html.index("const GuitarSamples = {")
end = html.index("GuitarSamples.load();", start) + len("GuitarSamples.load();")
lazy = r"""const GuitarSamples = {
  buffers: new Map(), ready: false, loading: null, failed: false,
  load(){
    if (this.loading) return this.loading;
    const AC = window.OfflineAudioContext || window.webkitOfflineAudioContext;
    if (!AC){ this.failed = true; setSampleState('error'); return Promise.resolve(); }
    setSampleState('loading');
    this.loading = ensureGuitarSamples().then(() => {
      const data = window.GUITAR_SAMPLE_DATA;
      if (!data) throw new Error('missing sample data');
      const ctx = new AC(1, 1, 44100);
      const decode64 = (uri) => {
        const bin = atob(uri.slice(uri.indexOf(',') + 1));
        const bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        return bytes.buffer;
      };
      return Promise.all(Object.entries(data).map(async ([root, uri]) => {
        try {
          const buf = await ctx.decodeAudioData(decode64(uri));
          this.buffers.set(+root, buf);
        } catch(e){}
      }));
    }).then(() => {
      this.ready = this.buffers.size >= 6;
      this.failed = !this.ready;
      setSampleState(this.ready ? 'ready' : 'error');
    }).catch(() => {
      this.failed = true;
      setSampleState('error');
    }).finally(() => { this.loading = null; });
    return this.loading;
  },
  pick(midi){
    if (!this.ready) return null;
    let best = null, dist = 1e9;
    this.buffers.forEach((buf, root) => {
      const d = Math.abs(root - midi);
      if (d < dist){ dist = d; best = { root, buf, tune: GUITAR_TUNE[root] || 0 }; }
    });
    return best;
  }
};

/* Samples live in assets/guitar-samples.js and are fetched on first need. */
function ensureGuitarSamples(){
  if (window.GUITAR_SAMPLE_DATA) return Promise.resolve();
  if (window.__temperSamplesPromise) return window.__temperSamplesPromise;
  window.__temperSamplesPromise = new Promise((resolve, reject) => {
    const s = document.createElement('script');
    s.src = '/assets/guitar-samples.js';
    s.async = true;
    s.onload = () => resolve();
    s.onerror = () => reject(new Error('sample script failed'));
    document.head.appendChild(s);
  });
  return window.__temperSamplesPromise;
}

function setSampleState(state){
  const el = document.getElementById('sampleState');
  if (!el) return;
  const text = { loading:'采样加载中', ready:'采样就绪', error:'采样不可用 · 使用合成音' }[state] || '';
  el.textContent = text;
  el.dataset.state = state;
  el.hidden = false;
  if (state === 'ready') setTimeout(() => { el.hidden = true; }, 2800);
}

function ensureGuitarSamplesForMode(){
  if (Instrument.mode === 'guitar' && !GuitarSamples.ready && !GuitarSamples.loading) GuitarSamples.load();
}"""
html = html[:start] + lazy + html[end:]

# 3. Kick off loading when the guitar is selected ------------------------------
html = html.replace(
    "    Sound.setTimbre(mode);\n    this.listeners.forEach(fn => fn(mode));",
    "    Sound.setTimbre(mode);\n    if (mode === 'guitar') GuitarSamples.load();\n    this.listeners.forEach(fn => fn(mode));",
)
html = html.replace(
    "document.documentElement.dataset.inst = Instrument.mode;\nSound.setTimbre(Instrument.mode);",
    "document.documentElement.dataset.inst = Instrument.mode;\nSound.setTimbre(Instrument.mode);\nensureGuitarSamplesForMode();",
)

# 4. Navigation + sample status ------------------------------------------------
html = html.replace(
    '      <a class="nav__link" href="#staff">谱表</a>\n    </nav>',
    '      <a class="nav__link" href="#staff">谱表</a>\n'
    '      <a class="nav__link nav__link--app" href="/studio">工坊</a>\n'
    '      <a class="nav__link nav__link--app" href="/practice">练习</a>\n'
    '      <a class="nav__link nav__link--app" href="/profile">档案</a>\n'
    '    </nav>',
)
html = html.replace(
    '    <div class="nav__tools">\n      <div class="inst"',
    '    <div class="nav__tools">\n      <span class="sample-state" id="sampleState" hidden></span>\n      <div class="inst"',
)

# 5. Cloud section before </main> ---------------------------------------------
cloud = r"""
<!-- ================= CLOUD PRACTICE ================= -->
<section class="chapter chapter--surface" id="cloud" aria-labelledby="cloud-h">
  <div class="wrap">
    <div class="chapter__head">
      <div>
        <div class="chapter__meta"><span class="n">+</span><span class="rule"></span><span class="label">Practice loop</span></div>
        <h2 id="cloud-h">把看过的，<br>变成练过的</h2>
      </div>
      <div class="chapter__lead">
        <p>手册负责讲清楚，练习负责留下来。下面的三处共用同一份进度：练耳、节奏、读谱的每一题都会记到你的档案里，和弦工坊的进行可以存下来，也可以生成一个分享链接。</p>
      </div>
    </div>
    <div class="cloudgrid">
      <a class="card cloudcard" href="/practice">
        <span class="cloudcard__k">01 · PRACTICE</span>
        <h3>练耳与节奏</h3>
        <p>音程、和弦、音阶、读谱、拍点。服务端出题与判分，答错会给出可重听的解释。</p>
        <span class="cloudcard__go">开始一组 →</span>
      </a>
      <a class="card cloudcard" href="/studio">
        <span class="cloudcard__k">02 · STUDIO</span>
        <h3>和弦工坊</h3>
        <p>C++ 指法引擎一次给出多个可弹把位；把进行写成 4–8 小节，试听、保存、分享。</p>
        <span class="cloudcard__go">打开工坊 →</span>
      </a>
      <a class="card cloudcard" href="/profile">
        <span class="cloudcard__k">03 · ARCHIVE</span>
        <h3>练习档案</h3>
        <p>正确率、连对、薄弱音程与和弦、最近练习。清单进度与乐器偏好跨设备保留。</p>
        <span class="cloudcard__go">查看档案 →</span>
      </a>
    </div>
    <div class="cloudbar">
      <span class="cloudbar__dot" id="cloudDot" data-state="idle"></span>
      <span id="cloudStatus">正在连接练习服务…</span>
      <span class="cloudbar__spacer"></span>
      <span class="cloudbar__mono" id="deviceShort">—</span>
    </div>
  </div>
</section>
</main>"""
html = html.replace("</main>", cloud, 1)

# 5b. Mobile menu entries for the new pages --------------------------------
html = html.replace('  <div class="menu__foot">', '''  <div class="menu__apps">
    <a href="/studio"><span>和弦工坊</span><span>STUDIO</span></a>
    <a href="/practice"><span>练习</span><span>PRACTICE</span></a>
    <a href="/profile"><span>练习档案</span><span>ARCHIVE</span></a>
  </div>
''' + '  <div class="menu__foot">', 1)

# 6. Additions to the existing stylesheet ------------------------------------
style = r"""
<style id="temper-cloud-style">
.sample-state{font-family:var(--f-mono);font-size:10px;letter-spacing:.06em;color:var(--muted);border:1px solid var(--line);border-radius:999px;padding:5px 9px;white-space:nowrap;transition:color 160ms,border-color 160ms}
.sample-state[data-state="loading"]{color:var(--accent);border-color:rgba(156,82,39,.42)}
.sample-state[data-state="error"]{color:#9B3B2E;border-color:rgba(155,59,46,.42)}
.nav__link--app{position:relative}
.nav__link--app::after{content:"";position:absolute;left:12px;right:12px;bottom:3px;height:1px;background:var(--line-2);opacity:.55}
.cloudgrid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:var(--gap);margin-top:var(--s6)}
.cloudcard{display:flex;flex-direction:column;gap:10px;padding:26px 24px 22px;text-decoration:none;color:inherit;transition:transform 180ms var(--ease),border-color 180ms var(--ease)}
.cloudcard:hover{transform:translateY(-3px);border-color:var(--line-2)}
.cloudcard__k{font-family:var(--f-mono);font-size:10px;letter-spacing:.12em;color:var(--accent)}
.cloudcard h3{font-family:var(--f-display);font-size:26px;font-weight:400;letter-spacing:-.01em}
.cloudcard p{font-size:13.5px;line-height:1.62;color:var(--muted);flex:1}
.cloudcard__go{font-family:var(--f-mono);font-size:11px;letter-spacing:.04em;color:var(--ink)}
.cloudbar{display:flex;align-items:center;gap:10px;margin-top:var(--s5);padding-top:18px;border-top:1px solid var(--line);font-family:var(--f-mono);font-size:10.5px;letter-spacing:.04em;color:var(--muted)}
.cloudbar__dot{width:7px;height:7px;border-radius:50%;background:var(--line-2)}
.cloudbar__dot[data-state="ok"]{background:var(--accent-2)}
.cloudbar__dot[data-state="error"]{background:#9B3B2E}
.cloudbar__spacer{flex:1}
.menu__apps{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px;padding:14px 22px 0}
.menu__apps a{display:flex;flex-direction:column;gap:2px;padding:10px 12px;border:1px solid var(--line);border-radius:8px;text-decoration:none;color:var(--ink);font-size:13px}
.menu__apps a span:last-child{font-family:var(--f-mono);font-size:8.5px;letter-spacing:.1em;color:var(--muted)}
@media (max-width:899px){.cloudgrid{grid-template-columns:1fr}}
@media (max-width:1099px){.sample-state{display:none}}
</style>
"""
html = html.replace("</head>", style + "</head>", 1)

# 7. Stable ids for the eight checklist items --------------------------------
items = ["c4-a4", "fifth-tritone", "sing-scale", "key-signatures", "cfgc", "count-4-4", "staff-first-line", "play-widget"]
counter = {"i": 0}
def add_id(m):
    idx = counter["i"]
    counter["i"] += 1
    return f'<input type="checkbox" data-item="{items[idx]}">'
html = re.sub(r'<input type="checkbox">', add_id, html, count=8)

# 8. Bridge module ------------------------------------------------------------
html = html.replace(
    "</body>",
    '<!-- Talks to the practice server; safe when the page is opened offline. -->\n'
    '<script type="module" src="/js/handbook.js"></script>\n</body>',
)

OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_text(html, encoding="utf-8")
print(f"wrote {OUT} ({len(html)} bytes)")
print(f"wrote {SAMPLES} ({SAMPLES.stat().st_size} bytes)")
