"""Require complete provider coverage, including the owned changes to pinned hron."""
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path


def changed_lines(patch):
    """Read new-file line positions of additions in a unified source diff."""
    result = {}
    current = None
    position = 0
    for line in patch.splitlines():
        if line.startswith("+++ b/"):
            current = result.setdefault(Path(line[6:]).resolve(), set())
        elif line.startswith("@@"):
            position = int(re.search(r" \+(\d+)", line).group(1))
        elif line.startswith("+") and current is not None:
            current.add(position)
            position += 1
        elif line.startswith(" "):
            position += 1
    return result


def production_modules():
    root = Path("src").resolve()
    sources = set(root.rglob("*.rs"))
    tests = set()
    declaration = re.compile(r"#\[cfg\(test\)\]\s*(?:#\[[^\n]*\]\s*)*mod (\w+)\s*;")
    for source in sources:
        children = source.parent if source.stem in {"lib", "mod", "main"} else source.with_suffix("")
        for name in declaration.findall(source.read_text()):
            tests.add(children / f"{name}.rs")
            tests.update((children / name).rglob("*.rs"))
    return sources - tests | {Path("third_party/hron/src/eval/captured.rs").resolve()}


def patch_line_counts(segments, changed):
    """LLVM LineCoverageStats combines wrapped and non-gap region-entry counters."""
    grouped = defaultdict(list)
    for segment in segments:
        grouped[segment[0]].append(segment)
    wrapped = None
    result = {}
    for line in range(1, max(grouped, default=0) + 1):
        entries = grouped.get(line, [])
        if line in changed:
            counted = [s for s in entries if s[3] and s[4]]
            skipped = entries and not entries[0][3] and entries[0][4]
            if counted or (wrapped and wrapped[3] and not skipped):
                result[line] = max([wrapped[2] if wrapped else 0] + [s[2] for s in counted if not s[5]])
        if entries:
            wrapped = entries[-1]
    return result


def verify(report_dir):
    expected = production_modules()
    changed = changed_lines(Path("third_party/hron-2.2.1.patch").read_text())
    files = {}
    failures = []
    for entry in json.loads((report_dir / "coverage.json").read_text())["data"]:
        for item in entry["files"]:
            path = Path(item["filename"]).resolve()
            if path in expected or path in changed:
                if path in files:
                    failures.append(f"{path}: duplicate JSON file record")
                files[path] = item
    for path in sorted((expected | set(changed)) - files.keys()):
        failures.append(f"{path}: missing production file")
    for path in expected & files.keys():
        for metric in ("lines", "branches"):
            counts = files[path]["summary"].get(metric, {})
            if "count" not in counts or counts.get("covered") != counts["count"] or (metric == "lines" and not counts["count"]):
                failures.append(f"{path}: {metric} {counts}")

    # Compute expected added-line and branch-arm observations independently
    # from LLVM's JSON mapping, then reconcile both sets with the LCOV export.
    wanted_lines = {}
    wanted_arms = Counter()
    for path in changed.keys() & files.keys():
        info = files[path]
        for n, hits in patch_line_counts(info["segments"], changed[path]).items():
            wanted_lines[path, n] = hits
        for branch in info["branches"]:
            if branch[0] in changed[path]:
                for hits in branch[4:6]:
                    wanted_arms[path, branch[0], hits] += 1
    if not wanted_lines or not wanted_arms:
        failures.append("Missing mapped lines or branches for the patched engine")

    observed_lines = {}
    observed_arms = Counter()
    branch_ids = set()
    path = None
    for row in (report_dir / "coverage.lcov").read_text().splitlines():
        if row.startswith("SF:"):
            path = Path(row[3:]).resolve()
        elif row == "end_of_record":
            path = None
        elif path in changed and row.startswith(("DA:", "BRDA:")):
            fields = row.partition(":")[2].split(",")
            number = int(fields[0])
            if number not in changed[path]:
                continue
            if row.startswith("DA:"):
                if (path, number) in observed_lines:
                    failures.append(f"{path}:{number}: duplicate LCOV line")
                observed_lines[path, number] = int(fields[1])
            else:
                site = (path, number, fields[1], fields[2])
                if site in branch_ids:
                    failures.append(f"{path}:{number}: duplicate LCOV branch arm")
                branch_ids.add(site)
                hits = 0 if fields[3] == "-" else int(fields[3])
                observed_arms[path, number, hits] += 1
    for key in sorted(wanted_lines.keys() | observed_lines.keys()):
        if wanted_lines.get(key) != observed_lines.get(key) or not wanted_lines.get(key):
            failures.append(f"{key[0]}:{key[1]}: missing, inconsistent or uncovered patched line")
    for key in sorted(wanted_arms.keys() | observed_arms.keys()):
        if wanted_arms[key] != observed_arms[key] or not key[2]:
            failures.append(f"{key[0]}:{key[1]}: missing, inconsistent or uncovered patched branch")
    for failure in failures:
        print(failure, file=sys.stderr)
    print(f"Checked {len(expected)} production modules, {len(wanted_lines)} added lines "
          f"and {wanted_arms.total()} added branch arms")
    return bool(failures)


if __name__ == "__main__":
    sys.exit(verify(Path(sys.argv[1])))
