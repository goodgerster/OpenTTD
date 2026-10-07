#!/usr/bin/env python3
"""Measure how often each C++ source file changes, to plan the order of porting to Rust.

Porting a file turns every later change that a jgrpp merge brings to it into a
manual reimplementation, so files that rarely change are cheaper to port first.

Needs a clone of JGR's repository (https://github.com/JGRennison/OpenTTD-patches)
with the `jgrpp` branch (and `master`, to tell which files exist upstream):

    git clone --bare --shallow-since=2022-10-01 --single-branch --branch jgrpp \\
        https://github.com/JGRennison/OpenTTD-patches jgr.git
    git -C jgr.git fetch --shallow-since=2022-10-01 origin master:master

Each step on jgrpp's first-parent history is diffed against the previous step,
which is what a merge of jgrpp into this fork brings in. Steps are split into:
- upstream merges ("Merge branch 'master' into jgrpp", "Merge tag ..."), which
  include JGR's conflict resolutions and adaptations of upstream changes;
- jgrpp changes: every other step (JGR's commits and merges of feature branches).

Renames are followed; files split into several new files are not. Paths are matched against the files
currently in this repository's src/ (excluding src/3rdparty).
"""

import argparse
import collections
import csv
import datetime
import pathlib
import subprocess
import sys

UPSTREAM_MERGE_PREFIXES = ("Merge branch 'master' into jgrpp", "Merge tag ")


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True, text=True).stdout


def split_rename(path):
    """Split git's numstat rename notation ("a/{old => new}/f" or "old => new") into (old, new)."""
    if " => " not in path:
        return None, path
    if "{" in path:
        prefix, rest = path.split("{", 1)
        middle, suffix = rest.split("}", 1)
        old, new = middle.split(" => ")
        return ((prefix + old + suffix).replace("//", "/"), (prefix + new + suffix).replace("//", "/"))
    old, new = path.split(" => ")
    return old, new


def first_parent_changes(repo, since):
    """Yield (origin, date, path, lines changed) for each file in each first-parent step of jgrpp.

    Paths are reported under their current name: history is read newest first, and
    once a rename is seen, older changes to the old path count for the new one.
    """
    out = git(repo, "log", "--first-parent", "--diff-merges=first-parent", "-M", f"--since={since}",
              "--format=@%cI%x09%P%x09%s", "--numstat", "jgrpp")
    current_name = {}  # path at an older time -> path now

    def resolve(path):
        while path in current_name:
            path = current_name[path]
        return path

    origin = date = None
    renames = []
    for line in out.splitlines() + ["@"]:
        if line.startswith("@"):
            # Renames apply to commits older than the one that made them.
            for old, new in renames:
                current_name[old] = resolve(new)
            renames = []
            if line == "@":
                break
            date_str, parents, subject = line[1:].split("\t", 2)
            date = datetime.datetime.fromisoformat(date_str).date()
            is_upstream_merge = len(parents.split()) > 1 and subject.startswith(UPSTREAM_MERGE_PREFIXES)
            origin = "upstream" if is_upstream_merge else "jgrpp"
        elif line:
            added, deleted, path = line.split("\t", 2)
            old, path = split_rename(path)
            if old is not None and old != path:
                renames.append((old, path))
            lines = 0 if added == "-" else int(added) + int(deleted)
            yield origin, date, resolve(path), lines


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", required=True, help="clone of JGR's repository")
    parser.add_argument("--tree", default=".", help="this repository's working tree")
    parser.add_argument("--since", default="2022-10-01", help="start of the measured period")
    parser.add_argument("--recent-days", type=int, default=365, help="length of the 'recent' window")
    parser.add_argument("--csv", help="write per-file results to this CSV file")
    parser.add_argument("--markdown", help="write a summary report to this Markdown file")
    args = parser.parse_args()

    tree = pathlib.Path(args.tree)
    files = [
        p for p in git(args.tree, "ls-files", "src").splitlines()
        if p.endswith((".cpp", ".h", ".hpp")) and not p.startswith("src/3rdparty/")
    ]
    loc = {p: sum(1 for _ in open(tree / p, errors="replace")) for p in files}
    in_upstream = set(git(args.repo, "ls-tree", "-r", "--name-only", "master").splitlines())

    jgrpp_head = git(args.repo, "rev-parse", "jgrpp").strip()
    period_end = datetime.date.fromisoformat(git(args.repo, "log", "-1", "--format=%cs", "jgrpp").strip())
    recent_start = period_end - datetime.timedelta(days=args.recent_days)
    years = (period_end - datetime.date.fromisoformat(args.since)).days / 365.25

    # stats[origin][path] = [steps, lines, recent steps]
    stats = {o: collections.defaultdict(lambda: [0, 0, 0]) for o in ("upstream", "jgrpp")}
    build_system = collections.Counter()
    steps = collections.Counter()
    seen_steps = set()
    for origin, date, path, lines in first_parent_changes(args.repo, args.since):
        s = stats[origin][path]
        s[0] += 1
        s[1] += lines
        if date >= recent_start:
            s[2] += 1
        if path.endswith("CMakeLists.txt") or path.startswith("cmake/"):
            build_system[origin] += 1
    for line in git(args.repo, "log", "--first-parent", f"--since={args.since}", "--format=%P%x09%s", "jgrpp").splitlines():
        parents, subject = line.split("\t", 1)
        upstream = len(parents.split()) > 1 and subject.startswith(UPSTREAM_MERGE_PREFIXES)
        steps["upstream" if upstream else "jgrpp"] += 1

    rows = []
    for p in files:
        up, jg = stats["upstream"].get(p, [0, 0, 0]), stats["jgrpp"].get(p, [0, 0, 0])
        rows.append({
            "path": p,
            "loc": loc[p],
            "in_upstream": p in in_upstream,
            "upstream_merges": up[0],
            "upstream_lines": up[1],
            "upstream_recent": up[2],
            "jgrpp_changes": jg[0],
            "jgrpp_lines": jg[1],
            "jgrpp_recent": jg[2],
            "changes_per_year": round((up[0] + jg[0]) / years, 1),
        })

    if args.csv:
        with open(args.csv, "w", newline="") as f:
            writer = csv.DictWriter(f, fieldnames=rows[0].keys())
            writer.writeheader()
            writer.writerows(rows)

    if args.markdown:
        write_report(args, rows, build_system, steps, jgrpp_head, period_end, recent_start, years)
    return 0


def directory(path):
    parts = path.split("/")
    return "/".join(parts[:2]) + "/" if len(parts) > 2 else "src/ (top level)"


def write_report(args, rows, build_system, steps, jgrpp_head, period_end, recent_start, years):
    def table(header, items):
        lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
        lines += ["| " + " | ".join(str(c) for c in item) + " |" for item in items]
        return "\n".join(lines)

    def file_row(r):
        return (f"`{r['path']}`", r["loc"], r["upstream_merges"], r["jgrpp_changes"],
                r["upstream_recent"] + r["jgrpp_recent"], r["changes_per_year"])

    file_header = ("File", "Lines", "Upstream merges", "jgrpp changes", "Changes, last year", "Changes/year")

    dirs = collections.defaultdict(lambda: [0, 0, 0, 0])
    for r in rows:
        d = dirs[directory(r["path"])]
        d[0] += 1
        d[1] += r["loc"]
        d[2] += r["upstream_merges"]
        d[3] += r["jgrpp_changes"]
    dir_rows = sorted(dirs.items(), key=lambda kv: -(kv[1][2] + kv[1][3]) / max(kv[1][1], 1))

    hot = sorted(rows, key=lambda r: -r["changes_per_year"])[:30]
    cold = sorted((r for r in rows if r["loc"] >= 150), key=lambda r: (r["changes_per_year"], -r["loc"]))[:40]
    jgrpp_only = sorted((r for r in rows if not r["in_upstream"] and r["loc"] >= 150),
                        key=lambda r: (r["changes_per_year"], -r["loc"]))

    out = [
        "# Change frequency of C++ files",
        "",
        f"Generated by `utils/port_churn.py` from jgrpp `{jgrpp_head[:10]}`, for {args.since} to {period_end} "
        f"({years:.1f} years); \"last year\" means since {recent_start}.",
        "",
        "Each change is one step on jgrpp's first-parent history that touched the file: an upstream merge "
        "(which includes JGR's adaptations of upstream changes) or a jgrpp change. A merge of jgrpp into this "
        "fork brings exactly these steps, so once a file is ported, each of them is a manual reimplementation. "
        "Renames are followed; changes made before a file was split out of another are not counted.",
        "",
        "## Summary",
        "",
        f"- Steps on jgrpp in the period: {steps['upstream']} upstream merges, {steps['jgrpp']} jgrpp changes.",
        f"- Files: {len(rows)} ({sum(r['loc'] for r in rows):,} lines); "
        f"{sum(1 for r in rows if not r['in_upstream'])} exist only in jgrpp.",
        f"- Files never changed in the period: {sum(1 for r in rows if r['changes_per_year'] == 0)}.",
        f"- Build system (`CMakeLists.txt`, `cmake/`) file changes: {build_system['upstream']} in upstream merges, "
        f"{build_system['jgrpp']} in jgrpp changes.",
        "",
        "## By directory (sorted by changes per 1,000 lines)",
        "",
        table(("Directory", "Files", "Lines", "Upstream merges", "jgrpp changes", "Changes per 1,000 lines"),
              [(f"`{d}`", v[0], v[1], v[2], v[3], round(1000 * (v[2] + v[3]) / max(v[1], 1), 1))
               for d, v in dir_rows]),
        "",
        "## Most frequently changed files (port late)",
        "",
        table(file_header, [file_row(r) for r in hot]),
        "",
        "## Least frequently changed files of 150+ lines (port candidates)",
        "",
        table(file_header, [file_row(r) for r in cold]),
        "",
        "## Files of 150+ lines that exist only in jgrpp, least frequently changed first",
        "",
        table(file_header, [file_row(r) for r in jgrpp_only[:30]]),
        "",
    ]
    pathlib.Path(args.markdown).write_text("\n".join(out))


if __name__ == "__main__":
    sys.exit(main())
