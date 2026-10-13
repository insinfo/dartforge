# Parity table from the oracle's JSONL.
# Usage: python summary.py <results.jsonl> [--examples N] [--method M]
import json, sys, re, collections, urllib.parse

args = sys.argv[1:]
path = args[0]
N_EX = int(args[args.index("--examples") + 1]) if "--examples" in args else 2
ONLY = args[args.index("--method") + 1] if "--method" in args else None


def nu(u):
    return urllib.parse.unquote(u or "").lower()


def rg(r):
    if not r:
        return None
    return (r["start"]["line"], r["start"]["character"], r["end"]["line"], r["end"]["character"])


def error(x):
    return isinstance(x, dict) and "error" in x


def locations(x):
    if x is None or error(x):
        return frozenset()
    if isinstance(x, dict):
        x = [x]
    s = set()
    for l in x:
        if "targetUri" in l:
            s.add((nu(l["targetUri"]), rg(l["targetSelectionRange"])))
        else:
            s.add((nu(l["uri"]), rg(l["range"])))
    return frozenset(s)


def hover_text(x):
    if not x or error(x):
        return None
    c = x.get("contents")
    if isinstance(c, dict):
        c = c.get("value")
    elif isinstance(c, list):
        c = "\n".join(i if isinstance(i, str) else i.get("value", "") for i in c)
    return re.sub(r"\s+", " ", c or "").strip() or None


def hover_signature(x):
    t = hover_text(x)
    if t is None:
        return None
    m = re.search(r"```dart (.*?) ```", t)
    return m.group(1) if m else t


def edits(x):
    if not x or error(x):
        return None if not error(x) else "ERROR"
    s = set()
    for dc in x.get("documentChanges") or []:
        if "textDocument" in dc:
            for e in dc["edits"]:
                s.add((nu(dc["textDocument"]["uri"]), rg(e["range"]), e["newText"]))
        else:
            s.add((dc.get("kind"), nu(dc.get("oldUri") or dc.get("uri")), nu(dc.get("newUri"))))
    for u, es in (x.get("changes") or {}).items():
        for e in es:
            s.add((nu(u), rg(e["range"]), e["newText"]))
    return frozenset(s)


def edit_by_title(x):
    """By title of the `quickfix`/`refactor` actions with an `edit`: the
    normalized edits, with the `insertTextFormat` (2 = snippet)."""
    if not x or error(x):
        return {}
    output = {}
    for action in x:
        k = action.get("kind") or ""
        if not (k.startswith("quickfix") or k.startswith("refactor")) or "edit" not in action:
            continue
        s = set()
        for dc in action["edit"].get("documentChanges") or []:
            if "textDocument" in dc:
                for e in dc["edits"]:
                    s.add((nu(dc["textDocument"]["uri"]), rg(e["range"]), e["newText"], e.get("insertTextFormat")))
            else:
                s.add((dc.get("kind"), nu(dc.get("oldUri") or dc.get("uri")), nu(dc.get("newUri"))))
        for u, es in (action["edit"].get("changes") or {}).items():
            for e in es:
                s.add((nu(u), rg(e["range"]), e["newText"], e.get("insertTextFormat")))
        output.setdefault(action.get("title"), frozenset(s))
    return output


def prepare(x):
    if error(x):
        return "ERROR"
    if not x:
        return None
    if "range" in x:
        return (rg(x["range"]), x.get("placeholder"))
    return rg(x)


def actions(x):
    if not x or error(x):
        return frozenset()
    return frozenset((a.get("title"), a.get("kind")) for a in x)


def titles(x):
    if not x or error(x):
        return frozenset()
    return frozenset(a.get("title") for a in x)


def selection(x):
    if not x or error(x):
        return None
    output = []
    for s in x:
        chain = []
        while s:
            chain.append(rg(s["range"]))
            s = s.get("parent")
        output.append(tuple(chain))
    return tuple(output)


def hierarchy(x):
    if not x or error(x):
        return frozenset()
    return frozenset((i["name"], i["kind"], nu(i["uri"]), rg(i["selectionRange"])) for i in x)


def highlights(x):
    if not x or error(x):
        return frozenset()
    return frozenset(rg(h["range"]) for h in x)


def symbols(x):
    if not x or error(x):
        return frozenset()
    s = set()

    def walk(l, parent):
        for i in l:
            r = i.get("selectionRange") or (i.get("location") or {}).get("range")
            s.add((i["name"], i["kind"], r["start"]["line"] if r else None))
            walk(i.get("children") or [], i["name"])
    walk(x, None)
    return frozenset(s)


def folds(x):
    if not x or error(x):
        return frozenset()
    return frozenset((f["startLine"], f["endLine"]) for f in x)


def hints(x):
    if not x or error(x):
        return frozenset()
    s = set()
    for h in x:
        l = h["label"]
        if isinstance(l, list):
            l = "".join(p["value"] for p in l)
        s.add((h["position"]["line"], h["position"]["character"], l))
    return frozenset(s)


def links(x):
    if not x or error(x):
        return frozenset()
    return frozenset((rg(l["range"]), nu(l.get("target"))) for l in x)


def ws(x):
    if not x or error(x):
        return frozenset()
    return frozenset((i["name"], nu((i.get("location") or {}).get("uri"))) for i in x)


def signature(x):
    if not x or error(x) or not x.get("signatures"):
        return None
    a = x.get("activeSignature") or 0
    s = x["signatures"][min(a, len(x["signatures"]) - 1)]
    ap = s.get("activeParameter", x.get("activeParameter"))
    # The active parameter diverges on purpose (see docs/LSP.md): only the label.
    return s["label"]


def label(i):
    return re.sub(r"\(.*$", "", i.get("filterText") or i["label"]).strip()


def completion_list(x, prefix):
    if not x or error(x):
        return []
    items = x["items"] if isinstance(x, dict) else x
    p = prefix.lower()
    filtered = [i for i in items if label(i).lower().startswith(p)]
    filtered.sort(key=lambda i: (i.get("sortText") or i["label"]))
    return [label(i) for i in filtered]


legends = {}
lines = [json.loads(l) for l in open(path, encoding="utf-8")]
for l in lines:
    if l["method"] == "initialize":
        cap = (l["result"] or {}).get("capabilities", {})
        legends[l["server"]] = (cap.get("semanticTokensProvider") or {}).get("legend")


def tokens(x, srv):
    if not x or error(x) or not legends.get(srv):
        return []
    d = x["data"]
    leg = legends[srv]
    l = c = 0
    output = []
    for i in range(0, len(d), 5):
        dl, dc, n, t, m = d[i:i + 5]
        l += dl
        c = c + dc if dl == 0 else dc
        mods = frozenset(nm for b, nm in enumerate(leg["tokenModifiers"]) if m & (1 << b))
        output.append((l, c, n, leg["tokenTypes"][t], mods))
    return output


NORMALIZERS = {
    "textDocument/hover": hover_text,
    "textDocument/definition": locations,
    "textDocument/typeDefinition": locations,
    "textDocument/implementation": locations,
    "textDocument/references": locations,
    "textDocument/documentHighlight": highlights,
    "textDocument/prepareRename": prepare,
    "textDocument/rename": edits,
    "textDocument/selectionRange": selection,
    "textDocument/prepareCallHierarchy": hierarchy,
    "textDocument/prepareTypeHierarchy": hierarchy,
    "textDocument/signatureHelp": signature,
    "textDocument/documentSymbol": symbols,
    "textDocument/foldingRange": folds,
    "textDocument/inlayHint": hints,
    "textDocument/documentLink": links,
    "workspace/symbol": ws,
}

stats = collections.OrderedDict()
examples = collections.defaultdict(list)


def count(key, equal, dart_covers, df_covers, ex):
    e = stats.setdefault(key, [0, 0, 0, 0, 0.0, 0.0])
    e[0] += 1
    e[1] += equal
    e[2] += dart_covers
    e[3] += df_covers and dart_covers
    if not equal and len(examples[key]) < N_EX:
        examples[key].append(ex)


ms = collections.defaultdict(lambda: [[], []])
for l in lines:
    m = l["method"]
    if m == "initialize" or (ONLY and ONLY not in m):
        continue
    a, b = l.get("dart"), l.get("dartforge")
    ms[m][0].append(l.get("ms_dart", 0))
    ms[m][1].append(l.get("ms_dartforge", 0))
    where = f'{l.get("project")}/{l.get("file")} {l.get("pos")} {l.get("name", "")}'
    if m == "textDocument/completion":
        pre = l.get("prefix", "")
        la, lb = completion_list(a, pre), completion_list(b, pre)
        target = l["name"]
        has_a, has_b = target in la, target in lb
        count("completion: target present", has_a == has_b or not has_a, has_a, has_b,
              (where, "dart has it" if has_a else "", "df has it" if has_b else ""))
        ra = la.index(target) if has_a else None
        rb = lb.index(target) if has_b else None
        count("completion: same top-1", (la[:1] == lb[:1]), bool(la), bool(lb), (where, la[:3], lb[:3]))
        sa, sb = set(la[:5]), set(lb[:5])
        # An empty list against an empty list counts as equal (ESTADO.md, round r6).
        count("completion: top-5 ≥ 60% shared", (not sa and not sb) or len(sa & sb) >= 0.6 * max(1, len(sa)), bool(la), bool(lb),
              (where, la[:5], lb[:5]))
        continue
    if m == "textDocument/codeAction":
        def by_kind(x, prefix):
            if not x or error(x):
                return frozenset()
            return frozenset(i.get("title") for i in x
                             if (i.get("kind") or "").startswith(prefix) and not i.get("title", "").startswith("Ignore '"))
        if l.get("diagnostic") is not None:
            ta, tb = by_kind(a, "quickfix"), by_kind(b, "quickfix")
            count("codeAction: fixes (quickfix) for Dart's diagnostics", ta <= tb, bool(ta), bool(ta & tb),
                  (where, l["diagnostic"], sorted(ta - tb), sorted(tb - ta)))
        else:
            ta, tb = by_kind(a, "refactor"), by_kind(b, "refactor")
            count("codeAction: assists (refactor) at the cursor", ta == tb, bool(ta), bool(ta & tb),
                  (where, sorted(ta - tb), sorted(tb - ta)))
        ta, tb = by_kind(a, "source"), by_kind(b, "source")
        count("codeAction: source actions (source)", ta == tb, bool(ta), bool(ta & tb), (where, sorted(ta - tb), sorted(tb - ta)))
        # The edit of each action with a shared title (fixes and assists): the
        # normalized `TextEdit`/`SnippetTextEdit`s, with the format.
        ea, eb = edit_by_title(a), edit_by_title(b)
        for title in sorted(set(ea) & set(eb)):
            key = "codeAction: edit (quickfix, shared title)" if l.get("diagnostic") is not None \
                else "codeAction: edit (refactor, shared title)"
            count(key, ea[title] == eb[title], True, True, (where, title, str(ea[title])[:300], str(eb[title])[:300]))
        continue
    if m == "textDocument/semanticTokens/full":
        ta, tb = tokens(a, "dart"), tokens(b, "dartforge")
        da = {(x[0], x[1], x[2]): (x[3], x[4]) for x in ta}
        db = {(x[0], x[1], x[2]): (x[3], x[4]) for x in tb}
        for k, v in da.items():
            count("semanticTokens (per token: type)", db.get(k, (None,))[0] == v[0], True, k in db,
                  (where, k, v, db.get(k)))
        continue
    if m == "textDocument/formatting":
        count("formatting (no edit = equal)", (not a) == (not b), bool(a), bool(b), (where, str(a)[:80], str(b)[:80]))
        # The exact `TextEdit`s (range and text, in order): the minimal
        # whitespace edits of `generateEditsForFormatting`.
        def fmt_list(x):
            if not x or error(x):
                return []
            return [(rg(e.get("range")), e.get("newText")) for e in x]
        fa, fb = fmt_list(a), fmt_list(b)
        count("formatting (identical edits)", fa == fb, bool(fa), bool(fb), (where, str(fa)[:200], str(fb)[:200]))
        continue
    f = NORMALIZERS.get(m)
    if not f:
        continue
    na, nb = f(a), f(b)
    name = m.split("/")[-1]
    if m == "textDocument/hover":
        count("hover (whole text)", na == nb, na is not None, nb is not None, (where, na, nb))
        sa, sb = hover_signature(a), hover_signature(b)
        count("hover (signature)", sa == sb, sa is not None, sb is not None, (where, sa, sb))
        continue
    count(name, na == nb, bool(na), bool(nb), (where, str(na)[:300], str(nb)[:300]))

print("| Feature | Samples | Equal to Dart | Dart answers | DartForge answers where Dart answers |")
print("|---|---|---|---|---|")
for k, (n, eq, ca, cb, _, _) in stats.items():
    print(f"| {k} | {n} | {100 * eq / n:.0f}% | {ca} | {cb}/{ca} |")
print()
print("| Method | Dart median ms | DartForge median ms |")
print("|---|---|---|")
for m, (xa, xb) in ms.items():
    xa, xb = sorted(xa), sorted(xb)
    print(f"| {m} | {xa[len(xa) // 2]:.1f} | {xb[len(xb) // 2]:.1f} |")
print()
for k, ex in examples.items():
    print("##", k)
    for e in ex:
        print("   ", e)
