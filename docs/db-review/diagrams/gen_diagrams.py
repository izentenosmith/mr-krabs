"""Generate Barker ER + relational draw.io diagrams from the target schema (introspected via sqlite3)."""
import re, sqlite3, sys
from xml.sax.saxutils import escape

SCHEMAS, OUT = sys.argv[1:-1], sys.argv[-1]
sql = "\n".join(open(f).read() for f in SCHEMAS)
db = sqlite3.connect(":memory:")
db.executescript(sql)

SYNC_COLS = {"hlc", "hlc_device", "deleted"}
LOCAL_ONLY = {"local_setting", "device", "import_row", "import_row_candidate", "import_budget_row", "change_log", "field_clock", "peer_state", "sync_conflict"}
SYNC_META = {"change_log", "field_clock", "peer_state", "sync_conflict", "local_setting"}

tables = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY rowid")]
cols = {t: db.execute(f"PRAGMA table_info({t})").fetchall() for t in tables}  # cid,name,type,notnull,dflt,pk
fks = {t: db.execute(f"PRAGMA foreign_key_list({t})").fetchall() for t in tables}  # id,seq,table,from,to,...
tsql = {t: db.execute("SELECT sql FROM sqlite_master WHERE name=?", (t,)).fetchone()[0] for t in tables}

def unique_keys(t):
    """Return list of (label, [cols], where) for unique indexes / constraints (excluding PK)."""
    out = []
    for _, name, uniq, origin, partial in db.execute(f"PRAGMA index_list({t})"):
        if not uniq or origin == "pk":
            continue
        icols = [r[2] if r[2] is not None else "<expr>" for r in db.execute(f"PRAGMA index_info({name})")]
        isql = db.execute("SELECT sql FROM sqlite_master WHERE name=?", (name,)).fetchone()
        where = ""
        if isql and isql[0]:
            m = re.search(r"\bWHERE\b(.*)$", isql[0], re.S)
            where = m.group(1).strip() if m else ""
            m2 = re.search(r"\((.*)\)\s*(WHERE|$)", isql[0], re.S)
            if "<expr>" in icols and m2:
                icols = [m2.group(1).strip()]
        out.append((name, icols, where))
    return out

def checks(t):
    s = tsql[t]
    res, i = [], 0
    while True:
        j = s.find("CHECK (", i)
        if j < 0:
            break
        k, depth = j + 6, 0
        while k < len(s):
            if s[k] == "(":
                depth += 1
            elif s[k] == ")":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        expr = " ".join(s[j + 6:k + 1].split())
        res.append(expr)
        i = k
    # drop trivial per-column checks that the row already shows (booleans/deleted)
    return [c for c in res if not re.fullmatch(r"\(\w+ IN \(0,1\)\)", c)]

ENT = lambda t: t.replace("_", " ").upper()

# ---------- role names (child role, parent role) ----------
ROLES = {
    ("fx_rate", "base_code"): ("quoted from", "base of"),
    ("fx_rate", "quote_code"): ("quoted into", "quote of"),
    ("member", "household_id"): ("part of", "composed of"),
    ("device", "member_id"): ("used by", "user of"),
    ("household", "base_currency_code"): ("reported in", "base of"),
    ("account", "currency_code"): ("denominated in", "denomination of"),
    ("account", "kind"): ("classified as", "classification of"),
    ("journal_entry", "kind"): ("classified as", "classification of"),
    ("recurring_rule", "payee_id"): ("paid to", "payee of"),
    ("recurring_rule", "payer_member_id"): ("paid by", "payer of"),
    ("recurring_rule", "attributed_member_id"): ("attributed to", "attributed"),
    ("recurring_rule_posting", "rule_id"): ("template leg of", "templated by"),
    ("recurring_rule_posting", "account_id"): ("booked to", "template target of"),
    ("recurring_rule_posting", "category_id"): ("booked to", "template target of"),
    ("recurring_rule_share", "rule_id"): ("template share of", "split into"),
    ("recurring_rule_share", "member_id"): ("borne by", "bearer of"),
    ("recurring_rule_share", "participant_id"): ("borne by", "bearer of"),
    ("account", "owner_member_id"): ("owned by", "owner of"),
    ("account", "linked_account_id"): ("section of", "parent of"),
    ("account_valuation", "account_id"): ("of", "valued by"),
    ("category", "parent_id"): ("placed under", "parent of"),
    ("category", "merged_into_id"): ("merged into", "target of"),
    ("rule_condition", "rule_id"): ("part of", "guarded by"),
    ("rule_condition", "payee_id"): ("tests", "tested by"),
    ("rule_condition", "account_id"): ("tests", "tested by"),
    ("rule_action", "rule_id"): ("part of", "source of"),
    ("rule_action", "category_id"): ("setter of", "set by"),
    ("rule_action", "payee_id"): ("setter of", "set by"),
    ("rule_action", "tag_id"): ("adder of", "added by"),
    ("import_row", "transfer_account_id"): ("transfer to", "target of"),
    ("import_row_candidate", "row_id"): ("about", "matched by"),
    ("import_row_candidate", "entry_id"): ("match for", "candidate in"),
    ("import_budget_row", "batch_id"): ("part of", "composed of"),
    ("import_budget_row", "category_id"): ("resolved to", "resolution of"),
    ("import_category_mapping", "profile_id"): ("remembered for", "remembering"),
    ("import_category_mapping", "category_id"): ("resolved to", "resolution of"),
    ("payee", "default_category_id"): ("defaulted to", "default of"),
    ("installment_plan", "account_id"): ("charged to", "charged"),
    ("recurring_rule", "currency_code"): ("denominated in", "denomination of"),
    ("import_batch", "account_id"): ("for", "imported by"),
    ("import_batch", "profile_id"): ("read with", "used by"),
    ("journal_entry", "payee_id"): ("paid to", "paid in"),
    ("journal_entry", "payer_member_id"): ("paid by", "payer of"),
    ("journal_entry", "attributed_member_id"): ("attributed to", "attributed"),
    ("journal_entry", "installment_plan_id"): ("purchase of", "recorded by"),
    ("journal_entry", "recurring_rule_id"): ("generated by", "generator of"),
    ("journal_entry", "import_batch_id"): ("committed from", "source of"),
    ("installment", "plan_id"): ("part of", "scheduled as"),
    ("installment", "paid_entry_id"): ("paid by", "payment of"),
    ("reconciliation", "account_id"): ("of", "reconciled by"),
    ("posting", "entry_id"): ("leg of", "composed of"),
    ("posting", "account_id"): ("booked to", "booked by"),
    ("posting", "category_id"): ("booked to", "booked by"),
    ("posting", "currency_code"): ("denominated in", "denomination of"),
    ("posting", "reconciliation_id"): ("cleared by", "clearing of"),
    ("entry_share", "entry_id"): ("share of", "split into"),
    ("entry_share", "member_id"): ("borne by", "bearer of"),
    ("entry_share", "participant_id"): ("borne by", "bearer of"),
    ("entry_share", "currency_code"): ("denominated in", "denomination of"),
    ("entry_share", "settled_entry_id"): ("settled by", "settlement of"),
    ("entry_tag", "entry_id"): ("about", "tagged by"),
    ("entry_tag", "tag_id"): ("use of", "used in"),
    ("payee_rule", "account_id"): ("limited to", "scope of"),
    ("payee_rule", "set_payee_id"): ("setter of", "set by"),
    ("payee_rule", "set_category_id"): ("setter of", "set by"),
    ("budget_line", "category_id"): ("for", "budgeted by"),
    ("budget_line", "member_id"): ("for", "budgeted by"),
    ("savings_goal", "account_id"): ("held in", "holder of"),
    ("import_row", "batch_id"): ("part of", "composed of"),
    ("import_row", "suggested_category_id"): ("suggested", "suggested for"),
    ("import_row", "suggested_payee_id"): ("suggested", "suggested for"),
    ("import_row", "duplicate_of_entry_id"): ("duplicate of", "duplicated by"),
    ("attachment", "entry_id"): ("attached to", "documented by"),
}
# Parent ends that the model says are *must* (enforced in Rust, annotated with rule ids)
MUST_PARENT = {("posting", "entry_id"): "≥2 legs (Rust: validate_balanced)",
               ("recurring_rule_posting", "rule_id"): "≥2 legs (Rust)",
               ("rule_action", "rule_id"): "≥1 action (Rust)",
               ("installment", "plan_id"): "schedule (Rust)",
               ("journal_entry", "installment_plan_id"): "1:1 (A-04)"}
# 1:1 relationships (parent end is "one", not crow's foot)
ONE_TO_ONE = {("journal_entry", "installment_plan_id")}

# Currency FKs are numerous; keep the ER readable by drawing them, but they are reference data.
# Layout: (x, y) per entity, columns by domain.
LAYOUT = {
    "currency": (40, 20), "fx_rate": (40, 340), "household": (40, 560), "member": (40, 790), "device": (40, 1060),
    "account": (420, 40), "account_valuation": (420, 460), "reconciliation": (420, 660),
    "installment_plan": (420, 880), "installment": (420, 1160), "savings_goal": (420, 1380), "account_kind": (420, 1580),
    "journal_entry": (820, 40), "posting": (820, 470), "entry_share": (820, 800), "external_participant": (820, 1060),
    "entry_tag": (820, 1230), "tag": (820, 1400), "attachment": (820, 1560), "entry_kind": (820, 1740),
    "category": (1220, 40), "payee": (1220, 560), "payee_rule": (1220, 760),
    "budget_line": (1220, 1040), "recurring_rule": (1220, 1320), "recurring_rule_posting": (1220, 1720), "recurring_rule_share": (1220, 1960),
    "import_profile": (1620, 40), "import_batch": (1620, 280), "import_row": (1620, 640),
    "import_row_candidate": (1620, 1060), "import_budget_row": (1620, 1220), "import_category_mapping": (1620, 1480),
    "rule": (2020, 40), "rule_condition": (2020, 300), "rule_action": (2020, 700),
    "local_setting": (2020, 1060), "change_log": (2020, 1180), "field_clock": (2020, 1440),
    "peer_state": (2020, 1600), "sync_conflict": (2020, 1720),
}

def cell(id_, value, style, x=None, y=None, w=None, h=None, parent="1", vertex=True, source=None, target=None, points=None):
    v = escape(value, {'"': "&quot;"})
    if vertex:
        return (f'<mxCell id="{id_}" value="{v}" style="{style}" vertex="1" parent="{parent}">'
                f'<mxGeometry x="{x}" y="{y}" width="{w}" height="{h}" as="geometry"/></mxCell>')
    pts = ""
    if points:
        pts = '<Array as="points">' + "".join(f'<mxPoint x="{px}" y="{py}"/>' for px, py in points) + "</Array>"
    return (f'<mxCell id="{id_}" value="{v}" style="{style}" edge="1" parent="{parent}" source="{source}" target="{target}">'
            f'<mxGeometry relative="1" as="geometry">{pts}</mxGeometry></mxCell>')

def wrap(name, cells, w=2100, h=2000):
    body = "".join(cells)
    return (f'<mxfile host="dbdiagram" version="24.0.0"><diagram id="{name}" name="{name}">'
            f'<mxGraphModel dx="1600" dy="1000" grid="1" gridSize="10" guides="1" tooltips="1" connect="1" arrows="1" '
            f'fold="1" page="1" pageScale="1" pageWidth="{w}" pageHeight="{h}" math="0" shadow="0"><root>'
            f'<mxCell id="0"/><mxCell id="1" parent="0"/>{body}</root></mxGraphModel></diagram></mxfile>')

# ======================= Barker ER =======================
er = []
er.append(cell("er-title", "Barker ER Model — Mr Krabs target schema (design rev. 3, 2026-11-22, requirements at PR 14)",
               "text;fontSize=20;fontStyle=1;align=left;verticalAlign=middle;", 40, -40, 900, 30))
ENTW = 300
ent_h = {}
for t in tables:
    if t not in LAYOUT:
        raise SystemExit(f"no layout for {t}")
    fkcols = {f[3] for f in fks[t]}
    lines = [f"<b>{ENT(t)}</b>"]
    for c in cols[t]:
        name, notnull, pk = c[1], c[3], c[5]
        if name in SYNC_COLS or (name in fkcols and not pk):
            continue
        if pk and name in fkcols:
            continue  # part of UID via relationship (UID bar)
        mark = "#" if pk else ("*" if notnull else "o")
        lines.append(f"{mark} {name}")
    if t == "account":
        lines.append("<i>subtype CREDIT CARD: credit_limit_minor, statement_day, due_day (D-09)</i>")
    if t == "posting":
        lines.append("<i>arc A1 on leg: ACCOUNT | CATEGORY | fx (no target)</i>")
    if t == "entry_share":
        lines.append("<i>arc A2: MEMBER | EXTERNAL PARTICIPANT</i>")
    if t == "category":
        lines.append("<i>tree of any depth; child kind = parent kind (declared); merged_into only when deleted</i>")
    if t in ("account_kind", "entry_kind"):
        lines.append("<i>code-owned reference list, installed by migrations, not synced</i>")
    if t == "recurring_rule_posting":
        lines.append("<i>arc on leg: ACCOUNT | CATEGORY | fx</i>")
    if t == "rule_action":
        lines.append("<i>arc on action: CATEGORY | PAYEE | TAG | text</i>")
    if t in ("rule", "rule_condition", "rule_action", "import_row_candidate", "import_budget_row", "import_category_mapping", "account_valuation"):
        lines.append("<i>planned (M4/M5) — schema-planned.sql</i>")
    if t in SYNC_META:
        lines.append("<i>sync/local metadata — no business relationships</i>")
    h = 30 + 16 * len(lines) + 16 * sum(1 for l in lines if l.startswith('<i>'))
    ent_h[t] = h
    x, y = LAYOUT[t]
    fill = "#f5f5f5" if t in LOCAL_ONLY else "#dae8fc"
    er.append(cell(f"er-{t}", "<br>".join(lines),
                   f"rounded=1;arcSize=8;whiteSpace=wrap;html=1;align=left;verticalAlign=top;spacingLeft=8;fillColor={fill};strokeColor=#6c8ebf;fontSize=11;",
                   x, y, ENTW, h))

def center(t):
    x, y = LAYOUT[t]
    return x + ENTW / 2, y + ent_h[t] / 2

for t in tables:
    pkcols = {c[1] for c in cols[t] if c[5]}
    groups = {}
    for f in fks[t]:
        groups.setdefault(f[0], []).append(f)
    for fid, fl in groups.items():
        parent = fl[0][2]
        fcols = [f[3] for f in fl]
        if parent == "currency":
            continue  # reference relationships to CURRENCY listed in a note (legibility)
        # composite FKs that only add currency/account consistency are drawn once via their leading column
        lead = fcols[0]
        if len(fcols) > 1 and any((t, lead) == k for k in ROLES) is False:
            pass
        if len(fcols) > 1:
            # (account_id, currency_code) → account ; (reconciliation_id, account_id) → reconciliation
            lead = fcols[0]
        rid = f"er-rel-{t}-{lead}"
        cr, pr = ROLES.get((t, lead), ("references", "referenced by"))
        notnull = all(next(c for c in cols[t] if c[1] == col)[3] for col in fcols)
        child_must = notnull
        parent_must = (t, lead) in MUST_PARENT
        in_uid = set(fcols) <= pkcols and len(pkcols) > 1
        cx, cy = center(t)
        px, py = center(parent)
        if parent == t:
            jx, jy = LAYOUT[t][0] + ENTW + 50, LAYOUT[t][1] + (40 if lead == "parent_id" else ent_h[t] - 30)
        else:
            jx, jy = (cx + px) / 2, (cy + py) / 2
        er.append(cell(f"{rid}-j", "", "shape=waypoint;fillStyle=solid;size=2;strokeColor=none;fillColor=none;",
                       jx - 1, jy - 1, 2, 2))
        # child end: one or more (crow's foot); UID bar if FK participates in the PK
        child_arrow = "ERoneToMany" if in_uid else "ERmany"
        if (t, lead) in ONE_TO_ONE:
            child_arrow = "none"
        er.append(cell(f"{rid}-child", cr,
                       f"endArrow={child_arrow};startArrow=none;endFill=0;dashed={0 if child_must else 1};html=1;fontSize=10;strokeColor=#333333;labelBackgroundColor=#ffffff;",
                       vertex=False, source=f"{rid}-j", target=f"er-{t}"))
        plabel = pr + (f" [{MUST_PARENT[(t, lead)]}]" if parent_must else "")
        er.append(cell(f"{rid}-parent", plabel,
                       f"endArrow=none;startArrow=none;dashed={0 if parent_must else 1};html=1;fontSize=10;strokeColor=#333333;labelBackgroundColor=#ffffff;",
                       vertex=False, source=f"{rid}-j", target=f"er-{parent}"))

# Legend
lg = [cell("er-legend", "Legend — Barker notation", "swimlane;startSize=24;fontStyle=1;fillColor=#fff2cc;strokeColor=#d6b656;container=1;",
           40, 2200, 1180, 230)]
items = [
    ("solid half-line = must be (mandatory at that end)", "endArrow=none;dashed=0;"),
    ("dashed half-line = may be (optional at that end)", "endArrow=none;dashed=1;"),
    ("crow's foot (ERmany) = one or more; plain end = one and only one", "endArrow=ERmany;dashed=0;"),
    ("ERoneToMany = UID bar: relationship is part of the child's unique identifier", "endArrow=ERoneToMany;dashed=0;"),
]
for i, (txt, st) in enumerate(items):
    y = 34 + i * 34
    lg.append(cell(f"er-legend-s{i}", "", st + "html=1;", vertex=False, source=None, target=None, parent="er-legend")
              .replace('<mxGeometry relative="1" as="geometry">',
                       f'<mxGeometry relative="1" as="geometry"><mxPoint x="20" y="{y}" as="sourcePoint"/><mxPoint x="120" y="{y}" as="targetPoint"/>')
              .replace(' source="None" target="None"', ""))
    lg.append(cell(f"er-legend-t{i}", txt, "text;align=left;verticalAlign=middle;fontSize=11;", 140, y - 10, 1000, 20, parent="er-legend"))
lg.append(cell("er-legend-attr", "Attributes: # = UID (primary identifier) · * = mandatory · o = optional. FK columns are drawn as relationships, "
               "sync columns (hlc, hlc_device, deleted) omitted. Grey = local-only (never synced). Each relationship = two half-lines meeting at an "
               "invisible junction; role names read from the near entity. [..] on a parent end = a must the schema does not enforce (rule owner).",
               "text;align=left;verticalAlign=top;whiteSpace=wrap;fontSize=11;", 20, 170, 1140, 50, parent="er-legend"))
er += lg
cur_children = sorted({t for t in tables for f in fks[t] if f[2] == "currency"})
er.append(cell("er-currency-note", "CURRENCY relationships (not drawn, for legibility): each " + ", ".join(ENT(t) for t in cur_children) +
               " must be (or, where the column is nullable, may be) denominated in one and only one CURRENCY; FX RATE has two (quoted from / quoted into); "
               "HOUSEHOLD must be reported in one CURRENCY. Each CURRENCY may be the denomination of one or more of each.",
               "text;align=left;verticalAlign=top;whiteSpace=wrap;fontSize=11;fillColor=#fff2cc;strokeColor=#d6b656;spacing=6;", 40, 170 + 1, 300, 150))
er.append(cell("er-provenance",
               "Provenance: docs/db-review/design/schema.sql + schema-planned.sql + design.md rev. 3 (target, not yet migrated; planned tables marked). Relationships derived from declared FKs "
               "(PRAGMA foreign_key_list); optionality at the child end from NOT NULL; parent-end musts from design §8 rules. "
               "Inferred/open: 1:1 JOURNAL ENTRY–INSTALLMENT PLAN (A-04/Q-06); composite FKs (account_id, currency_code) and "
               "(reconciliation_id, account_id) drawn once on their leading column; CURRENCY relationships summarized in a note. Source notation: SQL DDL (no glyph translation).",
               "text;align=left;verticalAlign=top;whiteSpace=wrap;fontSize=11;fillColor=#f8cecc;strokeColor=#b85450;spacing=6;",
               1260, 2200, 760, 120))
open(f"{OUT}/er-model.drawio", "w").write(wrap("Barker ER Model", er, 2400, 2500))

# ======================= Relational =======================
rel = [cell("rel-title", "Relational Model — Mr Krabs target schema rev. 3 (SQLite, STRICT; PR 14 requirements)",
            "text;fontSize=20;fontStyle=1;align=left;verticalAlign=middle;", 40, -40, 900, 30)]
RW, ROWH = 340, 18
REL_LAYOUT = {}
# reuse column layout but with more vertical room: stack per column
colx = {40: 40, 420: 430, 820: 820, 1220: 1210, 1620: 1600, 2020: 1990}
ystack = {}
order = sorted(tables, key=lambda t: (LAYOUT[t][0], LAYOUT[t][1]))
row_ids = {}
for t in order:
    x = colx[LAYOUT[t][0]]
    y = ystack.get(x, 0)
    fkcols = {}
    for i, f in enumerate(fks[t]):
        fkcols.setdefault(f[3], []).append(f"FK{f[0] + 1}")
    pkcols = {c[1] for c in cols[t] if c[5]}
    aks = unique_keys(t)
    akmap = {}
    extra = []
    for n, (iname, icols, where) in enumerate(aks, 1):
        lab = f"AK{n}"
        for c in icols:
            akmap.setdefault(c, []).append(lab)
        if where or any("(" in c or " " in c for c in icols):
            extra.append(f"{lab}: UNIQUE ({', '.join(icols)})" + (f" WHERE {where}" if where else ""))
    chk = checks(t)
    rows = []
    for c in cols[t]:
        name, typ, notnull = c[1], c[2], c[3]
        keys = (["PK"] if name in pkcols else []) + fkcols.get(name, []) + [a for a in akmap.get(name, [])]
        rows.append((name, ",".join(keys), f"{typ} {'NOT NULL' if notnull or name in pkcols else 'NULL'}"))
    notes = extra + [f"CHECK {c}" for c in chk]
    note_lines = sum(-(-len(n) // 72) for n in notes)
    h = 26 + ROWH * len(rows) + 11 * note_lines + (12 if notes else 0)
    title = t + ("  (local-only)" if t in LOCAL_ONLY else "")
    tid = f"rel-{t}"
    rel.append(cell(tid, title, "shape=table;startSize=26;container=1;collapsible=0;childLayout=tableLayout;fixedRows=1;rowLines=0;"
                    "fontStyle=1;align=center;resizeLast=1;html=1;fillColor=" + ("#f5f5f5" if t in LOCAL_ONLY else "#d5e8d4") + ";strokeColor=#82b366;fontSize=12;",
                    x, y, RW, h))
    for i, (name, keys, typ) in enumerate(rows):
        rid = f"rel-{t}-{name}"
        row_ids[(t, name)] = rid
        rel.append(cell(rid, "", "shape=tableRow;horizontal=0;startSize=0;swimlaneHead=0;swimlaneBody=0;fillColor=none;collapsible=0;"
                        "dropTarget=0;points=[[0,0.5],[1,0.5]];portConstraint=eastwest;top=0;left=0;right=0;bottom=0;",
                        0, 26 + i * ROWH, RW, ROWH, parent=tid))
        bold = "fontStyle=1;" if "PK" in keys else ""
        rel.append(cell(f"{rid}-k", keys, "shape=partialRectangle;connectable=0;fillColor=none;top=0;left=0;bottom=0;right=0;fontSize=9;align=left;spacingLeft=3;" + bold,
                        0, 0, 70, ROWH, parent=rid))
        rel.append(cell(f"{rid}-n", name, "shape=partialRectangle;connectable=0;fillColor=none;top=0;left=0;bottom=0;right=0;fontSize=10;align=left;spacingLeft=3;" + bold,
                        70, 0, 150, ROWH, parent=rid))
        rel.append(cell(f"{rid}-t", typ, "shape=partialRectangle;connectable=0;fillColor=none;top=0;left=0;bottom=0;right=0;fontSize=9;align=left;spacingLeft=3;fontColor=#555555;",
                        220, 0, 120, ROWH, parent=rid))
    if notes:
        ny = 26 + ROWH * len(rows) + 4
        rel.append(cell(f"{tid}-constraints", "<br>".join(escape(n) for n in notes),
                        "text;html=1;align=left;verticalAlign=top;whiteSpace=wrap;fontSize=8;fontColor=#7a4f01;spacingLeft=4;overflow=hidden;",
                        0, ny, RW, 11 * note_lines + 8, parent=tid))
    ystack[x] = y + h + 30

for t in tables:
    groups = {}
    for f in fks[t]:
        groups.setdefault(f[0], []).append(f)
    for fid, fl in groups.items():
        parent = fl[0][2]
        fcols = [f[3] for f in fl]
        label = f"FK{fid + 1} (" + ", ".join(fcols) + ")" + (" → (" + ", ".join(f[4] for f in fl) + ")" if len(fl) > 1 else "")
        rel.append(cell(f"rel-fk-{t}-{'-'.join(fcols)}", label,
                        "edgeStyle=entityRelationEdgeStyle;endArrow=ERone;startArrow=ERmany;html=1;fontSize=8;strokeColor=#6d8764;labelBackgroundColor=#ffffff;curved=0;",
                        vertex=False, source=row_ids[(t, fcols[0])], target=f"rel-{parent}"))

rel.append(cell("rel-legend", "Legend", "swimlane;startSize=24;fontStyle=1;fillColor=#fff2cc;strokeColor=#d6b656;container=1;",
                40, max(ystack.values()) + 20, 900, 110))
rel.append(cell("rel-legend-t", "PK = primary key · FKn = foreign key n (edge from the FK column to the referenced table; composite FKs list both columns) · "
                "AKn = alternate key n (UNIQUE / unique index; partial ones restricted to live rows are spelled out under the table) · "
                "CHECK lines = table and column checks (boolean 0/1 checks omitted) · grey = local-only table. Types are SQLite STRICT storage classes.",
                "text;align=left;verticalAlign=top;whiteSpace=wrap;fontSize=11;", 10, 30, 880, 70, parent="rel-legend"))
rel.append(cell("rel-provenance", "Provenance: generated from docs/db-review/design/schema.sql + schema-planned.sql by introspection (PRAGMA table_info / foreign_key_list / "
                "index_list) on SQLite in-memory; identifiers match the ER model (er-<table> ↔ rel-<table>). Target design, not the live 0001 schema.",
                "text;align=left;verticalAlign=top;whiteSpace=wrap;fontSize=11;fillColor=#f8cecc;strokeColor=#b85450;spacing=6;",
                980, max(ystack.values()) + 20, 900, 70))
open(f"{OUT}/relational-model.drawio", "w").write(wrap("Relational Model", rel, 2400, int(max(ystack.values()) + 200)))
print("ok", len(tables), "tables")
