"""Extract features and acceptance criteria from docs/specs/*.md into JSON rows.

Usage: python -I extract_specs.py <specs_dir> <out_dir>
"""
import json
import re
import sys
from pathlib import Path

specs_dir, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
out_dir.mkdir(parents=True, exist_ok=True)

FEAT_RE = re.compile(r"^###\s+([A-Z]+)-(\d+)\s+(.*?)\s*$")
AC_RE = re.compile(r"^\s*-\s+\*\*([A-Z]+-\d+\.\d+)\*\*\s*(.*)$")
META_RE = re.compile(r"\*\*(Priority|Milestone|Decisions|Legacy|Depends on):\*\*\s*(.*?)\s*(?=·\s*\*\*|$)")
VERIFY_RE = re.compile(r"\*\(Verify:\s*([^)]*)\)\*\s*$")
TITLE_RE = re.compile(r"^#\s+(.*)$")

features, criteria, areas = [], [], []


def clean(text):
    text = re.sub(r"\s+", " ", text).strip()
    return text


for path in sorted(specs_dir.glob("[0-9][0-9]-*.md")):
    lines = path.read_text(encoding="utf-8").splitlines()
    spec_title = next((TITLE_RE.match(l).group(1) for l in lines if TITLE_RE.match(l)), path.stem)
    spec_no = path.name[:2]
    placeholder = any("Status: not written yet" in l for l in lines[:12])
    prefix = None
    feat = None
    ac = None

    def close_ac():
        global ac
        if ac is not None:
            body = clean(ac["_buf"])
            m = VERIFY_RE.search(body)
            ac["verify"] = clean(m.group(1)) if m else ""
            ac["text"] = clean(VERIFY_RE.sub("", body))
            ac["withdrawn"] = bool(re.match(r"\W*withdrawn", body, re.I))
            del ac["_buf"]
            criteria.append(ac)
            ac = None

    in_code = False
    for line in lines:
        if line.strip().startswith("```"):
            in_code = not in_code
            continue
        if in_code:
            continue
        fm = FEAT_RE.match(line)
        if fm:
            close_ac()
            prefix = fm.group(1)
            title = fm.group(3)
            withdrawn = "withdrawn" in title.lower()
            feat = {
                "id": f"{fm.group(1)}-{fm.group(2)}",
                "area": fm.group(1),
                "spec": spec_no,
                "spec_title": spec_title,
                "spec_file": path.name,
                "title": re.sub(r"\s*[*(]*\s*withdrawn\s*[*)]*", "", title, flags=re.I).strip(),
                "priority": "", "milestone": "", "milestone_note": "", "decisions": "", "depends_on": "",
                "withdrawn": withdrawn,
            }
            features.append(feat)
            continue
        if line.startswith("## ") or line.startswith("### "):
            close_ac()
            if line.startswith("## "):
                feat = None
            continue
        if feat is not None and "**Priority:**" in line:
            for key, val in META_RE.findall(line):
                val = clean(val).rstrip("·").strip()
                if key == "Priority":
                    if "withdrawn" in val.lower():
                        feat["withdrawn"] = True
                        feat["priority"] = "Withdrawn"
                    else:
                        feat["priority"] = val.split()[0].strip("*") if val else ""
                elif key == "Milestone":
                    mm = re.match(r"(M\d)\s*(.*)", val)
                    feat["milestone"] = mm.group(1) if mm else val
                    feat["milestone_note"] = mm.group(2).strip(" ()") if mm else ""
                elif key == "Decisions":
                    feat["decisions"] = val
                elif key == "Depends on":
                    feat["depends_on"] = val
            continue
        am = AC_RE.match(line)
        if am and feat is not None:
            close_ac()
            ac = {"id": am.group(1), "feature": feat["id"], "area": feat["area"],
                  "_buf": am.group(2)}
            continue
        if ac is not None:
            if line.strip() == "":
                close_ac()
            elif line.startswith("  ") or not line.lstrip().startswith(("-", "#", "|", ">")):
                ac["_buf"] += " " + line.strip()
            else:
                close_ac()
    close_ac()
    tm = re.search(r"\(`([A-Z]+)`\)", spec_title)
    name = re.sub(r"^\d+\s+", "", re.sub(r"\s*\(`[A-Z]+`\)", "", spec_title)).strip()
    n = sum(1 for f in features if f["spec"] == spec_no)
    areas.append({"area": prefix or (tm.group(1) if tm else ""), "name": name, "spec": spec_no,
                  "file": path.name, "placeholder": placeholder})
    for f in features:
        if f["spec"] == spec_no:
            f["spec_title"] = name

# Attach feature attributes to criteria; count criteria per feature.
by_id = {f["id"]: f for f in features}
for c in criteria:
    f = by_id[c["feature"]]
    c["milestone"] = f["milestone"]
    c["priority"] = f["priority"]
    c["status"] = "Not started"
for f in features:
    f["criteria"] = sum(1 for c in criteria if c["feature"] == f["id"])
    f["status"] = "Not started"

(out_dir / "features.json").write_text(json.dumps(features, ensure_ascii=False, indent=0), encoding="utf-8")
(out_dir / "criteria.json").write_text(json.dumps(criteria, ensure_ascii=False, indent=0), encoding="utf-8")
(out_dir / "areas.json").write_text(json.dumps(areas, ensure_ascii=False, indent=0), encoding="utf-8")

# Report
from collections import Counter
print("specs:", [(a["spec"], a["area"], a["name"], "placeholder" if a["placeholder"] else "") for a in areas])
print("features:", len(features), "criteria:", len(criteria))
print("priorities:", Counter(f["priority"] for f in features))
print("milestones:", Counter(f["milestone"] for f in features))
print("verify:", Counter(c["verify"] for c in criteria).most_common(15))
bad = [f["id"] for f in features if not f["priority"] or not f["milestone"]]
print("features missing meta:", bad[:20])
print("features without criteria:", [f["id"] for f in features if f["criteria"] == 0][:20])
print("criteria without verify:", [c["id"] for c in criteria if not c["verify"]][:20])
print("max text len:", max(len(c["text"]) for c in criteria))
