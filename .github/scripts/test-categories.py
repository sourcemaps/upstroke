#!/usr/bin/env python3
"""Generate the category matrices, or check libtest's native inventory.

The Rust table is the only registry. Planning inventories and timings are not
inputs to this program. No check ever executes an unmatched test.
"""
import argparse
import ast
from collections import Counter
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
MODEL = ROOT / "src/effects/tests/ci_model.rs"
WORKFLOW = ROOT / ".github/workflows/ci.yml"
PLATFORMS = ("ubuntu", "macos", "windows")


def registry():
    source = MODEL.read_text()
    body = source.split("const TEST_CATEGORIES: &[TestCategory] = ", 1)[1].split("\n];", 1)[0] + "\n]"
    # This deliberately accepts only the table's literal Rust records and arrays.
    body = body.replace("TestCategory {", "{").replace("&[", "[")
    body = re.sub(r"(?m)^(\s*)(category|subcategory|filters|macos|windows):", r'\1"\2":', body)
    rows = ast.literal_eval(body)
    seen = set()
    for row in rows:
        if set(row) != {"category", "subcategory", "filters", "macos", "windows"}:
            raise ValueError("unexpected TEST_CATEGORIES fields")
        pair = row["category"], row["subcategory"]
        if pair in seen or not row["filters"] or row["macos"] < 1 or not row["windows"]:
            raise ValueError(f"invalid category row: {pair}")
        seen.add(pair)
        for word in [*row["filters"], *(f for _, _, fs in row["windows"] for f in fs)]:
            if not re.fullmatch(r"[A-Za-z0-9_:]+", word):
                raise ValueError(f"unsafe or empty libtest filter: {word!r}")
        if any(leg < 1 or floor < 0 for leg, floor, _ in row["windows"]):
            raise ValueError(f"invalid Windows partition: {pair}")
    return rows


def flattened(rows, platform):
    return [
        (row, leg, floor, filters or row["filters"])
        for row in rows
        for leg, floor, filters in (
            row["windows"] if platform == "windows"
            else [(row["macos"] if platform == "macos" else 1, 0, row["filters"])]
        )
    ]


def legs(rows, platform):
    flat = flattened(rows, platform)
    result = []
    for category in sorted({row["category"] for row in rows}):
        count = max(leg for row, leg, _, _ in flat if row["category"] == category)
        for number in range(1, count + 1):
            def owns(part):
                return part[0]["category"] == category and part[1] == number
            selected = [part for part in flat if owns(part)]
            filters = [f for _, _, _, fs in selected for f in fs]
            # Earlier categories win. A category that recurs (unit/model precedes
            # self-checks/effects) starts at its FIRST row, preserving that priority.
            skips = []
            for row in rows:
                if row["category"] == category:
                    break
                skips.extend(row["filters"])
            # A broad residual topic must exclude earlier pieces owned by another
            # leg of this category, but never exclude its own earlier pieces.
            for index, part in enumerate(flat):
                if part[0]["category"] != category or owns(part):
                    continue
                skips.extend(f for f in part[3] if any(
                    owns(later) and any(g in f for g in later[3])
                    for later in flat[index + 1:]
                ))
            filters = list(dict.fromkeys(
                f for f in filters if not any(g != f and g in f for g in filters)
            ))
            if not filters:
                raise ValueError(f"{category}/{number} has no positive selector")
            words = filters + [word for f in dict.fromkeys(skips) for word in ("--skip", f)]
            subs = ", ".join(sorted({row["subcategory"] for row, _, _, _ in selected}))
            suffix = f" {number} of {count}" if count > 1 else ""
            result.append(dict(name=f"{category}{suffix} ({subs})", tests=" ".join(words),
                               floor=sum(floor for _, _, floor, _ in selected)))
    return result


def owner(rows, platform, name):
    row = next((row for row in rows if any(f in name for f in row["filters"])), None)
    if row is None:
        raise ValueError(f"uncategorised: {name} — add it to a TEST_CATEGORIES row in "
                         "src/effects/tests/ci_model.rs, or put it in a module a row names")
    if platform != "windows":
        return row["category"], row["macos"] if platform == "macos" else 1
    matches = [leg for leg, _, filters in row["windows"]
               if not filters or any(f in name for f in filters)]
    if len(matches) != 1:
        raise ValueError(f"uncategorised partition: {name} — {row['category']}/"
                         f"{row['subcategory']} selects Windows legs {matches}; update TEST_CATEGORIES")
    return row["category"], matches[0]


def leg_key(leg):
    words = leg["name"].split()
    return words[0], int(words[1]) if words[1].isdigit() else 1


def runs(selection, name):
    words = iter(selection.split())
    filters, skips = [], []
    for word in words:
        if word == "--skip":
            skips.append(next(words))
        else:
            filters.append(word)
    return (not filters or any(f in name for f in filters)) and not any(f in name for f in skips)


def validate(rows, platform, names):
    if not names:
        raise ValueError("libtest listed no tests; refusing an empty inventory")
    selections = legs(rows, platform)
    errors = []
    counts = Counter()
    for name in names:
        try:
            expected = owner(rows, platform, name)
        except ValueError as error:
            errors.append(str(error))
            continue
        actual = [leg_key(leg) for leg in selections if runs(leg["tests"], name)]
        if actual != [expected]:
            errors.append(f"category selection: {name} — expected {expected}, selected {actual}; "
                          "update TEST_CATEGORIES (libtest filters are substrings)")
        counts[expected] += 1
    if errors:
        raise ValueError("\n".join(errors))
    return counts


def listed(output):
    names = [line[:-6] for line in output.splitlines() if line.endswith(": test")]
    if len(set(names)) != len(names):
        raise ValueError("libtest listed duplicate test names")
    return names


def explain(rows, names):
    counts = Counter()
    collisions = []
    for name in names:
        matches = [(row["category"], row["subcategory"]) for row in rows
                   if any(f in name for f in row["filters"])]
        counts[matches[0]] += 1
        if len(matches) > 1:
            collisions.append((name, matches))
    for row in rows:
        pair = row["category"], row["subcategory"]
        print(f"subcategory {pair[0]}/{pair[1]}: {counts[pair]}")
    for name, matches in collisions:
        print(f"first-match overlap: {name}: {matches[0]} owns it; later matches {matches[1:]}")


def checked(command):
    process = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE)
    if process.returncode:
        raise ValueError(f"command failed ({process.returncode}): {command}")
    return process.stdout


def cargo():
    # A build box owns target isolation. CI and workstations use ordinary Cargo.
    # A check called by a test already runs inside Cargo's caller-owned build
    # slot. Reuse that process environment; acquiring the slot again deadlocks.
    if os.environ.get("CARGO_TARGET_DIR") and os.environ.get("CARGO"):
        return [os.environ["CARGO"]]
    wrapper = shutil.which("upstroke-build")
    return [wrapper, "cargo"] if wrapper else ["cargo"]


def native_platform():
    return {"linux": "ubuntu", "darwin": "macos", "win32": "windows"}[sys.platform]


def prove(rows, platform, harness):
    names = listed(checked([harness, "--list", "--format", "terse"]))
    validate(rows, platform, names)
    observed = Counter()
    for leg in legs(rows, platform):
        actual = listed(checked([harness, "--list", "--format", "terse", *leg["tests"].split()]))
        expected = {name for name in names if owner(rows, platform, name) == leg_key(leg)}
        if set(actual) != expected:
            raise ValueError(f"{leg['name']}: real libtest differs; missing {sorted(expected-set(actual))}, "
                             f"extra {sorted(set(actual)-expected)}")
        observed.update(actual)
    if observed != Counter(names):
        raise ValueError("real libtest did not list each registration exactly once")
    skips = [word for row in rows for f in row["filters"] for word in ("--skip", f)]
    unmatched = listed(checked([harness, "--list", "--format", "terse", *skips]))
    if unmatched:
        raise ValueError(f"uncategorised: {unmatched}")
    print(f"{platform}: real libtest, {len(names)} registrations, exactly one leg each (ignored included)")


def matrix(rows, platform):
    key = "shard" if platform == "windows" else "leg"
    lines = []
    for leg in legs(rows, platform):
        name = leg["name"] if platform == "windows" else f"{platform}-latest, {leg['name']}"
        lines.extend([f"          - {key}: {json.dumps(name)}",
                      f"            tests: {json.dumps(leg['tests'])}"])
        if platform == "windows":
            lines.append(f"            floor: {leg['floor']}")
    return "\n".join(lines) + "\n"


def generated(rows, source):
    for platform in PLATFORMS:
        start = f"        # BEGIN TEST_CATEGORIES {platform}\n"
        end = f"        # END TEST_CATEGORIES {platform}\n"
        if source.count(start) != 1 or source.count(end) != 1:
            raise ValueError(f"missing or duplicated matrix markers for {platform}")
        before, rest = source.split(start)
        _, after = rest.split(end)
        source = before + start + "        include:\n" + matrix(rows, platform) + end + after
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("generate", "check", "prove", "contract"))
    parser.add_argument("--check", action="store_true", help="refuse a stale generated matrix")
    parser.add_argument("--platform", choices=PLATFORMS, default=native_platform())
    parser.add_argument("--harness", help="already built native libtest executable")
    parser.add_argument("--explain", action="store_true", help="show subcategory counts and later matches")
    args = parser.parse_args()
    rows = registry()
    if args.action == "contract":
        print(json.dumps(dict(registry=rows, legs={p: legs(rows, p) for p in PLATFORMS})))
    elif args.action == "generate":
        before = WORKFLOW.read_text()
        after = generated(rows, before)
        if args.check:
            if before != after:
                raise ValueError("stale category matrix; run python3 .github/scripts/test-categories.py generate")
        else:
            WORKFLOW.write_text(after)
    elif args.action == "prove":
        if not args.harness:
            parser.error("prove requires --harness")
        prove(rows, args.platform, args.harness)
    else:
        names = listed(checked(cargo() + ["test", "--all-targets", "--all-features",
                                         "--", "--list", "--format", "terse"]))
        counts = validate(rows, args.platform, names)
        skips = [word for row in rows for f in row["filters"] for word in ("--skip", f)]
        unmatched = listed(checked(cargo() + ["test", "--all-targets", "--all-features",
                                              "--", "--list", "--format", "terse", *skips]))
        if unmatched:
            raise ValueError("\n".join(f"uncategorised: {name} — update TEST_CATEGORIES in {MODEL}"
                                       for name in unmatched))
        print(f"{args.platform}: {len(names)} registrations including ignored; uncategorised: 0")
        for (category, number), count in sorted(counts.items()):
            print(f"  {category}/{number}: {count}")
        if args.explain:
            explain(rows, names)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, SyntaxError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)

