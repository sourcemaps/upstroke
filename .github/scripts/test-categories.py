#!/usr/bin/env python3
"""Generate the category matrices, or check libtest's native inventory.

The Rust table is the only registry. Planning inventories and timings are not
inputs to this program. No check ever executes an unmatched test.
The standalone check also verifies the crate-root metadata consumed by tests;
refresh-targets updates those snapshots after a manifest or target change.
"""
import argparse
import ast
from collections import Counter, namedtuple
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
MODEL = ROOT / "src/effects/tests/ci_model.rs"
WORKFLOW = ROOT / ".github/workflows/ci.yml"
TARGET_SOURCE = ROOT / "src/effects/tests.rs"
PLATFORMS = ("ubuntu", "macos", "windows")
Registration = namedtuple("Registration", "harness name")


def registry():
    source = MODEL.read_text(encoding="utf-8")
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
                               floor=sum(floor for _, _, floor, _ in selected),
                               check_inventory=platform == "macos" and any(
                                   row["category"] == "unit" and row["subcategory"] == "cli"
                                   for row, _, _, _ in selected)))
    return result


def category_row(rows, name):
    row = next((row for row in rows if any(f in name for f in row["filters"])), None)
    if row is None:
        raise ValueError(f"uncategorised: {name} — add it to a TEST_CATEGORIES row in "
                         "src/effects/tests/ci_model.rs, or put it in a module a row names")
    return row


def owner(rows, platform, name):
    row = category_row(rows, name)
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


def validate(rows, platform, registrations, require_cli=False):
    if not registrations:
        raise ValueError("libtest listed no tests; refusing an empty inventory")
    selections = legs(rows, platform)
    errors = []
    counts = Counter()
    for harness, name in registrations:
        try:
            expected = owner(rows, platform, name)
        except ValueError as error:
            errors.append(f"{error} (harness {harness})")
            continue
        actual = [leg_key(leg) for leg in selections if runs(leg["tests"], name)]
        if actual != [expected]:
            errors.append(f"category selection: {name} (harness {harness}) — "
                          f"expected {expected}, selected {actual}; "
                          "update TEST_CATEGORIES (libtest filters are substrings)")
        counts[expected] += 1
    if errors:
        raise ValueError("\n".join(errors))
    if require_cli and not any(
        row["category"] == "unit" and row["subcategory"] == "cli"
        for row in (category_row(rows, item.name) for item in registrations)
    ):
        raise ValueError("libtest listed no unit/cli registrations; refusing an incomplete all-target inventory")
    return counts


def listed(output, harness):
    names = [line[:-6] for line in output.splitlines() if line.endswith(": test")]
    if len(set(names)) != len(names):
        raise ValueError(f"libtest listed duplicate test names in harness {harness}")
    return [Registration(harness, name) for name in names]


def explain(rows, registrations):
    counts = Counter()
    collisions = []
    for registration in registrations:
        matches = [(row["category"], row["subcategory"]) for row in rows
                   if any(f in registration.name for f in row["filters"])]
        counts[matches[0]] += 1
        if len(matches) > 1:
            collisions.append((registration, matches))
    for row in rows:
        pair = row["category"], row["subcategory"]
        print(f"subcategory {pair[0]}/{pair[1]}: {counts[pair]}")
    for (harness, name), matches in collisions:
        print(f"first-match overlap: {name} (harness {harness}): "
              f"{matches[0]} owns it; later matches {matches[1:]}")


def checked(command):
    process = subprocess.run(command, cwd=ROOT, encoding="utf-8", stdout=subprocess.PIPE)
    if process.returncode:
        raise ValueError(f"command failed ({process.returncode}): {command}")
    return process.stdout


def cargo():
    # A build box owns target isolation. CI and workstations use ordinary Cargo.
    wrapper = shutil.which("upstroke-build")
    return [wrapper, "cargo"] if wrapper else ["cargo"]


def native_platform():
    return {"linux": "ubuntu", "darwin": "macos", "win32": "windows"}[sys.platform]


def target_constant(source, name):
    matches = list(re.finditer(rf'(?ms)^const {name}: &str = r#"(.*?)"#;', source))
    if len(matches) != 1:
        raise ValueError(f"expected one target inventory constant {name}")
    return matches[0]


def target_metadata(manifest):
    manifest = manifest.resolve(strict=True)
    document = json.loads(checked(cargo() + [
        "metadata", "--format-version", "1", "--no-deps", "--offline",
        "--manifest-path", str(manifest),
    ]))
    packages = [package for package in document["packages"]
                if Path(package["manifest_path"]) == manifest]
    if len(packages) != 1 or not packages[0]["targets"]:
        raise ValueError(f"Cargo did not report one nonempty target inventory for {manifest}")
    paths = sorted({Path(os.path.relpath(target["src_path"], manifest.parent)).as_posix()
                    for target in packages[0]["targets"]})
    return dict(manifest_sha256=hashlib.sha256(
        manifest.read_text(encoding="utf-8").encode("utf-8")).hexdigest(), packages=[dict(
            manifest_path="Cargo.toml", targets=[dict(src_path=path) for path in paths])])


def sync_target_metadata(refresh=False):
    source = TARGET_SOURCE.read_text(encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="upstroke-targets-") as directory:
        fixture = Path(directory) / "Cargo.toml"
        fixture.write_text(target_constant(source, "CUSTOM_TARGET_MANIFEST").group(1),
                           encoding="utf-8")
        for name, manifest in (("REPOSITORY_TARGET_METADATA", ROOT / "Cargo.toml"),
                               ("CUSTOM_TARGET_METADATA", fixture)):
            match = target_constant(source, name)
            actual = target_metadata(manifest)
            if not refresh and json.loads(match.group(1)) != actual:
                raise ValueError(f"stale {name}; run python3 .github/scripts/test-categories.py "
                                 "refresh-targets before testing")
            if refresh:
                source = source[:match.start(1)] + json.dumps(actual, indent=2) + source[match.end(1):]
    if refresh:
        TARGET_SOURCE.write_text(source, encoding="utf-8")


def built_harnesses():
    output = checked(cargo() + ["test", "--all-targets", "--all-features", "--no-run",
                                "--message-format=json"])
    harnesses = []
    for line in output.splitlines():
        artifact = json.loads(line)
        if (artifact.get("reason") == "compiler-artifact"
                and artifact.get("profile", {}).get("test")
                and artifact.get("executable")):
            harness = artifact["executable"]
            if harness in harnesses:
                raise ValueError(f"Cargo listed duplicate test harness: {harness}")
            harnesses.append(harness)
    if not harnesses:
        raise ValueError("Cargo listed no test harnesses; refusing an empty inventory")
    return harnesses


def inventory(harnesses, selection=()):
    return [registration for harness in harnesses
            for registration in listed(
                checked([harness, "--list", "--format", "terse", *selection]), harness)]


def prove(rows, platform, harness, require_cli=False):
    registrations = inventory([harness])
    validate(rows, platform, registrations, require_cli=require_cli)
    observed = Counter()
    for leg in legs(rows, platform):
        actual = inventory([harness], leg["tests"].split())
        expected = {item for item in registrations
                    if owner(rows, platform, item.name) == leg_key(leg)}
        if set(actual) != expected:
            raise ValueError(f"{leg['name']}: real libtest differs; missing {sorted(expected-set(actual))}, "
                             f"extra {sorted(set(actual)-expected)}")
        observed.update(actual)
    if observed != Counter(registrations):
        raise ValueError("real libtest did not list each registration exactly once")
    skips = [word for row in rows for f in row["filters"] for word in ("--skip", f)]
    unmatched = inventory([harness], skips)
    if unmatched:
        raise ValueError(f"uncategorised: {unmatched}")
    print(f"{platform}: real libtest, {len(registrations)} registrations, "
          "exactly one leg each (ignored included)")


def matrix(rows, platform):
    key = "shard" if platform == "windows" else "leg"
    lines = []
    for leg in legs(rows, platform):
        name = leg["name"] if platform == "windows" else f"{platform}-latest, {leg['name']}"
        lines.extend([f"          - {key}: {json.dumps(name)}",
                      f"            tests: {json.dumps(leg['tests'])}"])
        if platform == "windows":
            lines.append(f"            floor: {leg['floor']}")
        elif platform == "macos":
            lines.append(f"            check_inventory: {json.dumps(leg['check_inventory'])}")
    return "\n".join(lines) + "\n"


def generated(rows, source):
    for platform in PLATFORMS:
        starts = list(re.finditer(rf"(?m)^        # BEGIN TEST_CATEGORIES {platform}(\r?\n)", source))
        ends = list(re.finditer(rf"(?m)^        # END TEST_CATEGORIES {platform}(?:\r?\n|$)", source))
        if len(starts) != 1 or len(ends) != 1:
            raise ValueError(f"missing or duplicated matrix markers for {platform}")
        start, end = starts[0], ends[0]
        if start.end() > end.start():
            raise ValueError(f"reversed matrix markers for {platform}")
        block = ("        include:\n" + matrix(rows, platform)).replace("\n", start.group(1))
        source = source[:start.end()] + block + source[end.start():]
    return source


def sync_workflow(rows, check=False):
    # Decode explicitly without universal-newline translation: only marked
    # interiors may change, and each block inherits its opening marker's EOL.
    before = WORKFLOW.read_bytes()
    after = generated(rows, before.decode("utf-8")).encode("utf-8")
    if before != after:
        if check:
            raise ValueError("stale category matrix; run python3 .github/scripts/test-categories.py generate")
        WORKFLOW.write_bytes(after)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("generate", "check", "prove", "contract", "refresh-targets"))
    parser.add_argument("--check", action="store_true", help="refuse a stale generated matrix")
    parser.add_argument("--platform", choices=PLATFORMS, default=native_platform())
    parser.add_argument("--harness", help="already built native libtest executable")
    parser.add_argument("--require-cli", action="store_true",
                        help="require a unit/cli registration (always required by check)")
    parser.add_argument("--explain", action="store_true", help="show subcategory counts and later matches")
    args = parser.parse_args()
    if args.action in ("check", "refresh-targets") and "RUSTC_BOOTSTRAP" in os.environ:
        raise ValueError(f"{args.action} refuses to run Cargo while RUSTC_BOOTSTRAP is set; "
                         "use prove --harness for a running libtest")
    rows = registry()
    if args.action == "contract":
        print(json.dumps(dict(registry=rows, legs={p: legs(rows, p) for p in PLATFORMS})))
    elif args.action == "generate":
        sync_workflow(rows, check=args.check)
    elif args.action == "refresh-targets":
        sync_target_metadata(refresh=True)
    elif args.action == "prove":
        if not args.harness:
            parser.error("prove requires --harness")
        prove(rows, args.platform, args.harness, require_cli=args.require_cli)
    else:
        sync_workflow(rows, check=True)
        sync_target_metadata()
        harnesses = built_harnesses()
        registrations = inventory(harnesses)
        counts = validate(rows, args.platform, registrations, require_cli=True)
        skips = [word for row in rows for f in row["filters"] for word in ("--skip", f)]
        unmatched = inventory(harnesses, skips)
        if unmatched:
            raise ValueError("\n".join(f"uncategorised: {name} (harness {harness}) — "
                                       f"update TEST_CATEGORIES in {MODEL}"
                                       for harness, name in unmatched))
        print(f"{args.platform}: {len(registrations)} registrations including ignored; uncategorised: 0")
        for (category, number), count in sorted(counts.items()):
            print(f"  {category}/{number}: {count}")
        if args.explain:
            explain(rows, registrations)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, SyntaxError, OSError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)

