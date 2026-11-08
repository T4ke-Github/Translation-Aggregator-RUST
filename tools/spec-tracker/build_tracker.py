"""Build tracker.html from the template and the extracted spec JSON.

Usage: python -I build_tracker.py <dash_dir> <out_html>
"""
import datetime
import json
import sys
from pathlib import Path

d = Path(sys.argv[1])
out = Path(sys.argv[2])
data = {
    "generated": datetime.date.today().isoformat(),
    "areas": json.loads((d / "out" / "areas.json").read_text(encoding="utf-8")),
    "features": [
        {k: f[k] for k in ("id", "area", "title", "priority", "milestone", "withdrawn")}
        for f in json.loads((d / "out" / "features.json").read_text(encoding="utf-8"))
    ],
    "criteria": [
        {k: c[k] for k in ("id", "feature", "area", "text", "verify", "withdrawn")}
        for c in json.loads((d / "out" / "criteria.json").read_text(encoding="utf-8"))
    ],
}
blob = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
# Keep the JSON inert inside <script type="application/json">.
blob = blob.replace("</", "<\\/").replace("<!--", "<\\!--")
tpl = (d / "tracker.template.html").read_text(encoding="utf-8")
assert tpl.count("__SPEC_DATA__") == 1
out.write_text(tpl.replace("__SPEC_DATA__", blob), encoding="utf-8")
print(out, f"{out.stat().st_size/1024:.0f} KB", len(data["features"]), "features", len(data["criteria"]), "criteria")
