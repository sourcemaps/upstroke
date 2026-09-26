//! Extended notes: `docs/internals/effects/tests.md`

// Allowlist placement: the funnel section of `effects/allowlist.toml`, which

#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
#![forbid(clippy::disallowed_macros)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::census_domain::{CrateRoots, InventoryRefusal};
use super::{
    ALLOWLIST_TOML, CLIPPY_TOML, DENIAL_CONTROL, DENIAL_FIXTURES, EFFECT_SITES_JSON,
    FROZEN_LEGACY_ALLOWLIST, FUNNEL_MODULES_JSON, REGENERATE, RESIDUE_CLASSES_JSON,
    TOPOLOGY_MODULES, USED_GOVERNED_LINTS, WRAPPERS_TOML, blank_comments,
    blank_comments_and_strings, governed_allows, legacy_growth, normalize_lint, production_region,
    topology_modules_among,
};
use crate::topology::effects::{EffectSiteId, effect_sites, effect_sites_json};

mod policy;

use policy::{PACKET_PRIMITIVES, PACKET_TYPES, host_conditional_paths, marker_before};

mod classification;

use classification::checks;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub(in crate::effects) fn crate_roots() -> &'static CrateRoots {
    static ROOTS: std::sync::OnceLock<CrateRoots> = std::sync::OnceLock::new();
    ROOTS.get_or_init(|| crate_roots_of(&repo_root()).unwrap_or_else(|refusal| panic!("{refusal}")))
}

pub(in crate::effects) fn crate_roots_of(
    manifest_dir: &Path,
) -> Result<CrateRoots, InventoryRefusal> {
    let manifest = manifest_dir.join("Cargo.toml");
    CrateRoots::from_metadata_json(&cargo_metadata_json(&manifest)?, &manifest)
}

fn cargo_metadata_json(manifest: &Path) -> Result<String, InventoryRefusal> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .map_err(|error| InventoryRefusal::NotRun {
            manifest: manifest.to_path_buf(),
            why: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(InventoryRefusal::Failed {
            manifest: manifest.to_path_buf(),
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|error| InventoryRefusal::Unreadable {
        manifest: manifest.to_path_buf(),
        why: error.to_string(),
    })
}

fn scanned_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.map(|e| e.expect("entry").path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                walk(&path, into);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                into.push(path);
            }
        }
    }
    let root = repo_root();
    let mut files = Vec::new();
    walk(&root.join("src"), &mut files);
    walk(&root.join("examples"), &mut files);
    assert!(files.len() > 30, "the walk found the tree: {}", files.len());
    files
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(&root)
                .expect("under the manifest")
                .to_string_lossy()
                .replace('\\', "/");
            (relative, fs::read_to_string(&path).expect("read source"))
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Allowlist {
    #[serde(default)]
    funnel: Vec<AllowlistEntry>,
    #[serde(default)]
    legacy: Vec<AllowlistEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowlistEntry {
    path: String,
    #[serde(default)]
    allows: Vec<String>,
    #[serde(default)]
    expect_sites: usize,
    #[serde(default)]
    absent: bool,
    packet: String,
    #[serde(default)]
    review: String,
    #[serde(default)]
    legacy_effect: String,
    #[serde(default)]
    shrinks_when: String,
}

fn allowlist() -> Allowlist {
    let text =
        fs::read_to_string(repo_root().join(ALLOWLIST_TOML)).expect("effects/allowlist.toml");
    toml::from_str(&text).expect("the allowlist parses")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClippyToml {
    #[serde(default, rename = "disallowed-methods")]
    disallowed_methods: Vec<DeniedPath>,
    #[serde(default, rename = "disallowed-types")]
    disallowed_types: Vec<DeniedPath>,
    #[serde(default, rename = "disallowed-macros")]
    disallowed_macros: Vec<DeniedPath>,
    #[serde(default, rename = "allow-expect-in-tests")]
    allow_expect_in_tests: bool,
    #[serde(default, rename = "allow-panic-in-tests")]
    allow_panic_in_tests: bool,
    #[serde(default, rename = "allow-print-in-tests")]
    allow_print_in_tests: bool,
}

#[test]
fn clippy_toml_turns_the_allowances_on_and_gives_unwrap_none() {
    let clippy = denylist();
    assert!(
        clippy.allow_expect_in_tests,
        "§7 allows .expect( with a message in tests"
    );
    assert!(
        clippy.allow_panic_in_tests,
        "§7 allows panic! in a test's own assertion helpers"
    );
    assert!(clippy.allow_print_in_tests, "§7 allows printing from tests");
    let text = fs::read_to_string(repo_root().join(CLIPPY_TOML)).expect("clippy.toml");
    assert!(
        !text.contains("allow-unwrap-in-tests"),
        "§7: .unwrap() has no allowance -- it is denied everywhere, tests included"
    );
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeniedPath {
    path: String,
    reason: String,
    #[serde(default, rename = "allow-invalid")]
    allow_invalid: bool,
}

impl ClippyToml {
    fn all(&self) -> impl Iterator<Item = &DeniedPath> {
        self.disallowed_methods
            .iter()
            .chain(&self.disallowed_types)
            .chain(&self.disallowed_macros)
    }

    fn paths(&self) -> BTreeSet<&str> {
        self.all().map(|entry| entry.path.as_str()).collect()
    }
}

fn denylist() -> ClippyToml {
    let text = fs::read_to_string(repo_root().join(CLIPPY_TOML)).expect("clippy.toml");
    denylist_from(&text)
}

fn denylist_from(text: &str) -> ClippyToml {
    toml::from_str(text).expect("clippy.toml parses")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wrappers {
    module: Vec<ModuleClassification>,
    libc: LibcClassification,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleClassification {
    path: String,
    crate_path: String,
    #[serde(default)]
    funnel: Vec<String>,
    #[serde(default)]
    effectful: Vec<String>,
    #[serde(default)]
    effectful_unnameable: Vec<String>,
    #[serde(default)]
    effect_free: Vec<String>,
    #[serde(default)]
    shared: BTreeMap<String, usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LibcClassification {
    effect: Vec<String>,
    not_an_effect: Vec<String>,
}

fn wrappers() -> Wrappers {
    let text = fs::read_to_string(repo_root().join(WRAPPERS_TOML)).expect("effects/wrappers.toml");
    wrappers_from(&text)
}

fn wrappers_from(text: &str) -> Wrappers {
    toml::from_str(text).expect("the wrapper classification parses")
}

#[test]
fn the_readiness_expectations_are_per_site_and_both_records_say_so() {
    const READINESS: &str = "src/agent/proc/test_support/readiness.rs";
    const NOTES: &str = "docs/internals/agent/proc/test_support/readiness.md";
    const LINT: &str = "clippy::disallowed_methods";
    const SITES: usize = 6;
    const DECISION: &str = "standards/02_standards_automated_baseline.md";
    const SPELLED: [&str; 8] = [
        "one", "two", "three", "four", "five", "six", "seven", "eight",
    ];
    let sites_in_words = SPELLED[SITES - 1];

    let source = fs::read_to_string(repo_root().join(READINESS)).expect("the readiness module");

    for lint in USED_GOVERNED_LINTS {
        let level = if *lint == LINT { "deny" } else { "forbid" };
        assert_eq!(
            crate::effects::lint_levels::file_level_lint_state(&source, lint),
            Some(level),
            "{READINESS} must {level} `{lint}` at file-module level: `deny` for the one lint its \
             per-site expectations narrow, where `forbid` is E0453 at each of them, and `forbid` \
             for every other, which nothing in the file lowers"
        );
    }

    let found = governed_allows(&source);
    let per_site: Vec<&crate::effects::GovernedAllow> =
        found.iter().filter(|allow| !allow.module_level).collect();
    assert_eq!(
        per_site.len(),
        SITES,
        "{READINESS} carries {} per-site governed attributes: {per_site:#?}",
        per_site.len()
    );
    assert!(
        found.len() == SITES,
        "a governed attribute at module level is a file-scope allowance and this file has \
         none: {found:#?}"
    );
    for allow in &per_site {
        assert_eq!(allow.keywords, ["expect"], "{READINESS}:{}", allow.line);
        assert_eq!(allow.written, [LINT], "{READINESS}:{}", allow.line);
        assert!(allow.reasoned, "{READINESS}:{} has no reason", allow.line);
    }
    let indices: BTreeSet<usize> = (1..=SITES)
        .filter(|index| source.contains(&format!("site {index} of {SITES}")))
        .collect();
    assert_eq!(
        indices,
        (1..=SITES).collect::<BTreeSet<usize>>(),
        "each expectation's reason names which of the {SITES} sites it is"
    );

    let list = allowlist();
    let row = list
        .funnel
        .iter()
        .find(|entry| entry.path == READINESS)
        .expect("the readiness row is in the funnel section");
    assert_eq!(row.allows, vec![LINT.to_owned()]);
    assert_eq!(row.expect_sites, SITES);

    let phrase = format!("five distinct denied paths across {sites_in_words} sites");
    let shouted = phrase.to_uppercase();
    let allowlist_text =
        fs::read_to_string(repo_root().join(ALLOWLIST_TOML)).expect("the allowlist");
    let notes = fs::read_to_string(repo_root().join(NOTES)).expect("the readiness notes");
    for (record, text, needle) in [
        (NOTES, notes.as_str(), phrase.as_str()),
        (ALLOWLIST_TOML, allowlist_text.as_str(), shouted.as_str()),
    ] {
        for spelling in [text.to_owned(), text.replace('\n', "\r\n")] {
            assert!(
                spelling.lines().any(|line| line.contains(needle)),
                "{record} no longer states `{needle}` on a line of its own"
            );
        }
        assert!(
            text.contains(DECISION),
            "{record} does not cite `{DECISION}`, which is what admits the placement"
        );
    }

    assert!(
        repo_root().join(DECISION).is_file(),
        "`{DECISION}` is cited by both records and is not in the tree"
    );
}

#[test]
fn the_internals_readme_names_the_records_that_carry_the_readiness_statement() {
    const README: &str = "docs/internals/README.md";
    const NOTES: &str = "docs/internals/agent/proc/test_support/readiness.md";
    const READINESS: &str = "src/agent/proc/test_support/readiness.rs";
    const SECTION: &str = "\n## What moves\n";

    let readme = fs::read_to_string(repo_root().join(README))
        .expect("the internals README")
        .replace("\r\n", "\n");
    let (_, below) = readme
        .split_once(SECTION)
        .unwrap_or_else(|| panic!("{README} no longer has a `What moves` section"));
    let what_moves = below
        .split_once("\n## ")
        .map_or(below, |(section, _)| section);

    for record in [NOTES, ALLOWLIST_TOML] {
        assert!(
            what_moves.contains(record),
            "{README}'s `What moves` section does not name `{record}`, which is one of the two \
             records `the_readiness_expectations_are_per_site_and_both_records_say_so` reads the \
             per-site allowance statement from"
        );
    }
    assert!(
        !what_moves.contains(READINESS),
        "{README}'s `What moves` section names `{READINESS}` as prose a census reads. The \
         statement moved to `{NOTES}` and `{ALLOWLIST_TOML}`; the module keeps its marker and \
         nothing else, so a maintainer sent to the source finds no such sentence"
    );
}

fn file_level_denies(source: &str, lint: &str) -> bool {
    matches!(
        crate::effects::lint_levels::file_level_lint_state(source, lint),
        Some("deny" | "forbid")
    )
}

#[test]
fn every_allow_of_a_governed_lint_is_module_level_and_in_the_allowlist() {
    let list = allowlist();
    let recorded: BTreeMap<&str, (&AllowlistEntry, &'static str)> = list
        .funnel
        .iter()
        .map(|entry| (entry.path.as_str(), (entry, "funnel")))
        .chain(
            list.legacy
                .iter()
                .map(|entry| (entry.path.as_str(), (entry, "legacy"))),
        )
        .collect();
    assert_eq!(
        recorded.len(),
        list.funnel.len() + list.legacy.len(),
        "a path is listed in both sections, or twice in one"
    );

    let mut carried: BTreeSet<String> = BTreeSet::new();
    let mut attributes = 0;
    for (path, source) in scanned_sources() {
        let found = governed_allows(&source);
        if found.is_empty() {
            continue;
        }
        attributes += found.len();
        let Some((entry, section)) = recorded.get(path.as_str()) else {
            panic!(
                "{path} allows a governed lint and is in no section of {ALLOWLIST_TOML}: {found:#?}"
            );
        };
        carried.insert(path.clone());
        let mut per_site = 0;
        for allow in &found {
            if !allow.module_level
                && allow.keywords == ["expect"]
                && entry.expect_sites > 0
                && allow.reasoned
                && allow
                    .lints
                    .iter()
                    .all(|lint| file_level_denies(&source, lint))
            {
                per_site += 1;
                continue;
            }
            assert!(
                allow.module_level,
                "{path}:{} allows {:?} below module level; `mechanism` (2) permits it \
                 \"only as module-level attributes\", and the per-site `#[expect]` the \
                 2026-08-30 amendment admits needs a reason, a file-level deny of the same \
                 lint, and an `expect_sites` count in {ALLOWLIST_TOML}",
                allow.line, allow.lints
            );
            let marker = marker_before(&source, allow.line, allow.inner);
            assert!(
                marker.contains(ALLOWLIST_TOML),
                "{path}:{} carries no pointer to {ALLOWLIST_TOML} above the attribute",
                allow.line
            );
            let expected_marker = if *section == "legacy" {
                "LEGACY-EFFECT"
            } else {
                "funnel section"
            };
            assert!(
                marker.contains(expected_marker),
                "{path}:{} is in the {section} section and its prologue never says \
                 `{expected_marker}`",
                allow.line
            );
        }
        let written: BTreeSet<&str> = found
            .iter()
            .flat_map(|allow| allow.written.iter().map(String::as_str))
            .filter(|entry| normalize_lint(entry).is_some())
            .collect();
        let declared: BTreeSet<&str> = entry.allows.iter().map(String::as_str).collect();
        assert_eq!(
            written, declared,
            "{path}: the attribute allows {written:?} and {ALLOWLIST_TOML} records {declared:?}"
        );
        assert_eq!(
            per_site, entry.expect_sites,
            "{path} carries {per_site} per-site `#[expect]` attributes and {ALLOWLIST_TOML} \
             records {}",
            entry.expect_sites
        );
    }

    for (path, (entry, _)) in &recorded {
        assert!(
            entry.expect_sites == 0 || carried.contains(*path),
            "{path} records {} per-site expectations and carries no governed attribute",
            entry.expect_sites
        );
    }

    for (path, (entry, _)) in &recorded {
        if entry.allows.is_empty() || entry.absent {
            continue;
        }
        assert!(
            carried.contains(*path),
            "{path} records allows {:?} and carries no attribute",
            entry.allows
        );
    }
    assert!(
        attributes >= 25,
        "the scan found only {attributes} governed attributes; it is measuring nothing"
    );
}

fn module_directory(path: &Path) -> PathBuf {
    let heads_its_directory = path
        .file_name()
        .is_some_and(|file| file == "mod.rs" || file == "lib.rs" || file == "main.rs");
    match path.parent() {
        Some(directory) if heads_its_directory => directory.to_path_buf(),
        _ => path.with_extension(""),
    }
}

fn governed_deny_lists_written_anywhere(source: &str) -> Vec<BTreeSet<&'static str>> {
    let text: String = blank_comments_and_strings(source)
        .chars()
        .filter(|character| !super::is_rustc_whitespace(*character))
        .collect();
    let used = governed_lints_in_use();
    let mut lists = Vec::new();
    let mut pieces = text.split("deny(");
    let mut before = pieces.next().unwrap_or_default();
    for piece in pieces {
        let part_of_a_longer_word = before
            .chars()
            .next_back()
            .is_some_and(|last| last.is_alphanumeric() || last == '_');
        if !part_of_a_longer_word {
            let list: BTreeSet<&'static str> = piece
                .split(')')
                .next()
                .unwrap_or_default()
                .split(',')
                .filter_map(normalize_lint)
                .filter(|lint| used.contains(*lint))
                .collect();
            if !list.is_empty() {
                lists.push(list);
            }
        }
        before = piece;
    }
    lists
}

fn fences_that_deny_where_forbid_would_compile(sources: &[(String, String)]) -> Vec<String> {
    let allowed: Vec<(&Path, BTreeSet<&'static str>)> = sources
        .iter()
        .map(|(path, source)| {
            let lints: BTreeSet<&'static str> = governed_allows(source)
                .iter()
                .flat_map(|allow| allow.lints.iter())
                .filter_map(|lint| normalize_lint(lint))
                .collect();
            (Path::new(path), lints)
        })
        .filter(|(_, lints)| !lints.is_empty())
        .collect();
    let mut wrong = Vec::new();
    for (path, source) in sources {
        let read: BTreeSet<&'static str> = USED_GOVERNED_LINTS
            .iter()
            .filter(|lint| {
                crate::effects::lint_levels::file_level_lint_state(source, lint) == Some("deny")
            })
            .filter_map(|lint| normalize_lint(lint))
            .collect();
        let lists = governed_deny_lists_written_anywhere(source);
        let swept: BTreeSet<&'static str> = lists.iter().flatten().copied().collect();
        for lint in read.difference(&swept) {
            wrong.push(format!(
                "{path}: `file_level_lint_state` reads `deny` of `{lint}` and the sweep for a \
                 written `deny(` found none, so the two disagree and this census measures nothing"
            ));
        }
        let file = Path::new(path);
        let below = module_directory(file);
        let refused: BTreeSet<&'static str> = allowed
            .iter()
            .filter(|(other, _)| *other == file || other.starts_with(&below))
            .flat_map(|(_, lints)| lints.iter().copied())
            .collect();
        for list in lists {
            if !list.is_disjoint(&refused) {
                continue;
            }
            wrong.push(format!(
                "{path} fences {list:?} with `deny`, and no allowance of any of them sits in it \
                 or in a module file below it, so `forbid` compiles here and would make an inner \
                 `allow` E0453 instead of a level an attribute can reopen{}",
                if list.is_subset(&read) {
                    ""
                } else {
                    "; `file_level_lint_state` does not read this attribute as the file's level"
                }
            ));
        }
    }
    wrong
}

#[test]
fn the_fence_rule_names_a_deny_that_could_forbid_and_excuses_one_that_could_not() {
    fn tree(files: &[(&str, &str)]) -> Vec<(String, String)> {
        files
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
            .collect()
    }
    const DENY: &str = "#![deny(clippy::disallowed_methods)]\nfn go() {}\n";
    const FORBID: &str = "#![forbid(clippy::disallowed_methods)]\nfn go() {}\n";
    const ALLOW: &str = "#![allow(clippy::disallowed_methods)]\nfn go() {}\n";

    let named = fences_that_deny_where_forbid_would_compile(&tree(&[
        ("src/a.rs", FORBID),
        ("src/a/b.rs", DENY),
        ("src/c.rs", ALLOW),
    ]));
    assert_eq!(named.len(), 1, "{named:#?}");
    assert!(
        named.iter().all(|line| line.starts_with("src/a/b.rs ")),
        "the refusal names the file that dropped to `deny`: {named:#?}"
    );

    for (what, files) in [
        ("every fence forbids", vec![("src/a.rs", FORBID)]),
        (
            "an out-of-line child allows",
            vec![("src/a.rs", DENY), ("src/a/tests.rs", ALLOW)],
        ),
        (
            "a grandchild under a `mod.rs` allows",
            vec![("src/a/mod.rs", DENY), ("src/a/b/c.rs", ALLOW)],
        ),
        (
            "a module under a `lib.rs` allows",
            vec![("src/lib.rs", DENY), ("src/a.rs", ALLOW)],
        ),
        (
            "a module under a `main.rs` allows",
            vec![("src/main.rs", DENY), ("src/a/b.rs", ALLOW)],
        ),
        (
            "one attribute fences three lints and the child allows one of them",
            vec![
                (
                    "src/a.rs",
                    "#![deny(\n    clippy::disallowed_methods,\n    clippy::disallowed_types,\n    \
                     clippy::disallowed_macros\n)]\n",
                ),
                ("src/a/tests.rs", ALLOW),
            ],
        ),
        (
            "the file carries its own per-site expectation",
            vec![(
                "src/a.rs",
                "#![deny(clippy::disallowed_methods)]\n\
                 #[expect(clippy::disallowed_methods, reason = \"site 1 of 1\")]\nfn go() {}\n",
            )],
        ),
        (
            "`deny` is spelled only in a comment and a string",
            vec![(
                "src/a.rs",
                "// #![deny(clippy::disallowed_methods)]\n\
                 const F: &str = \"#![deny(clippy::disallowed_methods)]\";\n",
            )],
        ),
        (
            "the lint is not a governed one",
            vec![("src/a.rs", "#![deny(clippy::indexing_slicing)]\n")],
        ),
    ] {
        let named = fences_that_deny_where_forbid_would_compile(&tree(&files));
        assert!(named.is_empty(), "{what}: {named:#?}");
    }

    for (what, files) in [
        (
            "a sibling's allowance is not below the fence",
            vec![("src/a.rs", DENY), ("src/b.rs", ALLOW)],
        ),
        (
            "a name that only starts like the fence's is not below it",
            vec![("src/a.rs", DENY), ("src/ab/tests.rs", ALLOW)],
        ),
        (
            "the parent's allowance is above the fence, which is what it fences against",
            vec![("src/a.rs", ALLOW), ("src/a/b.rs", DENY)],
        ),
        (
            "the file's own allowance is of another lint, in another attribute",
            vec![(
                "src/a.rs",
                "#![allow(clippy::disallowed_methods)]\n#![deny(clippy::disallowed_macros)]\n",
            )],
        ),
        (
            "the allowance below is of another lint",
            vec![
                ("src/a.rs", "#![deny(clippy::disallowed_types)]\n"),
                ("src/a/tests.rs", ALLOW),
            ],
        ),
    ] {
        let named = fences_that_deny_where_forbid_would_compile(&tree(&files));
        assert_eq!(named.len(), 1, "{what}: {named:#?}");
    }

    for spelling in [
        "# ![deny(clippy::disallowed_methods)]\n",
        "#! [deny(clippy::disallowed_methods)]\n",
        "#![deny (clippy::disallowed_methods)]\n",
        "#![deny(\n    clippy::disallowed_types,\n    clippy :: disallowed_methods\n)]\n",
        "#![cfg_attr(all(), deny(clippy::disallowed_methods))]\n",
        "fn go() {}\n#[deny(clippy::disallowed_methods)]\nfn late() {}\n",
    ] {
        let named = fences_that_deny_where_forbid_would_compile(&tree(&[("src/a.rs", spelling)]));
        assert_eq!(named.len(), 1, "{spelling:?}: {named:#?}");
        assert!(
            named.iter().all(|line| line.starts_with("src/a.rs ")),
            "{spelling:?}: {named:#?}"
        );
    }

    let mixed = "#![forbid(clippy::disallowed_types)]\n#![deny(clippy::disallowed_methods)]\n";
    let named = fences_that_deny_where_forbid_would_compile(&tree(&[("src/a.rs", mixed)]));
    assert_eq!(named.len(), 1, "{named:#?}");
    assert!(
        named
            .iter()
            .all(|line| line.contains("disallowed_methods") && !line.contains("disallowed_types")),
        "only the lint still at `deny` is named: {named:#?}"
    );
}

#[test]
fn every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile() {
    let sources = scanned_sources();
    let forbidding = sources
        .iter()
        .filter(|(_, source)| {
            USED_GOVERNED_LINTS.iter().any(|lint| {
                crate::effects::lint_levels::file_level_lint_state(source, lint) == Some("forbid")
            })
        })
        .count();
    assert!(
        forbidding > 0,
        "no scanned file forbids a governed lint, so the sweep below is measuring nothing"
    );
    let wrong = fences_that_deny_where_forbid_would_compile(&sources);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

fn is_whole_file_test_module(path: &str) -> bool {
    path.strip_prefix("src/").is_some_and(|under_src| {
        WHOLE_FILE_TEST_MODULES
            .iter()
            .any(|module| module.to_string_lossy().replace('\\', "/") == under_src)
    })
}

use super::census_domain::{Predicate, parse_predicate, with_literal_identity};
use super::lint_levels::{
    Applied, applied_attributes, attribute_arguments, attribute_name, top_level_arguments,
};

struct ReadAttribute {
    start: usize,
    end: usize,
    inner: bool,
    stack: usize,
    name: String,
    text: Option<String>,
}

fn attributes_in_stacks(source: &str, blanked: &str) -> Vec<ReadAttribute> {
    let bytes = blanked.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    let mut stack = 0;
    let mut previous: Option<(usize, bool)> = None;
    while let Some(byte) = bytes.get(at) {
        if *byte != b'#' {
            at += 1;
            continue;
        }
        let bang = skip_whitespace(bytes, at + 1);
        let inner = bytes.get(bang) == Some(&b'!');
        let open = if inner {
            skip_whitespace(bytes, bang + 1)
        } else {
            bang
        };
        if bytes.get(open) != Some(&b'[') {
            at += 1;
            continue;
        }
        let Some(close) = super::matching(bytes, open, b'[', b']') else {
            break;
        };
        let contiguous = previous.is_some_and(|(end, was_inner)| {
            was_inner == inner
                && blanked
                    .get(end..at)
                    .is_some_and(|gap| gap.bytes().all(|byte| byte.is_ascii_whitespace()))
        });
        if !contiguous {
            stack += 1;
        }
        let shape = blanked.get(open + 1..close).unwrap_or_default();
        found.push(ReadAttribute {
            start: at,
            end: close + 1,
            inner,
            stack,
            name: attribute_name(shape.trim()).to_owned(),
            text: source
                .get(open + 1..close)
                .and_then(|raw| with_literal_identity(raw, shape)),
        });
        previous = Some((close + 1, inner));
        at = close + 1;
    }
    found
}

fn skip_whitespace(bytes: &[u8], from: usize) -> usize {
    let mut at = from;
    while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}

struct Gate {
    from: usize,
    to: usize,
    predicate: Option<Predicate>,
}

fn under_every_predicate(under: Vec<Result<Predicate, String>>) -> Option<Vec<Predicate>> {
    under.into_iter().collect::<Result<Vec<_>, _>>().ok()
}

fn generated_gates(attribute: &ReadAttribute) -> Vec<Option<Predicate>> {
    if !matches!(attribute.name.as_str(), "cfg" | "cfg_attr") {
        return Vec::new();
    }
    let Some(text) = &attribute.text else {
        return vec![None];
    };
    applied_attributes(text.trim())
        .into_iter()
        .filter(|applied| attribute_name(applied.text) == "cfg")
        .map(|Applied { text, under }| {
            let gate = parse_predicate(attribute_arguments(text).unwrap_or_default()).ok()?;
            let under = under_every_predicate(under)?;
            Some(if under.is_empty() {
                gate
            } else {
                Predicate::Any(vec![Predicate::Not(Box::new(Predicate::All(under))), gate])
            })
        })
        .collect()
}

fn brace_blocks(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut open = Vec::new();
    let mut blocks = Vec::new();
    for (at, byte) in bytes.iter().enumerate() {
        match byte {
            b'{' => open.push(at),
            b'}' => blocks.extend(open.pop().map(|from| (from, at))),
            _ => {}
        }
    }
    blocks
}

fn gates_in_the_file(blanked: &str, attributes: &[ReadAttribute]) -> Vec<Gate> {
    let bytes = blanked.as_bytes();
    let blocks = brace_blocks(bytes);
    let mut gates = Vec::new();
    for attribute in attributes {
        let predicates = generated_gates(attribute);
        if predicates.is_empty() {
            continue;
        }
        let (from, to) = if attribute.inner {
            blocks
                .iter()
                .filter(|(open, close)| *open < attribute.start && attribute.start < *close)
                .min_by_key(|(open, close)| close - open)
                .map_or((0, bytes.len()), |(open, close)| (*open, close + 1))
        } else {
            let stack = attributes
                .iter()
                .filter(|other| !other.inner && other.stack == attribute.stack);
            let first = stack
                .clone()
                .map(|other| other.start)
                .min()
                .unwrap_or(attribute.start);
            let last = stack.map(|other| other.end).max().unwrap_or(attribute.end);
            let item = skip_whitespace(bytes, last);
            (first, super::configured_item_end(bytes, item).max(last))
        };
        gates.extend(predicates.into_iter().map(|predicate| Gate {
            from,
            to,
            predicate,
        }));
    }
    gates
}

fn governed_lints_allowed_by(applied: &str) -> Vec<&'static str> {
    if !matches!(attribute_name(applied), "allow" | "expect") {
        return Vec::new();
    }
    attribute_arguments(applied).map_or_else(Vec::new, |list| {
        top_level_arguments(list)
            .into_iter()
            .filter_map(normalize_lint)
            .collect()
    })
}

fn allowance_applies_in_a_production_build(
    at: usize,
    under: Vec<Result<Predicate, String>>,
    gates: &[Gate],
) -> bool {
    let Some(mut conjuncts) = under_every_predicate(under) else {
        return false;
    };
    for gate in gates
        .iter()
        .filter(|gate| (gate.from..gate.to).contains(&at))
    {
        let Some(predicate) = &gate.predicate else {
            return false;
        };
        conjuncts.push(predicate.clone());
    }
    some_production_build_satisfies(&Predicate::All(conjuncts))
}

fn some_production_build_satisfies(predicate: &Predicate) -> bool {
    CI_TARGETS
        .iter()
        .any(|target| holds_in_the_production_build(predicate, target) == Some(true))
}

fn holds_in_the_production_build(
    predicate: &Predicate,
    target: &ci_model::CiTarget,
) -> Option<bool> {
    match predicate {
        Predicate::Test => Some(false),
        Predicate::Other(written) => atom_in_the_production_build(written, target),
        Predicate::Not(inner) => holds_in_the_production_build(inner, target).map(|value| !value),
        Predicate::All(parts) => {
            let mut decided = Some(true);
            for part in parts {
                match holds_in_the_production_build(part, target) {
                    Some(false) => return Some(false),
                    Some(true) => {}
                    None => decided = None,
                }
            }
            decided
        }
        Predicate::Any(parts) => {
            let mut decided = Some(false);
            for part in parts {
                match holds_in_the_production_build(part, target) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => decided = None,
                }
            }
            decided
        }
    }
}

fn atom_in_the_production_build(written: &str, target: &ci_model::CiTarget) -> Option<bool> {
    let Some((key, value)) = written.split_once('=') else {
        let flag = written.trim();
        return CI_TARGETS
            .iter()
            .any(|each| each.flags.contains(&flag))
            .then(|| target.flags.contains(&flag));
    };
    let key = key.trim();
    let value = super::census_domain::literal_token_value(value.trim())?;
    CI_TARGETS
        .iter()
        .all(|each| each.keys.iter().any(|(name, _)| *name == key))
        .then(|| {
            target
                .keys
                .iter()
                .any(|(name, set)| *name == key && *set == value)
        })
}

fn governed_allows_in_the_production_build(source: &str) -> BTreeSet<&'static str> {
    let blanked = blank_comments_and_strings(source);
    let attributes = attributes_in_stacks(source, &blanked);
    let gates = gates_in_the_file(&blanked, &attributes);
    let mut allowed = BTreeSet::new();
    for attribute in &attributes {
        let Some(text) = &attribute.text else {
            continue;
        };
        let recorded: BTreeSet<String> = governed_allows(
            source
                .get(attribute.start..attribute.end)
                .unwrap_or_default(),
        )
        .into_iter()
        .flat_map(|allow| allow.lints)
        .collect();
        for Applied { text, under } in applied_attributes(text.trim()) {
            let lints: Vec<&'static str> = governed_lints_allowed_by(text)
                .into_iter()
                .filter(|lint| recorded.contains(*lint))
                .collect();
            if !lints.is_empty()
                && allowance_applies_in_a_production_build(attribute.start, under, &gates)
            {
                allowed.extend(lints);
            }
        }
    }
    allowed
}

fn denies_the_production_build_could_forbid(
    sources: &[(String, String)],
    is_test_module: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let by_path: BTreeMap<String, String> = sources.iter().cloned().collect();
    let test_code = |path: &str| {
        is_test_module(path)
            || ancestor_module_files(path, &by_path)
                .iter()
                .any(|ancestor| is_test_module(ancestor))
    };
    let allowed: Vec<(&Path, BTreeSet<&'static str>)> = sources
        .iter()
        .filter(|(path, _)| !test_code(path))
        .map(|(path, source)| {
            (
                Path::new(path),
                governed_allows_in_the_production_build(source),
            )
        })
        .filter(|(_, lints)| !lints.is_empty())
        .collect();
    let mut named = Vec::new();
    for (path, source) in sources {
        if test_code(path) {
            continue;
        }
        let file = Path::new(path);
        let below = module_directory(file);
        for lint in USED_GOVERNED_LINTS {
            if crate::effects::lint_levels::file_level_lint_state(source, lint) != Some("deny") {
                continue;
            }
            let Some(bare) = normalize_lint(lint) else {
                continue;
            };
            let excused = allowed.iter().any(|(other, lints)| {
                (*other == file || other.starts_with(&below)) && lints.contains(bare)
            });
            if !excused {
                named.push(format!(
                    "{path}: `{lint}` is `deny` at file level, and every allowance of it in the \
                     file or in a module file below it sits in test code, so \
                     `#![cfg_attr(not(test), forbid({lint}))]` compiles in the production build \
                     and would make an inner `allow` there E0453 instead of a level an attribute \
                     can reopen"
                ));
            }
        }
    }
    named
}

#[test]
fn the_production_fence_rule_names_a_deny_only_test_code_excuses_and_excuses_one_production_code_does()
 {
    fn tree(files: &[(&str, &str)]) -> Vec<(String, String)> {
        files
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
            .collect()
    }
    fn named(files: &[(&str, &str)], test_modules: &[&str]) -> Vec<String> {
        denies_the_production_build_could_forbid(&tree(files), &|path| test_modules.contains(&path))
    }
    const DENY: &str = "#![deny(clippy::disallowed_methods)]\nfn go() {}\n";
    const ALLOW: &str = "#![allow(clippy::disallowed_methods)]\nfn go() {}\n";
    const INLINE_TEST_ALLOW: &str =
        "#[cfg(test)]\n#[allow(clippy::disallowed_methods)]\nmod tests {\n    fn t() {}\n}\n";

    for (what, files, test_modules) in [
        (
            "the shape src/agent/bin.rs carried at e851b676: the file's only allowance is an outer \
             attribute on its inline `#[cfg(test)]` module",
            vec![("src/a.rs", &*format!("{DENY}{INLINE_TEST_ALLOW}"))],
            vec![],
        ),
        (
            "the attributes of the inline test module in the other order",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[allow(clippy::disallowed_methods)]\n#[cfg(test)]\nmod tests {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "an inner allowance written inside the `#[cfg(test)]` module's braces",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(test)]\nmod tests {{\n    #![allow(clippy::disallowed_methods)]\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "the only allowance below is a whole-file test module",
            vec![("src/a.rs", DENY), ("src/a/tests.rs", ALLOW)],
            vec!["src/a/tests.rs"],
        ),
        (
            "no allowance below at all",
            vec![("src/a.rs", DENY)],
            vec![],
        ),
        (
            "a sibling's allowance is not below the fence",
            vec![("src/a.rs", DENY), ("src/b.rs", ALLOW)],
            vec![],
        ),
        (
            "the file's own allowance is of another lint",
            vec![(
                "src/a.rs",
                "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::disallowed_types)]\n",
            )],
            vec![],
        ),
        (
            "the inline test module is gated by `cfg(all(test))`",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(all(test))]\n#[allow(clippy::disallowed_methods)]\nmod tests {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "`cfg(all(test))` written after the allowance",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[allow(clippy::disallowed_methods)]\n#[cfg(all(test))]\nmod tests {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "`cfg(all(test, unix))`: a test module on one platform, a production module on none",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(all(test, unix))]\n#[allow(clippy::disallowed_methods)]\nmod tests {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "`cfg(any(test))`",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(any(test))]\n#[allow(clippy::disallowed_methods)]\nmod tests {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "two `cfg` attributes on the module, `cfg(test)` and `cfg(unix)`",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(test)]\n#[cfg(unix)]\n#[allow(clippy::disallowed_methods)]\nmod tests {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "the allowance itself is applied only under `cfg_attr(test, ..)`",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg_attr(test, allow(clippy::disallowed_methods))]\nmod tests {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "a per-site expectation applied only under `cfg_attr(test, ..)`",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg_attr(test, expect(clippy::disallowed_methods))]\nfn t() {{}}\n"
                ),
            )],
            vec![],
        ),
        (
            "an item no build compiles, `cfg(any())`",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(any())]\n#[allow(clippy::disallowed_methods)]\nmod never {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "an inner allowance inside a module gated by `cfg(all(test, unix))`",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(all(test, unix))]\nmod tests {{\n    #![allow(clippy::disallowed_methods)]\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "an inner allowance in a module nested inside one gated by `cfg(all(test, unix))`",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(all(test, unix))]\npub(crate) mod tests {{\n    mod deeper {{\n        \
                     #![allow(clippy::disallowed_methods)]\n    }}\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "a `cfg(not(not(test)))` module is a test module however it is spelled",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(not(not(test)))]\n#[allow(clippy::disallowed_methods)]\nmod tests {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "a feature no CI valuation sets establishes no production build",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(feature = \"x\")]\n#[allow(clippy::disallowed_methods)]\nmod m {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "an allowance only a macro's expansion writes, which the placement census does not \
             read",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}macro_rules! lower {{\n    ($level:ident) => {{\n        \
                     #[$level(clippy::disallowed_methods)]\n        mod m {{}}\n    }};\n}}\n\
                     lower!(allow);\n"
                ),
            )],
            vec![],
        ),
    ] {
        let found = named(&files, &test_modules);
        assert_eq!(found.len(), 1, "{what}: {found:#?}");
        assert!(
            found.iter().all(|line| line.starts_with("src/a.rs: ")),
            "{what}: {found:#?}"
        );
    }

    for (what, files, test_modules) in [
        (
            "the repair: `forbid` in the production build, the inline test allowance kept",
            vec![(
                "src/a.rs",
                &*format!(
                    "#![cfg_attr(not(test), forbid(clippy::disallowed_methods))]\n{INLINE_TEST_ALLOW}"
                ),
            )],
            vec![],
        ),
        (
            "every fence forbids",
            vec![(
                "src/a.rs",
                "#![forbid(clippy::disallowed_methods)]\nfn go() {}\n",
            )],
            vec![],
        ),
        (
            "a production child allows the lint, so `forbid` is E0453 in every build",
            vec![("src/a.rs", DENY), ("src/a/b.rs", ALLOW)],
            vec![],
        ),
        (
            "the file's own production region allows the lint",
            vec![(
                "src/a.rs",
                "#![deny(clippy::disallowed_methods)]\n#[allow(clippy::disallowed_methods)]\nfn go() {}\n",
            )],
            vec![],
        ),
        (
            "an allowance whose tokens are written apart, which rustc applies and the placement \
             census reads",
            vec![(
                "src/a.rs",
                &*format!("{DENY}# [allow (clippy::disallowed_methods)]\nmod m {{\n}}\n"),
            )],
            vec![],
        ),
        (
            "a platform-gated allowance is production code on that platform",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(windows)]\n#[allow(clippy::disallowed_methods)]\nmod win {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "a whole-file test module that fences has no production region to forbid in",
            vec![("src/a/tests.rs", DENY)],
            vec!["src/a/tests.rs"],
        ),
        (
            "a `deny` the file-level reader does not read is the sweep's, not this rule's",
            vec![(
                "src/a.rs",
                "fn go() {}\n#[deny(clippy::disallowed_methods)]\nfn late() {}\n",
            )],
            vec![],
        ),
        (
            "a `cfg(not(test))` module is production code",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(not(test))]\n#[allow(clippy::disallowed_methods)]\nmod production {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "an allowance applied under `cfg_attr(not(test), ..)` is a production allowance",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg_attr(not(test), allow(clippy::disallowed_methods))]\nmod m {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "`cfg(all())` holds in every build",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(all())]\n#[allow(clippy::disallowed_methods)]\nmod m {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "`cfg(any(test, unix))` holds in a production build on one platform",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(any(test, unix))]\n#[allow(clippy::disallowed_methods)]\nmod m {{\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "an inner allowance inside a `cfg(unix)` module",
            vec![(
                "src/a.rs",
                &*format!(
                    "{DENY}#[cfg(unix)]\nmod m {{\n    #![allow(clippy::disallowed_methods)]\n}}\n"
                ),
            )],
            vec![],
        ),
        (
            "a raw attribute name is the attribute",
            vec![(
                "src/a.rs",
                &*format!("{DENY}#[r#allow(clippy::disallowed_methods)]\nmod m {{\n}}\n"),
            )],
            vec![],
        ),
    ] {
        let found = named(&files, &test_modules);
        assert!(found.is_empty(), "{what}: {found:#?}");
    }
}

const NO_PRODUCTION_BUILD_APPLIES: &[(&str, &str)] = &[
    (
        "nested_platform_test",
        "#[cfg_attr(unix, cfg_attr(test, allow(LINT)))]\nmod m {}\n",
    ),
    (
        "nested_inactive",
        "#[cfg_attr(not(test), cfg_attr(test, allow(LINT)))]\nmod m {}\n",
    ),
    (
        "nested_all_test",
        "#[cfg_attr(all(), cfg_attr(test, allow(LINT)))]\nmod m {}\n",
    ),
    (
        "nested_never",
        "#[cfg_attr(not(test), cfg_attr(any(), allow(LINT)))]\nmod m {}\n",
    ),
    (
        "nested_expect",
        "#[cfg_attr(unix, cfg_attr(test, expect(LINT)))]\nmod m {}\n",
    ),
    (
        "conditional_cfg_test",
        "#[cfg_attr(not(test), cfg(test))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "conditional_cfg_never",
        "#[cfg_attr(not(test), cfg(any()))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "conditional_cfg_before",
        "#[allow(LINT)]\n#[cfg_attr(not(test), cfg(any()))]\nmod m {}\n",
    ),
    (
        "conditional_cfg_inside",
        "#[cfg_attr(not(test), cfg(any()))]\nmod m { #![allow(LINT)] }\n",
    ),
    ("literal_false", "#[cfg(any())]\n#[allow(LINT)]\nmod m {}\n"),
    (
        "composite_test",
        "#[cfg(all(test, unix))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "outer_test_os",
        "#[cfg(all(test, target_os = \"linux\"))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "attr_test_os",
        "#[cfg_attr(all(test, target_os = \"linux\"), allow(LINT))]\nmod m {}\n",
    ),
    (
        "test_feature",
        "#[cfg(all(test, feature = \"main_r3\"))]\n#[expect(LINT)]\nmod m {}\n",
    ),
    (
        "inactive_feature",
        "#[cfg(all(any(), feature = \"never\"))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "enclosing_test_os",
        "#[cfg(all(test, target_os = \"linux\"))]\nmod m { #![allow(LINT)] }\n",
    ),
    (
        "conditional_cfg_test_inside",
        "#[cfg_attr(not(test), cfg(test))]\nmod m { #![allow(LINT)] }\n",
    ),
    (
        "inactive_enclosing_function",
        "#[cfg(any())]\nfn f() { #[allow(LINT)] let _ = 1; }\n",
    ),
    (
        "two_scopes_one_line",
        "#[cfg_attr(not(test), allow(dead_code))] #[cfg_attr(test, allow(LINT))]\nmod m {}\n",
    ),
    (
        "escaped_quote_in_a_test_conjunction",
        "#[cfg(all(feature = \"a\\\"\", test, feature = \"b\\\"\"))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "correlated_generated_gate",
        "#[cfg_attr(unix, cfg(test), allow(LINT))]\nmod m {}\n",
    ),
    (
        "contradictory_platforms",
        "#[cfg(all(unix, windows))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "contradictory_values",
        "#[cfg(all(target_os = \"linux\", target_os = \"macos\"))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "nested_contradiction",
        "#[cfg_attr(unix, cfg_attr(target_family = \"windows\", allow(LINT)))]\nmod m {}\n",
    ),
    (
        "self_contradiction",
        "#[cfg(all(target_os = \"linux\", not(target_os = \"linux\")))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "enclosing_module_gated_twice",
        "#[cfg(test)]\nmod outer {\n    #[cfg(unix)]\n    mod inner {\n        #![allow(LINT)]\n    }\n}\n",
    ),
    (
        "raw_spelling_negated",
        "#[cfg_attr(all(target_os = \"linux\", not(target_os = r\"linux\")), allow(LINT))]\nmod m {}\n",
    ),
    (
        "hashed_raw_spelling_negated",
        "#[cfg_attr(all(target_os = \"linux\", not(target_os = r###\"linux\"###)), allow(LINT))]\nmod m {}\n",
    ),
    (
        "byte_escape_negated",
        "#[cfg_attr(all(target_os = \"linux\", not(target_os = \"lin\\x75x\")), allow(LINT))]\nmod m {}\n",
    ),
    (
        "unicode_escape_negated",
        "#[cfg_attr(all(target_os = \"linux\", not(target_os = \"lin\\u{75}x\")), allow(LINT))]\nmod m {}\n",
    ),
    (
        "underscored_unicode_escape_negated",
        "#[cfg_attr(all(target_os = \"lin\\u{7_5}x\", not(target_os = r#\"linux\"#)), allow(LINT))]\nmod m {}\n",
    ),
    (
        "continued_line_negated",
        "#[cfg_attr(all(target_os = \"lin\\\n        ux\", not(target_os = \"linux\")), allow(LINT))]\nmod m {}\n",
    ),
    (
        "raw_string_keeps_its_backslash",
        "#[cfg_attr(all(target_os = \"linux\", target_os = r\"lin\\x75x\"), allow(LINT))]\nmod m {}\n",
    ),
    (
        "raw_spelling_of_the_other_family",
        "#[cfg_attr(all(unix, target_family = r\"windows\"), allow(LINT))]\nmod m {}\n",
    ),
    (
        "an_os_of_the_other_family",
        "#[cfg_attr(all(target_os = \"linux\", target_family = \"windows\"), allow(LINT))]\nmod m {}\n",
    ),
    (
        "a_family_flag_beside_the_other_os",
        "#[cfg_attr(all(unix, target_os = \"windows\"), allow(LINT))]\nmod m {}\n",
    ),
    (
        "a_feature_no_valuation_sets",
        "#[cfg(feature = \"x\")]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "an_unknown_atom_negated_twice",
        "#[cfg_attr(not(not(some_flag_nothing_sets)), allow(LINT))]\nmod m {}\n",
    ),
    ("raw_cfg_gate", "#[r#cfg(test)]\n#[allow(LINT)]\nmod m {}\n"),
    (
        "raw_cfg_attr_gate",
        "#[r#cfg_attr(not(test), cfg(test))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "raw_generated_cfg",
        "#[cfg_attr(not(test), r#cfg(test))]\n#[allow(LINT)]\nmod m {}\n",
    ),
];

const SOME_PRODUCTION_BUILD_APPLIES: &[(&str, &str)] = &[
    ("every_build", "#[allow(LINT)]\nmod m {}\n"),
    (
        "not_test",
        "#[cfg_attr(not(test), allow(LINT))]\nmod m {}\n",
    ),
    ("all_empty", "#[cfg(all())]\n#[allow(LINT)]\nmod m {}\n"),
    (
        "every_ci_platform",
        "#[cfg(any(unix, windows))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "nested_not_test",
        "#[cfg_attr(any(unix, windows), cfg_attr(not(test), allow(LINT)))]\nmod m {}\n",
    ),
    (
        "gate_generated_in_test_only",
        "#[cfg_attr(test, cfg(any()))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "inside_a_production_module",
        "#[cfg(not(test))]\nmod m { #![allow(LINT)] }\n",
    ),
    (
        "inside_a_production_function",
        "#[cfg(not(test))]\nfn f() { #[allow(LINT)] let _ = 1; }\n",
    ),
    (
        "second_attribute_on_the_line",
        "#[cfg_attr(test, allow(dead_code))] #[cfg_attr(not(test), allow(LINT))]\nmod m {}\n",
    ),
    (
        "tautology_over_one_value",
        "#[cfg(any(target_os = \"linux\", not(target_os = \"linux\")))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "string_valued_and_every_ci_platform",
        "#[cfg(any(unix, windows, target_os = \"linux\"))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "each_ci_platform_spelled_another_way",
        "#[cfg(any(target_os = r\"linux\", target_os = \"ma\\x63os\", target_os = \"win\\u{6_4}ows\"))]\n#[allow(LINT)]\nmod m {}\n",
    ),
    (
        "each_family_spelled_another_way",
        "#[cfg_attr(any(all(unix, target_family = r#\"unix\"#), all(windows, target_family = \"win\\u{64}ows\")), allow(LINT))]\nmod m {}\n",
    ),
    ("raw_attribute_name", "#[r#allow(LINT)]\nmod m {}\n"),
];

const ONE_CI_PLATFORM_APPLIES: &[(&str, &str, bool)] = &[
    (
        "raw_spelling_on_linux",
        "#[cfg_attr(all(target_os = \"linux\", target_os = r\"linux\"), allow(LINT))]\nmod m {}\n",
        cfg!(target_os = "linux"),
    ),
    (
        "byte_escape_on_linux",
        "#[cfg_attr(all(target_os = \"linux\", target_os = \"lin\\x75x\"), allow(LINT))]\nmod m {}\n",
        cfg!(target_os = "linux"),
    ),
    (
        "unicode_escape_on_macos",
        "#[cfg_attr(all(target_os = \"macos\", target_os = \"ma\\u{63}os\"), allow(LINT))]\nmod m {}\n",
        cfg!(target_os = "macos"),
    ),
    (
        "hashed_raw_spelling_on_windows",
        "#[cfg_attr(all(target_os = \"windows\", target_os = r##\"windows\"##), allow(LINT))]\nmod m {}\n",
        cfg!(target_os = "windows"),
    ),
];

#[test]
fn the_production_fence_rule_reads_the_effective_activation_of_every_allowance() {
    fn named(lint: &str, shape: &str) -> Vec<String> {
        denies_the_production_build_could_forbid(
            &[(
                "src/probe.rs".to_owned(),
                format!("#![deny({lint})]\n{}", shape.replace("LINT", lint)),
            )],
            &|_| false,
        )
    }
    let scratch = scratch_dir("activation");
    let fenced = |lint: &str, shape: &str| {
        format!(
            "#![cfg_attr(not(test), forbid({lint}))]\n{}",
            shape.replace("LINT", lint)
        )
    };
    let mut compiled = 0;
    for lint in USED_GOVERNED_LINTS {
        let bare = normalize_lint(lint).expect("a governed lint");
        let mut builds: Vec<(&str, &str, &str)> = Vec::new();
        let mut refused: Vec<(&str, &str, &str)> = Vec::new();
        for (tag, shape) in NO_PRODUCTION_BUILD_APPLIES {
            let found = named(lint, shape);
            assert!(
                found.len() == 1 && found.iter().all(|line| line.contains(lint)),
                "`{tag}` for `{lint}`: the allowance applies in no production build, so it \
                 cannot excuse the file's `deny`, and the rule has to name it: {found:#?}"
            );
            builds.push((
                tag,
                shape,
                "the fence the rule asks for does not compile in the production build, so \
                 naming the `deny` was wrong",
            ));
        }
        for (tag, shape) in SOME_PRODUCTION_BUILD_APPLIES {
            let found = named(lint, shape);
            assert!(
                found.is_empty(),
                "`{tag}` for `{lint}`: the allowance applies in a production build, so `forbid` \
                 would be E0453 there and the `deny` is excused: {found:#?}"
            );
            refused.push((
                tag,
                shape,
                "clippy did not refuse the production allowance under the fence, so this \
                 control no longer shows why the rule excuses it",
            ));
        }
        for (tag, shape, here) in ONE_CI_PLATFORM_APPLIES {
            let found = named(lint, shape);
            assert!(
                found.is_empty(),
                "`{tag}` for `{lint}`: one CI platform's production build applies the allowance, \
                 so `forbid` would be E0453 there and the `deny` is excused: {found:#?}"
            );
            let why = "the predicate holds on this host exactly when it names this host's \
                       platform, and clippy disagreed";
            if *here {
                refused.push((tag, shape, why));
            } else {
                builds.push((tag, shape, why));
            }
        }
        let in_test: Vec<(&str, &str, &str)> = NO_PRODUCTION_BUILD_APPLIES
            .iter()
            .map(|(tag, shape)| {
                (
                    *tag,
                    *shape,
                    "the fence the rule asks for does not compile in the test build, so naming \
                     the `deny` was wrong",
                )
            })
            .collect();
        for (batch, cases, cfgs, expect_refused) in [
            ("production", &builds, &[][..], false),
            ("test", &in_test, &["test"][..], false),
            ("refused", &refused, &[][..], true),
        ] {
            let sources: Vec<String> = cases
                .iter()
                .map(|(_, shape, _)| fenced(lint, shape))
                .collect();
            let outcomes = clippy_outcomes(&scratch, &format!("{bare}_{batch}"), &sources, cfgs);
            for ((tag, _, why), (built, diagnostics)) in cases.iter().zip(&outcomes) {
                compiled += 1;
                let rejected = diagnostics.iter().any(|(_, code)| code == "E0453");
                assert_eq!(
                    (*built, rejected),
                    (!expect_refused, expect_refused),
                    "`{tag}` for `{lint}` ({batch}): {why}: {diagnostics:?}"
                );
            }
        }
    }
    assert_eq!(
        compiled,
        USED_GOVERNED_LINTS.len()
            * (2 * NO_PRODUCTION_BUILD_APPLIES.len()
                + SOME_PRODUCTION_BUILD_APPLIES.len()
                + ONE_CI_PLATFORM_APPLIES.len()),
        "a fixture was skipped"
    );
    let _ = fs::remove_dir_all(&scratch);
}

const ALLOWANCES_THE_PLACEMENT_CENSUS_DOES_NOT_READ: &[(&str, &str)] = &[
    (
        "macro_written_outer",
        "macro_rules! lower {\n    ($level:ident) => {\n        #[$level(LINT)]\n        mod m {}\n    };\n}\nlower!(allow);\n",
    ),
    (
        "macro_written_inner",
        "macro_rules! lower {\n    ($level:ident) => {\n        mod m {\n            #![$level(LINT)]\n        }\n    };\n}\nlower!(allow);\n",
    ),
];

#[test]
fn an_allowance_the_placement_census_does_not_read_excuses_no_deny() {
    let scratch = scratch_dir("unrecorded");
    for lint in USED_GOVERNED_LINTS {
        let bare = normalize_lint(lint).expect("a governed lint");
        let mut fenced = Vec::new();
        for (tag, shape) in ALLOWANCES_THE_PLACEMENT_CENSUS_DOES_NOT_READ {
            let shape = shape.replace("LINT", lint).replace("BARE", bare);
            let unread = governed_allows(&shape)
                .iter()
                .all(|allow| !allow.lints.iter().any(|named| named == bare));
            assert!(
                unread,
                "`{tag}` for `{lint}`: the placement census reads this allowance, so it is no \
                 witness for the rule below: {shape}"
            );
            let found = denies_the_production_build_could_forbid(
                &[(
                    "src/probe.rs".to_owned(),
                    format!("#![deny({lint})]\n{shape}"),
                )],
                &|_| false,
            );
            assert!(
                found.len() == 1 && found.iter().all(|line| line.contains(lint)),
                "`{tag}` for `{lint}`: an allowance no census records excused a `deny`: {found:#?}"
            );
            fenced.push(format!("#![cfg_attr(not(test), forbid({lint}))]\n{shape}"));
        }
        let outcomes = clippy_outcomes(&scratch, bare, &fenced, &[]);
        for ((tag, _), (built, diagnostics)) in ALLOWANCES_THE_PLACEMENT_CENSUS_DOES_NOT_READ
            .iter()
            .zip(&outcomes)
        {
            assert!(
                !built && diagnostics.iter().any(|(_, code)| code == "E0453"),
                "`{tag}` for `{lint}`: clippy did not apply the allowance, so the refusal above \
                 names nothing real: {diagnostics:?}"
            );
        }
    }
    let _ = fs::remove_dir_all(&scratch);
}

const ALLOWANCES_THE_PLACEMENT_CENSUS_READS_AS_RUSTC_DOES: &[(&str, &str)] = &[
    ("joined", "#[allow(LINT)]\nmod m {}\n"),
    ("spaced_outer", "# [allow(LINT)]\nmod m {}\n"),
    ("commented_outer", "#/* c */[allow(LINT)]\nmod m {}\n"),
    ("mark_outer", "#\u{200E}[allow(LINT)]\nmod m {}\n"),
    ("spaced_keyword", "#[allow (LINT)]\nmod m {}\n"),
    ("keyword_on_its_own_line", "#[allow\n(LINT)]\nmod m {}\n"),
    ("spaced_inner", "mod m {\n    # ![allow(LINT)]\n}\n"),
    ("spaced_bracket_inner", "mod m {\n    #! [allow(LINT)]\n}\n"),
    (
        "marked_inner",
        "mod m {\n    #\u{2028}!\u{200F}[allow\u{0085}(LINT)]\n}\n",
    ),
    ("spaced_path", "#[allow(clippy :: BARE)]\nmod m {}\n"),
    ("raw_lint_name", "#[allow(clippy::r#BARE)]\nmod m {}\n"),
    (
        "raw_lint_name_applied",
        "#[cfg_attr(not(test), allow(clippy::r#BARE))]\nmod m {}\n",
    ),
    ("raw_keyword", "#[r#allow(LINT)]\nmod m {}\n"),
];

#[test]
fn an_allowance_rustc_reads_whatever_separates_or_spells_its_tokens_is_one_the_placement_census_reads()
 {
    let scratch = scratch_dir("recorded");
    for lint in USED_GOVERNED_LINTS {
        let bare = normalize_lint(lint).expect("a governed lint");
        let renamed = [
            ("disallowed_method", "disallowed_methods"),
            ("disallowed_type", "disallowed_types"),
        ]
        .into_iter()
        .filter(|(_, new)| *new == bare)
        .map(|(old, _)| {
            (
                format!("renamed_{old}"),
                format!("#[allow(clippy::{old})]\nmod m {{}}\n"),
            )
        });
        let shapes: Vec<(String, String)> = ALLOWANCES_THE_PLACEMENT_CENSUS_READS_AS_RUSTC_DOES
            .iter()
            .map(|(tag, shape)| {
                (
                    (*tag).to_owned(),
                    shape.replace("LINT", lint).replace("BARE", bare),
                )
            })
            .chain(renamed)
            .collect();
        let mut fenced = Vec::new();
        for (tag, shape) in &shapes {
            let read: Vec<String> = governed_allows(shape)
                .into_iter()
                .flat_map(|allow| allow.lints)
                .collect();
            assert_eq!(
                read,
                [bare],
                "`{tag}` for `{lint}`: rustc applies this allowance of `{lint}` and the placement \
                 census does not read it as one: {shape:?}"
            );
            let found = denies_the_production_build_could_forbid(
                &[(
                    "src/probe.rs".to_owned(),
                    format!("#![deny({lint})]\n{shape}"),
                )],
                &|_| false,
            );
            assert!(
                found.is_empty(),
                "`{tag}` for `{lint}`: the allowance is read, so `forbid` would be E0453 here and \
                 the `deny` is excused, as it is for the joined spelling: {found:#?}"
            );
            fenced.push(format!("#![cfg_attr(not(test), forbid({lint}))]\n{shape}"));
        }
        let outcomes = clippy_outcomes(&scratch, bare, &fenced, &[]);
        assert_eq!(
            outcomes.len(),
            shapes.len(),
            "`{lint}`: a shape was skipped"
        );
        for ((tag, _), (built, diagnostics)) in shapes.iter().zip(&outcomes) {
            assert!(
                !built && diagnostics.iter().any(|(_, code)| code == "E0453"),
                "`{tag}` for `{lint}`: clippy did not apply the allowance, so reading it proves \
                 nothing about what rustc applies: {diagnostics:?}"
            );
        }
    }
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn no_deny_of_a_governed_lint_is_excused_by_test_code_alone() {
    let sources = scanned_sources();
    let denying = sources
        .iter()
        .filter(|(_, source)| {
            USED_GOVERNED_LINTS.iter().any(|lint| {
                crate::effects::lint_levels::file_level_lint_state(source, lint) == Some("deny")
            })
        })
        .count();
    assert!(
        denying > 0,
        "no scanned file denies a governed lint at file level, so this census is measuring \
         nothing"
    );
    let named = denies_the_production_build_could_forbid(&sources, &is_whole_file_test_module);
    assert!(
        named.is_empty(),
        "{} file-level `deny` fence(s) of a governed lint could be `forbid` in the production \
         build: in each, the production region compiles under a `deny` that an inner `allow` the \
         placement scan does not read -- macro-written, or spelled apart -- lowers, the shape \
         #318's first review executed a write through in src/agent/bin.rs and #318 then executed \
         in src/runner/container/census.rs, exec.rs and resolve.rs and under src/engine/mod.rs \
         before fencing all five. Write `#![cfg_attr(not(test), forbid(..))]` for the lint -- the \
         test-only allowance below still compiles, because the lib test target carries no forbid \
         -- or `forbid` where nothing below allows it at all. The pairs:\n{named:#?}",
        named.len()
    );
}

fn unclassified_production_files_leaving_a_governed_lint_unfenced(
    sources: &[(String, String)],
    classified: &[&str],
    declaration_only: &[&str],
    is_test_module: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let by_path: BTreeMap<String, String> = sources.iter().cloned().collect();
    let mut named = Vec::new();
    for (path, source) in sources {
        if !path.starts_with("src/")
            || classified.contains(&path.as_str())
            || declaration_only.contains(&path.as_str())
            || is_test_module(path)
        {
            continue;
        }
        let ancestors = ancestor_module_files(path, &by_path);
        for lint in USED_GOVERNED_LINTS {
            let own = crate::effects::lint_levels::file_level_lint_state(source, lint);
            if matches!(own, Some("forbid" | "deny" | "allow" | "expect")) {
                continue;
            }
            let inherited = ancestors.iter().find_map(|ancestor| {
                by_path
                    .get(ancestor)
                    .and_then(|above| {
                        crate::effects::lint_levels::file_level_lint_state(above, lint)
                    })
                    .map(|level| (ancestor.as_str(), level))
            });
            match inherited {
                Some((_, "forbid")) => {}
                Some((ancestor, level @ ("allow" | "expect"))) => named.push(format!(
                    "{path}: `{lint}` is stated at no level and the production build inherits \
                     `{level}` from {ancestor}: an allowance recorded for that file reaches this \
                     one without a row of its own"
                )),
                Some((ancestor, level)) => named.push(format!(
                    "{path}: `{lint}` is stated at no level and the production build inherits \
                     `{level}` from {ancestor}, which an inner `allow` the placement scan does not \
                     read lowers"
                )),
                None => named.push(format!(
                    "{path}: `{lint}` is stated at no level and no ancestor states it, so the \
                     production build takes it from `-D warnings` alone, which an inner `allow` \
                     the placement scan does not read lowers"
                )),
            }
        }
    }
    named
}

#[test]
fn the_unclassified_fence_rule_names_a_silent_file_and_excuses_one_a_forbid_reaches() {
    fn tree(files: &[(&str, &str)]) -> Vec<(String, String)> {
        files
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
            .collect()
    }
    fn named(files: &[(&str, &str)], classified: &[&str], test_modules: &[&str]) -> Vec<String> {
        unclassified_production_files_leaving_a_governed_lint_unfenced(
            &tree(files),
            classified,
            &["src/lib.rs"],
            &|path| test_modules.contains(&path),
        )
    }
    const SILENT: &str = "fn go() {}\n";
    const FORBID: &str = "#![forbid(clippy::disallowed_methods, clippy::disallowed_types, \
                          clippy::disallowed_macros)]\nfn go() {}\n";
    const DENY: &str = "#![deny(clippy::disallowed_methods, clippy::disallowed_types, \
                        clippy::disallowed_macros)]\nfn go() {}\n";

    for (what, files, classified, test_modules, expected) in [
        (
            "a silent file under a silent, declaration-only root takes every lint from \
             -D warnings alone; the root itself is held code-free by the declaration guard",
            vec![("src/lib.rs", "pub mod a;\n"), ("src/a.rs", SILENT)],
            vec![],
            vec![],
            3,
        ),
        (
            "a silent child of a root that denies inherits a level an allow lowers",
            vec![("src/a.rs", DENY), ("src/a/b.rs", SILENT)],
            vec![],
            vec![],
            3,
        ),
        (
            "a silent child of a root that allows one lint and forbids the rest inherits the \
             allowance, which reaches it without a row of its own",
            vec![
                (
                    "src/a.rs",
                    "#![allow(clippy::disallowed_methods)]\n#![forbid(clippy::disallowed_types, \
                     clippy::disallowed_macros)]\nfn go() {}\n",
                ),
                ("src/a/b.rs", SILENT),
            ],
            vec![],
            vec![],
            1,
        ),
        (
            "`warn` is a statement without being a fence or an allowance",
            vec![(
                "src/a.rs",
                "#![warn(clippy::disallowed_methods)]\n#![forbid(clippy::disallowed_types, \
                 clippy::disallowed_macros)]\nfn go() {}\n",
            )],
            vec![],
            vec![],
            1,
        ),
        (
            "a child of a root that forbids in the production build only is reached by that \
             forbid; a lint the root leaves unstated is named in the root and in the child",
            vec![
                (
                    "src/a.rs",
                    "#![cfg_attr(not(test), forbid(clippy::disallowed_methods, \
                     clippy::disallowed_types))]\nfn go() {}\n",
                ),
                ("src/a/b.rs", SILENT),
            ],
            vec![],
            vec![],
            2,
        ),
    ] {
        let found = named(&files, &classified, &test_modules);
        assert_eq!(found.len(), expected, "{what}: {found:#?}");
    }

    let root_not_held = unclassified_production_files_leaving_a_governed_lint_unfenced(
        &tree(&[("src/lib.rs", "pub mod a;\n"), ("src/a.rs", SILENT)]),
        &[],
        &[],
        &|_| false,
    );
    assert_eq!(
        root_not_held.len(),
        6,
        "a silent root nobody holds to declarations is named for every lint too: \
         {root_not_held:#?}"
    );

    for (what, files, classified, test_modules) in [
        (
            "the file states every lint itself",
            vec![("src/a.rs", FORBID)],
            vec![],
            vec![],
        ),
        (
            "a `deny` is a statement here; whether it could be `forbid` is the fence sweep's \
             question, and the two roots that state one hold only declarations",
            vec![("src/a.rs", DENY)],
            vec![],
            vec![],
        ),
        (
            "a silent child under a forbidding root",
            vec![("src/a.rs", FORBID), ("src/a/b.rs", SILENT)],
            vec![],
            vec![],
        ),
        (
            "a silent grandchild under a forbidding `mod.rs` root",
            vec![("src/a/mod.rs", FORBID), ("src/a/b/c.rs", SILENT)],
            vec![],
            vec![],
        ),
        (
            "a silent module under a forbidding crate root",
            vec![("src/lib.rs", FORBID), ("src/a.rs", SILENT)],
            vec![],
            vec![],
        ),
        (
            "a classified module is the roll-call guard's, not this rule's",
            vec![("src/a.rs", SILENT)],
            vec!["src/a.rs"],
            vec![],
        ),
        (
            "a whole-file test module has no production region",
            vec![("src/a/tests.rs", SILENT)],
            vec![],
            vec!["src/a/tests.rs"],
        ),
        (
            "an example is its own crate root and reaches nothing in the library",
            vec![("examples/probe.rs", SILENT)],
            vec![],
            vec![],
        ),
    ] {
        let found = named(&files, &classified, &test_modules);
        assert!(found.is_empty(), "{what}: {found:#?}");
    }
}

#[test]
fn an_undecided_prologue_is_no_fence_to_the_censuses_that_read_one() {
    const LINT: &str = "clippy::disallowed_methods";
    for (what, prologue) in [
        (
            "a list entry rustc refuses",
            "#![forbid(clippy::disallowed_methods)]\n#![allow(footool::disallowed_types)]\n",
        ),
        (
            "a predicate the grammar refuses",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(not(te st), allow(clippy::disallowed_methods))]\n",
        ),
        (
            "`warnings` lowering a `warn`",
            "#![warn(clippy::disallowed_methods)]\n#![allow(warnings)]\n",
        ),
        (
            "a platform the production valuations disagree on",
            "#![cfg_attr(unix, forbid(clippy::disallowed_methods))]\n",
        ),
    ] {
        let source = format!("{prologue}fn go() {{}}\n");
        let resolution = crate::effects::lint_levels::file_level_lint_resolution(&source, LINT);
        assert!(
            resolution.undecided && resolution.level.is_none(),
            "{what}: {resolution:?}"
        );
        assert!(
            !file_level_denies(&source, LINT),
            "{what}: the per-site expectation rule read an undecided prologue as a fence"
        );
        let own = unclassified_production_files_leaving_a_governed_lint_unfenced(
            &[("src/a.rs".to_owned(), source.clone())],
            &[],
            &[],
            &|_| false,
        );
        assert!(
            own.iter()
                .any(|line| line.starts_with(&format!("src/a.rs: `{LINT}`"))),
            "{what}: the roll-call guard read an undecided prologue as a fence: {own:#?}"
        );
        let inherited = unclassified_production_files_leaving_a_governed_lint_unfenced(
            &[
                ("src/a.rs".to_owned(), source.clone()),
                ("src/a/b.rs".to_owned(), "fn go() {}\n".to_owned()),
            ],
            &[],
            &["src/a.rs"],
            &|_| false,
        );
        assert!(
            inherited
                .iter()
                .any(|line| line.starts_with(&format!("src/a/b.rs: `{LINT}`"))),
            "{what}: a child took an undecided ancestor as the forbid it inherits: {inherited:#?}"
        );
    }
}

#[test]
fn every_unclassified_production_file_states_each_governed_lint_or_inherits_its_forbid() {
    let sources = scanned_sources();
    let stating = sources
        .iter()
        .filter(|(path, source)| {
            path.starts_with("src/")
                && !super::CLASSIFIED_MODULES.contains(&path.as_str())
                && USED_GOVERNED_LINTS.iter().all(|lint| {
                    crate::effects::lint_levels::file_level_lint_state(source, lint).is_some()
                })
        })
        .count();
    assert!(
        stating > 10,
        "only {stating} unclassified files state every governed lint at file level, so this \
         census is measuring nothing"
    );
    let named = unclassified_production_files_leaving_a_governed_lint_unfenced(
        &sources,
        super::CLASSIFIED_MODULES,
        DECLARATION_ONLY_MODULES,
        &is_whole_file_test_module,
    );
    assert!(
        named.is_empty(),
        "{} governed-lint pair(s) in production files outside `CLASSIFIED_MODULES` are fenced \
         by nothing: the file states no level for the lint and inherits no `forbid` from an \
         ancestor, so the shape Gate 5's fourth run executed in src/capacity.rs, and #318's \
         second MAIN review executed in src/plan/mod.rs, applies unchanged \
         (`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL`). Write \
         `#![forbid(clippy::disallowed_methods, clippy::disallowed_types, \
         clippy::disallowed_macros)]` in the file's prologue, `cfg_attr(not(test), forbid(..))` \
         for a lint only whole-file test children allow, or the fence at the root the file \
         descends from; a classified module is judged by the roll-call guard instead, and a \
         module `a_declaring_module_holds_declarations_and_re_exports_and_nothing_else` holds \
         code-free hosts nothing. The pairs:\n{named:#?}",
        named.len()
    );
}

const UNSTATED_GOVERNED_LINT_PAIRS_IN_CLASSIFIED_MODULES: usize = 29;

struct UnstatedLint {
    path: String,
    lint: &'static str,
    level: Option<&'static str>,
    inherited: Option<(String, &'static str)>,
}

impl UnstatedLint {
    fn describe(&self) -> String {
        let stated = match self.level {
            Some(level) => format!("{}: `{}` is `{level}` at file level", self.path, self.lint),
            None => format!("{}: `{}` is stated at no level", self.path, self.lint),
        };
        match &self.inherited {
            Some((ancestor, level)) => {
                format!("{stated}; the production build inherits `{level}` from {ancestor}")
            }
            None => format!("{stated}; the production build takes it from `-D warnings` alone"),
        }
    }
}

fn ancestor_module_files(path: &str, sources: &BTreeMap<String, String>) -> Vec<String> {
    let Some(module) = path
        .strip_prefix("src/")
        .and_then(|under| under.strip_suffix(".rs"))
    else {
        return Vec::new();
    };
    let mut segments: Vec<&str> = module.split('/').collect();
    if segments.last() == Some(&"mod") {
        segments.pop();
    }
    let mut ancestors = Vec::new();
    for depth in (0..segments.len()).rev() {
        let candidates = if depth == 0 {
            vec!["src/lib.rs".to_owned(), "src/main.rs".to_owned()]
        } else {
            let prefix = segments.get(..depth).unwrap_or_default().join("/");
            vec![format!("src/{prefix}.rs"), format!("src/{prefix}/mod.rs")]
        };
        ancestors.extend(
            candidates
                .into_iter()
                .find(|candidate| sources.contains_key(candidate)),
        );
    }
    ancestors
}

fn governed_lints_no_classified_module_states_at_file_level() -> Vec<UnstatedLint> {
    let sources: BTreeMap<String, String> = scanned_sources().into_iter().collect();
    let mut unstated = Vec::new();
    for path in super::CLASSIFIED_MODULES {
        let Some(source) = sources.get(*path) else {
            panic!(
                "{path} is in `CLASSIFIED_MODULES` and the scan of src/ and examples/ did not \
                 read it"
            );
        };
        let ancestors = ancestor_module_files(path, &sources);
        for lint in USED_GOVERNED_LINTS {
            let level = crate::effects::lint_levels::file_level_lint_state(source, lint);
            if matches!(level, Some("forbid" | "deny" | "allow" | "expect")) {
                continue;
            }
            let inherited = ancestors.iter().find_map(|ancestor| {
                sources
                    .get(ancestor)
                    .and_then(|above| {
                        crate::effects::lint_levels::file_level_lint_state(above, lint)
                    })
                    .map(|level| (ancestor.clone(), level))
            });
            unstated.push(UnstatedLint {
                path: (*path).to_owned(),
                lint,
                level,
                inherited,
            });
        }
    }
    unstated
}

#[test]
fn every_classified_module_carries_a_file_level_fence_or_allowance_of_a_governed_lint() {
    let unstated = governed_lints_no_classified_module_states_at_file_level();
    let mut per_file: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in &unstated {
        *per_file.entry(entry.path.as_str()).or_default() += 1;
    }
    assert!(
        per_file.len() < super::CLASSIFIED_MODULES.len(),
        "no classified module states a governed lint at file level, so this census is \
         measuring nothing"
    );
    let neither: Vec<&str> = per_file
        .iter()
        .filter(|(_, count)| **count == USED_GOVERNED_LINTS.len())
        .map(|(path, _)| *path)
        .collect();
    assert!(
        neither.is_empty(),
        "each of these classified modules carries neither a file-level fence nor a file-level \
         allowance of any governed lint, so every governed lint takes its level in it from \
         `-D warnings` alone, which an inner `allow` the placement scan does not read -- \
         macro-written, or spelled apart -- lowers: the shape Gate 5's fourth run executed in \
         src/capacity.rs and src/runner/invocation.rs \
         (`G5RUN4-RESIDUAL-BYPASS-OUTSIDE-THE-Q5-CARVE-OUT`). Write \
         `#![forbid(clippy::disallowed_methods, clippy::disallowed_types, \
         clippy::disallowed_macros)]` in its prologue, `deny` for a lint an allowance below \
         it needs (`every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile` says \
         which), or the module-level allow {ALLOWLIST_TOML} records:\n{neither:#?}"
    );
}

#[test]
fn the_governed_lint_pairs_classified_modules_leave_unstated_only_shrink() {
    let unstated = governed_lints_no_classified_module_states_at_file_level();
    let listed: Vec<String> = unstated.iter().map(UnstatedLint::describe).collect();
    let pinned = UNSTATED_GOVERNED_LINT_PAIRS_IN_CLASSIFIED_MODULES;
    let direction = if unstated.len() > pinned {
        "a classified module leaves a governed lint's level unstated that was stated before, \
         or arrived leaving one unstated, or the pin was lowered below what the tree states; \
         fence the pair -- `forbid`, or `deny` where an allowance below it makes `forbid` \
         E0453 -- record its allowance, or restore the pin"
    } else {
        "a pair was fenced or recorded; lower `UNSTATED_GOVERNED_LINT_PAIRS_IN_CLASSIFIED_MODULES` \
         to the count found, so the residue only ever shrinks"
    };
    assert_eq!(
        unstated.len(),
        pinned,
        "{} governed-lint pairs in classified modules are stated at no file level against \
         {pinned} pinned: {direction}. The pin counts what each prologue states, not which \
         pairs are lowerable: a pair's level in the production build is an ancestor's `forbid`, \
         which no inner `allow` lowers, an ancestor's `deny`, which one the placement scan does \
         not read lowers, or `-D warnings` alone, which one lowers too; each pair below says \
         which. The pairs:\n{listed:#?}",
        unstated.len()
    );
}

#[test]
fn the_placement_scan_refuses_an_allow_that_is_not_module_level_and_sees_through_no_disguise() {
    let on_a_function = "#[allow(clippy::disallowed_methods)]\nfn go() {}\n";
    let found = governed_allows(on_a_function);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(!found[0].module_level);

    let on_a_statement = "fn go() {\n    #[allow(clippy::disallowed_methods)]\n    let _ = 1;\n}\n";
    let found = governed_allows(on_a_statement);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(!found[0].module_level);

    let on_a_module = "#[allow(clippy::disallowed_methods)]\nmod inner { }\n";
    let found = governed_allows(on_a_module);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].module_level);

    let inner = "//! doc\n#![allow(clippy::disallowed_types)]\nfn go() {}\n";
    let found = governed_allows(inner);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].inner && found[0].module_level);

    let late = "fn go() {}\n#![allow(clippy::disallowed_types)]\n";
    let found = governed_allows(late);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(!found[0].module_level);

    let expected = "#![expect(clippy::disallowed_macros)]\n";
    assert_eq!(governed_allows(expected).len(), 1);

    assert!(governed_allows("#![allow(clippy::too_many_arguments)]\n").is_empty());
    assert!(governed_allows("#![allow(unused_variables)]\n").is_empty());

    let disguised = concat!(
        "//! ```\n",
        "//! #![allow(clippy::disallowed_methods)]\n",
        "//! ```\n",
        "// #![allow(clippy::disallowed_types)]\n",
        "/* #![allow(clippy::disallowed_macros)] */\n",
        "const FIXTURE: &str = \"#![allow(clippy::disallowed_methods)]\";\n",
        "const RAW: &str = r#\"#![allow(clippy::disallowed_types)]\"#;\n",
    );
    assert!(
        governed_allows(disguised).is_empty(),
        "{:#?}",
        governed_allows(disguised)
    );
    let blanked = blank_comments_and_strings(disguised);
    assert_eq!(blanked.len(), disguised.len(), "offsets are preserved");
    assert_ne!(blanked, disguised, "the blanking is a no-op");
    assert!(!blanked.contains("disallowed_methods"));

    let mixed = format!("{disguised}#![allow(clippy::disallowed_macros)]\n");
    let found = governed_allows(&mixed);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].lints, vec!["disallowed_macros".to_owned()]);

    let mechanisms = 9;
    assert_eq!(mechanisms, 9);
}

#[test]
fn the_three_blunt_governed_lints_are_used_by_nobody() {
    let mut blunt = Vec::new();
    for (path, source) in scanned_sources() {
        for allow in governed_allows(&source) {
            for lint in &allow.lints {
                if matches!(lint.as_str(), "style" | "all" | "warnings") {
                    blunt.push(format!("{path}:{} {lint}", allow.line));
                }
            }
        }
    }
    assert!(blunt.is_empty(), "{blunt:#?}");

    for probe in [
        "#![allow(warnings)]\n",
        "#![allow(clippy::all)]\n",
        "#![allow(clippy::style)]\n",
    ] {
        assert_eq!(governed_allows(probe).len(), 1, "{probe}");
    }

    let list = allowlist();
    let used: BTreeSet<&str> = list
        .funnel
        .iter()
        .chain(&list.legacy)
        .flat_map(|entry| entry.allows.iter().map(String::as_str))
        .collect();
    let expected: BTreeSet<&str> = USED_GOVERNED_LINTS.iter().copied().collect();
    assert_eq!(used, expected);
}

#[test]
fn cargo_toml_declares_no_lint_table_that_could_allow_a_governed_lint() {
    let text = fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml");
    let manifest: toml::Value = toml::from_str(&text).expect("Cargo.toml parses");
    let Some(lints) = manifest.get("lints") else {
        return;
    };
    let rendered = lints.to_string();
    for lint in super::GOVERNED_LINTS {
        assert!(
            !rendered.contains(lint),
            "Cargo.toml [lints] names the governed lint `{lint}`: {rendered}"
        );
    }
}

fn forbids_non_local_definitions(source: &str) -> bool {
    crate::effects::lint_levels::file_level_lint_state(source, "non_local_definitions")
        == Some("forbid")
}

#[test]
fn every_crate_root_forbids_non_local_definitions() {
    let roots = crate_roots();
    let mut read = Vec::new();
    let mut silent = Vec::new();
    for root in roots.roots() {
        let source = fs::read_to_string(root).expect("a crate root");
        if !forbids_non_local_definitions(&source) {
            silent.push(root.display().to_string());
        }
        read.push(root.display().to_string());
    }
    assert!(
        silent.is_empty(),
        "a crate root that does not forbid `non_local_definitions` at file level lets a macro \
         invoked inside a function define a method reachable from anywhere: {silent:#?}"
    );
    for named in ["src/lib.rs", "src/main.rs", "examples/probe.rs"] {
        assert!(
            roots.is_root_relative(named),
            "`{named}` is no longer a target root of this package, so the census above did not \
             read it: {read:#?}"
        );
    }

    for (source, forbidden) in [
        ("#![forbid(non_local_definitions)]\n", true),
        (
            "//! docs\n#![forbid(non_local_definitions)]\n// a note\n#![allow(dead_code)]\npub mod a;\n",
            true,
        ),
        (
            "#![cfg_attr(not(test), forbid(non_local_definitions))]\n",
            true,
        ),
        ("#![forbid(dead_code, non_local_definitions)]\n", true),
        ("", false),
        ("pub mod a;\n", false),
        ("#![deny(non_local_definitions)]\n", false),
        ("#![warn(non_local_definitions)]\n", false),
        ("#![cfg_attr(test, forbid(non_local_definitions))]\n", false),
        ("#![forbid(dead_code)]\n", false),
        ("// #![forbid(non_local_definitions)]\n", false),
        ("pub mod a;\n#![forbid(non_local_definitions)]\n", false),
        ("#[forbid(non_local_definitions)]\npub mod a;\n", false),
        (
            "mod inner {\n    #![forbid(non_local_definitions)]\n}\n",
            false,
        ),
    ] {
        assert_eq!(
            forbids_non_local_definitions(source),
            forbidden,
            "{source:?}"
        );
    }
}

fn production_files_a_governed_lint_is_not_forbidden_in(
    sources: &[(String, String)],
) -> BTreeSet<String> {
    let by_path: BTreeMap<String, String> = sources.iter().cloned().collect();
    let mut domain = BTreeSet::new();
    for (path, source) in sources {
        if is_whole_file_test_module(path) {
            continue;
        }
        let ancestors = ancestor_module_files(path, &by_path);
        let lowerable = USED_GOVERNED_LINTS.iter().any(|lint| {
            let own = crate::effects::lint_levels::file_level_lint_resolution(source, lint);
            if own.undecided {
                return true;
            }
            let effective = own.level.or_else(|| {
                ancestors.iter().find_map(|ancestor| {
                    by_path.get(ancestor).and_then(|above| {
                        crate::effects::lint_levels::file_level_lint_state(above, lint)
                    })
                })
            });
            effective != Some("forbid")
        });
        if lowerable {
            domain.insert(path.clone());
        }
    }
    domain
}

#[test]
fn every_macro_invocation_where_a_governed_lint_is_not_forbidden_is_inside_a_function_body() {
    let sources = scanned_sources();
    let domain = production_files_a_governed_lint_is_not_forbidden_in(&sources);
    let mut outside = Vec::new();
    for (path, source) in &sources {
        if !domain.contains(path) {
            continue;
        }
        for invocation in super::census_domain::macro_invocations_outside_function_bodies(source) {
            outside.push(format!("{path}:{} `{}!`", invocation.line, invocation.name));
        }
    }
    assert!(
        outside.is_empty(),
        "a macro invoked outside a function body can write an item no census reads -- a `fn` \
         under a name the source does not spell, a module, an allow -- and one in a module-level \
         `const _` defines a method `non_local_definitions` does not lint; in a file that does \
         not forbid every governed lint that item can hold an effect nothing classifies. Move \
         the invocation into a module that forbids all three:\n{outside:#?}"
    );

    for named in [
        "src/rundir.rs",
        "src/runner/host.rs",
        "src/util.rs",
        "src/workspace.rs",
        "src/runner/container.rs",
        "src/main.rs",
        "examples/probe.rs",
    ] {
        assert!(
            domain.contains(named),
            "`{named}` allows a governed lint and is not in the domain: {domain:#?}"
        );
    }
    for forbidding in ["src/util/terminal.rs", "src/runner/host/naming.rs"] {
        assert!(
            !domain.contains(forbidding),
            "`{forbidding}` forbids all three governed lints and was read as lowerable"
        );
    }
    let list = allowlist();
    for entry in list.funnel.iter().chain(&list.legacy) {
        if entry.allows.is_empty() || is_whole_file_test_module(&entry.path) {
            continue;
        }
        let source =
            fs::read_to_string(repo_root().join(&entry.path)).expect("an allowlisted file");
        let allowed_in_production = USED_GOVERNED_LINTS.iter().any(|lint| {
            crate::effects::lint_levels::file_level_lint_state(&source, lint) != Some("forbid")
        });
        assert!(
            !allowed_in_production || domain.contains(&entry.path),
            "`{}` carries an allowance the production build applies and is not in the domain",
            entry.path
        );
    }
}

#[test]
fn the_macro_position_reader_refuses_every_position_outside_a_function_body() {
    fn outside(source: &str) -> Vec<String> {
        super::census_domain::macro_invocations_outside_function_bodies(source)
            .into_iter()
            .map(|invocation| invocation.name)
            .collect()
    }

    for (position, source) in [
        (
            "module item position",
            "thread_local! { static X: u8 = 0; }\n",
        ),
        (
            "a macro's path",
            "std::thread_local! { static X: u8 = 0; }\n",
        ),
        ("spaced", "m ! ( );\n"),
        ("a comment between the tokens", "m /* c */ ! /* d */ { }\n"),
        ("a raw name", "r#m!();\n"),
        ("a raw keyword name", "r#match!();\n"),
        ("a non-ASCII name", "\u{e9}!();\n"),
        ("a definition", "macro_rules! m { () => {}; }\n"),
        ("a raw definition name", "macro_rules! r#m { () => {}; }\n"),
        (
            "an aliased include",
            "use std::include as rd;\nrd!(\"x.inc\");\n",
        ),
        ("an inline module", "mod inner {\n    m!();\n}\n"),
        ("an impl block", "impl X {\n    m!();\n}\n"),
        ("a trait block", "trait T {\n    m!();\n}\n"),
        ("an extern block", "extern \"C\" {\n    m!();\n}\n"),
        (
            "a module-level `const _`",
            "const _: () = {\n    m!();\n};\n",
        ),
        (
            "a nested `const _`",
            "const _: () = {\n    const _: () = {\n        m!();\n    };\n};\n",
        ),
        ("a named `const`", "const C: u8 = m!();\n"),
        ("a `static`", "static S: u8 = m!();\n"),
        (
            "a closure in a `static`",
            "static F: fn() = || {\n    m!();\n};\n",
        ),
        ("an enum discriminant", "enum E {\n    A = m!(),\n}\n"),
        ("a field's type", "struct S {\n    f: m!(),\n}\n"),
        ("a return type", "fn f() -> m!() {\n    0\n}\n"),
        ("a parameter's type", "fn f(x: [u8; m!()]) {}\n"),
        (
            "a const-generic default",
            "fn f<const N: usize = { m!() }>() {}\n",
        ),
        (
            "an attribute's value",
            "#[doc = concat!(\"a\")]\nfn f() {}\n",
        ),
        (
            "production code beside a test item",
            "#[cfg(test)]\nfn t() {}\nm!();\n",
        ),
        (
            "an impl for a type named `r#fn`",
            "impl T for r#fn where u8: Copy {\n    m!();\n}\n",
        ),
        (
            "an associated const after a bodiless declaration",
            "trait T {\n    fn a(&self);\n    const C: () = {\n        m!()\n    };\n}\n",
        ),
        (
            "an impl after a `fn` in a macro's arguments",
            "fn f() {\n    n!(fn x);\n}\nimpl X {\n    m!();\n}\n",
        ),
        (
            "an impl after a `fn` in a macro's brackets",
            "fn f() {\n    n![fn x];\n}\nimpl X {\n    m!();\n}\n",
        ),
        (
            "an impl after a `fn` in a macro's braces",
            "fn f() {\n    n! { fn x }\n}\nimpl X {\n    m!();\n}\n",
        ),
    ] {
        assert_eq!(outside(source).len(), 1, "{position}: {source:?}");
    }

    for (position, source) in [
        (
            "a block after a `fn` in a macro's parentheses",
            "const C: u8 = if n!(fn x) {\n    m!()\n} else {\n    0\n};\n",
        ),
        (
            "a block after a `fn` in a macro's brackets",
            "const C: u8 = if n![fn x] {\n    m!()\n} else {\n    0\n};\n",
        ),
    ] {
        assert_eq!(
            outside(source),
            vec!["n".to_owned(), "m".to_owned()],
            "{position}: {source:?}"
        );
    }

    for (position, source) in [
        ("a function body", "fn f() {\n    m!();\n}\n"),
        (
            "a method body",
            "impl X {\n    pub fn f(&self) {\n        m!();\n    }\n}\n",
        ),
        (
            "a trait's default body",
            "trait T {\n    fn d(&self) {\n        m!();\n    }\n}\n",
        ),
        (
            "a function inside a `const _`",
            "const _: () = {\n    fn g() {\n        m!();\n    }\n};\n",
        ),
        (
            "a header holding a const block",
            "fn f() -> Foo<{ 1 }> {\n    m!()\n}\n",
        ),
        (
            "a header holding an array",
            "fn f(x: [u8; 3]) -> [u8; 3] {\n    m!(x)\n}\n",
        ),
        (
            "a where clause",
            "fn f<T>() where T: Tr<{ 2 }>, for<'a> &'a T: Fn() -> u8 {\n    m!()\n}\n",
        ),
        (
            "an arrow in the return type",
            "fn f() -> impl Fn() -> u8 {\n    || m!()\n}\n",
        ),
        (
            "qualifiers",
            "pub(crate) const unsafe extern \"C\" fn f() {\n    m!();\n}\n",
        ),
        ("a raw function name", "fn r#match() {\n    m!();\n}\n"),
        ("a non-ASCII function name", "fn \u{e9}() {\n    m!();\n}\n"),
        (
            "a nested function",
            "fn f() {\n    fn g() {\n        m!();\n    }\n    n!();\n}\n",
        ),
        (
            "a test-only item",
            "#[cfg(test)]\nthread_local! { static X: u8 = 0; }\n",
        ),
        (
            "a test-only module",
            "#[cfg(test)]\nmod tests {\n    m!();\n}\n",
        ),
        (
            "no macro",
            "const B: bool = !A;\nfn f(a: bool, b: u8) -> bool {\n    if !a { return !(b != 1); }\n    a != (b == 2)\n}\n",
        ),
        (
            "a keyword before a `!` outside a body",
            "const X: bool = if !A { true } else { !B };\n",
        ),
        ("a macro in a comment", "// m!();\n/* n!{} */\nfn f() {}\n"),
        ("a macro in a string", "const S: &str = \"m!()\";\n"),
    ] {
        assert!(
            outside(source).is_empty(),
            "{position}: {source:?} -> {:?}",
            outside(source)
        );
    }
}

#[test]
fn the_macro_census_domain_is_every_production_file_a_governed_lint_can_be_lowered_in() {
    const ALL_THREE: &str = "#![forbid(\n    clippy::disallowed_methods,\n    clippy::disallowed_types,\n    clippy::disallowed_macros\n)]\n";
    let tree: Vec<(String, String)> = [
        ("src/forbidding.rs", ALL_THREE.to_owned()),
        (
            "src/forbidding/silent_child.rs",
            "pub fn f() {}\n".to_owned(),
        ),
        (
            "src/allowing.rs",
            "#![allow(clippy::disallowed_methods)]\n#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]\n".to_owned(),
        ),
        (
            "src/allowing/silent_child.rs",
            "#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]\n".to_owned(),
        ),
        (
            "src/denying.rs",
            "#![deny(clippy::disallowed_methods)]\n#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]\n".to_owned(),
        ),
        ("src/silent.rs", "pub fn f() {}\n".to_owned()),
        (
            "src/test_only_forbid.rs",
            "#![cfg_attr(test, forbid(\n    clippy::disallowed_methods,\n    clippy::disallowed_types,\n    clippy::disallowed_macros\n))]\n".to_owned(),
        ),
        (
            "src/forbidding/undecided_child.rs",
            "#![cfg_attr(unix, forbid(clippy::disallowed_methods))]\n#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]\n".to_owned(),
        ),
        (
            "src/rundir/tests.rs",
            "#![allow(clippy::disallowed_methods)]\n".to_owned(),
        ),
        ("examples/root.rs", "#![allow(clippy::disallowed_macros)]\n".to_owned()),
    ]
    .into_iter()
    .map(|(path, source)| (path.to_owned(), source))
    .collect();
    let domain = production_files_a_governed_lint_is_not_forbidden_in(&tree);
    let expected: BTreeSet<String> = [
        "src/allowing.rs",
        "src/allowing/silent_child.rs",
        "src/denying.rs",
        "src/silent.rs",
        "src/test_only_forbid.rs",
        "src/forbidding/undecided_child.rs",
        "examples/root.rs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(domain, expected);
}

#[test]
fn the_legacy_section_is_frozen_and_may_only_shrink() {
    let list = allowlist();
    let current: Vec<&str> = list.legacy.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        legacy_growth(FROZEN_LEGACY_ALLOWLIST, &current),
        Vec::<&str>::new(),
        "the legacy section grew past the frozen list"
    );
    assert_eq!(
        current.len(),
        FROZEN_LEGACY_ALLOWLIST.len(),
        "PR5 freezes the list at exactly what it ships"
    );

    let grown: Vec<&str> = current.iter().copied().chain(["src/catalog.rs"]).collect();
    assert_eq!(
        legacy_growth(FROZEN_LEGACY_ALLOWLIST, &grown),
        vec!["src/catalog.rs"]
    );
    let shrunk: Vec<&str> = current.iter().copied().skip(1).collect();
    assert!(legacy_growth(FROZEN_LEGACY_ALLOWLIST, &shrunk).is_empty());

    let frozen: BTreeSet<&str> = FROZEN_LEGACY_ALLOWLIST.iter().copied().collect();
    let listed: BTreeSet<&str> = current.iter().copied().collect();
    assert_eq!(frozen, listed);
}

#[test]
fn the_legacy_section_never_contains_a_topology_module() {
    let list = allowlist();
    let current: Vec<&str> = list.legacy.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        topology_modules_among(&current),
        Vec::<&str>::new(),
        "a topology module is in the frozen legacy section"
    );

    let probes = [
        "src/topology/registry.rs",
        "src/runner/mod.rs",
        "src/workspace_manager.rs",
        "src/workspace_manager/residue.rs",
        "src/engine/topology.rs",
        "src/engine/topology/create.rs",
    ];
    for probe in probes {
        assert_eq!(
            topology_modules_among(&[probe]),
            vec![probe],
            "`{probe}` is a topology module and the check missed it"
        );
    }
    assert_eq!(
        probes.len(),
        TOPOLOGY_MODULES.len(),
        "one probe per banned shape"
    );

    let sentence_shapes = [
        "src/topology/",
        "src/runner/",
        "src/workspace_manager.rs",
        "src/engine/topology.rs",
    ];
    let submodule = "src/engine/topology/create.rs";
    assert!(
        !submodule.starts_with("src/engine/topology.rs"),
        "the prefix relation this entry exists for no longer holds"
    );
    assert!(
        !sentence_shapes
            .iter()
            .any(|banned| submodule.starts_with(banned) || *banned == submodule),
        "the four shapes the packet sentence names already cover `{submodule}`, \
         so the fifth entry is dead weight and should be removed"
    );

    let before_the_split = [
        "src/topology/",
        "src/runner/",
        "src/workspace_manager.rs",
        "src/engine/topology.rs",
        "src/engine/topology/",
    ];
    let child = "src/workspace_manager/residue.rs";
    assert!(
        !child.starts_with("src/workspace_manager.rs"),
        "the prefix relation this entry exists for no longer holds"
    );
    assert!(
        !before_the_split
            .iter()
            .any(|banned| child.starts_with(banned) || *banned == child),
        "the shapes that predate the `m4-workspace` split already cover \
         `{child}`, so the `src/workspace_manager/` entry is dead weight and \
         should be removed"
    );

    let funnel: BTreeSet<&str> = list.funnel.iter().map(|e| e.path.as_str()).collect();
    for expected in [
        "src/workspace_manager.rs",
        "src/runner/host.rs",
        "src/runner/invocation.rs",
        "src/topology/effects.rs",
    ] {
        assert!(
            funnel.contains(expected),
            "{expected} left the funnel section"
        );
    }
}

#[test]
fn every_allowlist_entry_carries_its_justification_and_names_a_real_file() {
    let list = allowlist();
    let mut absent = Vec::new();
    for entry in &list.funnel {
        assert!(
            !entry.review.trim().is_empty(),
            "{} has no funnel review clause",
            entry.path
        );
        assert!(!entry.packet.trim().is_empty(), "{}", entry.path);
    }
    for entry in &list.legacy {
        assert!(
            entry.legacy_effect.contains("LEGACY-EFFECT"),
            "{} carries no LEGACY-EFFECT justification",
            entry.path
        );
        assert!(
            !entry.shrinks_when.trim().is_empty(),
            "{} does not say when it shrinks",
            entry.path
        );
    }
    for entry in list.funnel.iter().chain(&list.legacy) {
        let exists = repo_root().join(&entry.path).is_file();
        assert_eq!(
            exists, !entry.absent,
            "{} is marked absent={} and exists={exists}",
            entry.path, entry.absent
        );
        if entry.absent {
            absent.push(entry.path.as_str());
            assert!(
                entry.allows.is_empty(),
                "{} is absent and cannot carry an attribute",
                entry.path
            );
        }
    }
    assert_eq!(absent, Vec::<&str>::new(), "the absent set moved");
    assert!(
        repo_root().join("src/runner/container.rs").is_file(),
        "the Container funnel is the entry that used to be absent; if it is gone \
         again, this assertion is the one that says so rather than an empty set \
         reading as agreement"
    );
}

#[test]
fn the_denylist_names_every_primitive_the_packet_enumerates() {
    let denied = denylist();
    let methods: BTreeSet<&str> = denied
        .disallowed_methods
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    let types: BTreeSet<&str> = denied
        .disallowed_types
        .iter()
        .map(|e| e.path.as_str())
        .collect();

    let missing: Vec<&str> = PACKET_PRIMITIVES
        .iter()
        .copied()
        .filter(|path| !methods.contains(path))
        .collect();
    assert!(missing.is_empty(), "disallowed-methods omits {missing:?}");

    let missing: Vec<&str> = PACKET_TYPES
        .iter()
        .copied()
        .filter(|path| !types.contains(path))
        .collect();
    assert!(missing.is_empty(), "disallowed-types omits {missing:?}");

    assert!(!denied.disallowed_methods.is_empty());
    assert!(!denied.disallowed_types.is_empty());
    assert!(
        !denied.disallowed_macros.is_empty(),
        "the macro list is the one that can be vacuous without looking it"
    );

    for entry in denied.all() {
        assert!(
            entry.reason.starts_with("UPSTROKE-EFFECT")
                || entry.reason.starts_with("UPSTROKE-WRAPPER"),
            "{} has no classified reason: {}",
            entry.path,
            entry.reason
        );
    }

    const NAMES_A_CONTAINER_RUNTIME: &[(&str, &str)] = &[
        (
            "src/effects/tests.rs",
            "this census's own needle table, which is the one place the strings \
             have to be written down",
        ),
        (
            "src/agent/proc/tests.rs",
            "the Process funnel's `#[cfg(test)]` suite, out of line since M6. \
             The reaper-reclaim tests name the runtime the cleanup reaper is \
             armed with -- the same text was inside `src/agent/proc.rs` below \
             its `#[cfg(test)]` cut and so was never in this domain; it is \
             named for the same reason `fake.rs` is, the marker being at the \
             DECLARATION and not in the file",
        ),
        (
            "src/runner/container.rs",
            "the Container funnel: `FunnelGroup::Container.module()`, the one \
             production file that may reach a container runtime, and the one \
             `Command::new(` row in `every_production_process_start_is_classified`",
        ),
        (
            "src/runner/container/exec/tests.rs",
            "the `ContainerRunner`'s `#[cfg(test)]` suite, out of line since W1. \
             The same text was inside `exec.rs` below its `#[cfg(test)]` cut and \
             so was never in this domain; it is named for the same reason \
             `fake.rs` is, the marker being at the DECLARATION and not in the file",
        ),
        (
            "src/runner/container/fake.rs",
            "the funnel's `#[cfg(test)]` substrate — the fake runtime and the \
             Docker gate. Excluded from nothing by `production_region`, because \
             the `#[cfg(test)]` marker is at the DECLARATION and not in the file",
        ),
        (
            "src/runner/container/tests.rs",
            "the funnel's `#[cfg(test)]` suite, for the same reason",
        ),
    ];
    let expected: BTreeSet<&str> = NAMES_A_CONTAINER_RUNTIME
        .iter()
        .map(|(path, _)| *path)
        .collect();
    let mut naming: BTreeSet<String> = BTreeSet::new();
    for (path, source) in scanned_sources() {
        let production = blank_comments(&production_region(&source));
        for needle in ["\"docker", "\"podman", "docker::", "bollard", "DockerCli"] {
            if production.contains(needle) {
                naming.insert(path.clone());
            }
        }
    }
    assert_eq!(
        naming,
        expected.iter().map(|p| (*p).to_owned()).collect(),
        "the set of files naming a container runtime moved. A new one is either \
         a helper the denylist does not name, or a row this table needs"
    );

    for helper in [
        "upstroke::runner::container::runtime::ContainerRuntime::create",
        "upstroke::runner::container::runtime::ContainerRuntime::start",
        "upstroke::runner::container::runtime::ContainerRuntime::stop",
        "upstroke::runner::container::runtime::ContainerRuntime::remove",
        "upstroke::runner::container::GitView::materialize",
        "upstroke::runner::container::GitView::discard",
    ] {
        assert!(
            methods.contains(helper),
            "`{helper}` is a docker invocation helper and disallowed-methods does \
             not name it"
        );
    }
}

#[test]
fn every_denied_path_this_host_can_resolve_does_resolve() {
    let scratch = scratch_dir("resolve");
    let denied_text = fs::read_to_string(repo_root().join(CLIPPY_TOML)).expect("clippy.toml");
    let stripped = denied_text.replace(", allow-invalid = true", "");
    assert_ne!(stripped, denied_text, "no allow-invalid entry to strip");
    fs::write(scratch.join(CLIPPY_TOML), &stripped).expect("the probe config");

    let unresolved = unresolved_paths(&scratch, "probe");
    let expected: BTreeSet<String> = host_conditional_paths()
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        unresolved, expected,
        "the set of paths this host cannot resolve moved. Anything new here is a \
         denial that enforces nothing."
    );

    let with_typo = format!("{stripped}\n[[extra]]\n",).replace("[[extra]]\n", "");
    let with_typo = with_typo.replace(
        "disallowed-methods = [",
        "disallowed-methods = [\n    { path = \"std::fs::wrrite\", reason = \"UPSTROKE-EFFECT: control\" },",
    );
    fs::write(scratch.join(CLIPPY_TOML), with_typo).expect("the control config");
    let control = unresolved_paths(&scratch, "control");
    assert!(
        control.contains("std::fs::wrrite"),
        "the control typo was not reported: {control:?}"
    );
}

fn unresolved_paths(dir: &Path, tag: &str) -> BTreeSet<String> {
    let (deps, rlib) = crate_under_test();
    let source = dir.join(format!("{tag}.rs"));
    fs::write(&source, "pub fn nothing() {}\n").expect("the probe source");
    let out = dir.join(format!("{tag}-out"));
    fs::create_dir_all(&out).expect("an output directory");
    let mut command = std::process::Command::new(clippy_driver());
    command
        .env("CLIPPY_CONF_DIR", dir)
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit=metadata",
        ])
        .arg("--out-dir")
        .arg(&out)
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("upstroke={}", rlib.display()));
    for (name, path) in extern_dependencies(&deps) {
        command
            .arg("--extern")
            .arg(format!("{name}={}", path.display()));
    }
    let output = command
        .arg(&source)
        .output()
        .expect("clippy-driver runs; the lint gate uses the same binary");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    stderr
        .lines()
        .filter(|line| line.contains("does not refer to a reachable"))
        .filter_map(|line| {
            let start = line.find('`')? + 1;
            let end = line[start..].find('`')? + start;
            Some(line[start..end].to_owned())
        })
        .collect()
}

fn extern_dependencies(deps: &Path) -> Vec<(String, PathBuf)> {
    let mut best: BTreeMap<String, (std::time::SystemTime, PathBuf)> = BTreeMap::new();
    let Ok(entries) = fs::read_dir(deps) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(stem) = name
            .strip_prefix("lib")
            .and_then(|n| n.strip_suffix(".rlib"))
        else {
            continue;
        };
        let Some((crate_name, _)) = stem.rsplit_once('-') else {
            continue;
        };
        if crate_name == "upstroke" {
            continue;
        }
        let stamp = path
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        let slot = best
            .entry(crate_name.replace('-', "_"))
            .or_insert((stamp, path.clone()));
        if stamp >= slot.0 {
            *slot = (stamp, path);
        }
    }
    best.into_iter()
        .map(|(name, (_, path))| (name, path))
        .collect()
}

#[test]
fn every_platform_conditional_denial_names_something_real() {
    let denied = denylist();
    let sources: String = scanned_sources()
        .into_iter()
        .map(|(_, source)| source)
        .collect();
    let mut checked = 0;
    for entry in denied.all() {
        let conditional = entry.path.starts_with("windows_sys::")
            || entry.path.starts_with("libc::")
            || entry.path.starts_with("std::os::");
        if !conditional {
            continue;
        }
        let item = entry.path.rsplit("::").next().expect("a path has an item");
        const PACKET_ONLY: &[&str] = &[
            "setsid",
            "execv",
            "execve",
            "execvp",
            "execl",
            "execle",
            "execlp",
            "soft_link",
            "symlink_file",
            "symlink_dir",
            "OpenProcess",
            "TerminateProcess",
            "ResumeThread",
            "OpenJobObjectW",
            "TerminateJobObject",
            "UnlockFileEx",
        ];
        checked += 1;
        assert!(
            sources.contains(item) || PACKET_ONLY.contains(&item),
            "`{}` names `{item}`, which appears nowhere in this tree and is not one \
             of the primitives the packet's sentence requires regardless",
            entry.path
        );
    }
    assert!(
        checked >= 30,
        "only {checked} platform-conditional denials were checked"
    );

    let suppressed: BTreeSet<&str> = denied
        .all()
        .filter(|entry| entry.allow_invalid)
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(
        suppressed,
        BTreeSet::from([
            "libc::pipe2",
            "std::os::unix::fs::symlink",
            "std::os::windows::fs::symlink_dir",
            "std::os::windows::fs::symlink_file",
        ]),
        "an entry bought silence about whether it resolves"
    );
}

#[test]
fn every_declared_effect_denial_refuses_for_the_reason_it_declares() {
    let scratch = scratch_dir("denial");

    let (ok, diagnostics) = lint_fixture(&scratch, "control", DENIAL_CONTROL);
    assert!(
        ok && diagnostics.is_empty(),
        "the positive control did not compile clean, so no refusal below is \
         evidence of anything:\n{diagnostics:#?}"
    );

    let mut shapes = BTreeSet::new();
    let mut lints = BTreeSet::new();
    for fixture in DENIAL_FIXTURES {
        let tag = fixture.shape.replace([' ', '-'], "_");
        let (_, diagnostics) = lint_fixture(&scratch, &tag, fixture.source);
        let emitted: BTreeSet<&str> = diagnostics.iter().map(|(lint, _)| lint.as_str()).collect();
        assert_eq!(
            emitted,
            BTreeSet::from([fixture.lint]),
            "the `{}` fixture emitted {emitted:?}, not exactly {{{}}}",
            fixture.shape,
            fixture.lint
        );
        let named = diagnostics
            .iter()
            .any(|(_, message)| message.contains(fixture.resolves_to));
        assert!(
            named,
            "the `{}` fixture was denied, but clippy's message never names `{}` -- \
             so this proves a refusal, not that the alias resolved: {diagnostics:#?}",
            fixture.shape, fixture.resolves_to
        );
        shapes.insert(fixture.shape);
        lints.insert(fixture.lint);
    }

    assert_eq!(shapes.len(), 7, "{shapes:?}");
    assert_eq!(lints.len(), 3, "{lints:?}");
    for required in [
        "renamed-import",
        "re-export",
        "function-value",
        "legacy-wrapper call",
    ] {
        assert!(
            shapes.contains(required),
            "proof_tests[4] names `{required}`"
        );
    }
}

#[test]
fn the_topology_root_re_denies_every_lint_the_engine_facade_allows() {
    // #306 (`PR7-WRAPPERS-EMPTY-DOMAIN`) put `#![allow(clippy::disallowed_methods)]`
    // on `src/engine/mod.rs` so the v0.1 facade could call two conductor entry
    // points denied by path. A lint level is scoped by the module tree, so
    // that allow reached every module under `engine::topology` -- forty files,
    // none writing an attribute of its own -- and the placement scan could not
    // see it: `governed_allows` records what a file WRITES, and a child
    // exempted by inheritance writes nothing. What stopped it was
    // `#![deny(..)]` on `src/engine/topology.rs`, the one root every topology
    // child descends from. The facade's allow is gone since 2026-09-20
    // (`PR306-FACADE-INLINE-ESCAPE`: its entry points moved into the conductor
    // modules, and `the_engine_facade_allows_no_governed_lint_and_refuses_both_escape_routes`
    // holds that), and the root's deny stays: it is what keeps the topology
    // closed against an allow written above it, by the facade again or by
    // anything else, and an attribute somebody can delete is a weaker
    // guarantee than the absence of an allow ever was. So this test holds it
    // twice: lexically, from the two files, and executed, by compiling the
    // same shape -- an ancestor with the allow #306 wrote, a topology root
    // with this tree's deny, a child reaching one denied primitive per
    // governed lint -- against the real denylist, beside the shape without
    // the deny, which is what removing it would leave.
    const FACADE: &str = "src/engine/mod.rs";
    const TOPOLOGY_ROOT: &str = "src/engine/topology.rs";
    const GOVERNED: [&str; 3] = [
        "clippy::disallowed_methods",
        "clippy::disallowed_types",
        "clippy::disallowed_macros",
    ];
    const MODELLED_ANCESTOR_ALLOW: [&str; 1] = ["clippy::disallowed_methods"];

    let facade = fs::read_to_string(repo_root().join(FACADE)).expect(FACADE);
    let written_by_the_facade: BTreeSet<String> = governed_allows(&facade)
        .iter()
        .flat_map(|allow| allow.lints.iter().cloned())
        .collect();
    let allowed: BTreeSet<String> = MODELLED_ANCESTOR_ALLOW
        .iter()
        .filter_map(|lint| normalize_lint(lint))
        .map(str::to_owned)
        .collect();
    assert!(
        written_by_the_facade.is_subset(&allowed),
        "{FACADE} allows {written_by_the_facade:?}, which is more than the ancestor allow this \
         test models ({allowed:?}); the fixtures below would no longer show what the topology \
         root's deny is held against"
    );

    let topology = fs::read_to_string(repo_root().join(TOPOLOGY_ROOT)).expect(TOPOLOGY_ROOT);
    let denied: Vec<&str> = GOVERNED
        .iter()
        .copied()
        .filter(|lint| file_level_denies(&topology, lint))
        .collect();
    assert_eq!(
        denied, GOVERNED,
        "{TOPOLOGY_ROOT} no longer denies every governed lint at file level, so every module \
         under `engine::topology` inherits whatever a module above it allows -- as it inherited \
         {FACADE}'s allow on #306 -- and the placement scan cannot see it \
         (`PR7-WRAPPERS-EMPTY-DOMAIN`)"
    );
    let mut children = 0;
    for (path, source) in scanned_sources() {
        if !path.starts_with("src/engine/topology/") {
            continue;
        }
        children += 1;
        assert!(
            governed_allows(&source).is_empty(),
            "{path} allows a governed lint below the topology root's deny; that deny is the \
             guarantee this test holds and a child's allow re-opens it"
        );
    }
    assert!(children > 30, "only {children} topology children scanned");

    let scratch = scratch_dir("facade");
    fs::write(
        scratch.join("facade-child.rs"),
        "pub fn probe(p: &std::path::Path) -> bool {\n\
         \x20   let _ = upstroke::util::write_text(p, \"x\");\n\
         \x20   println!(\"{}\", p.display());\n\
         \x20   p.exists()\n\
         }\n\
         pub fn takes(_command: std::process::Command) {}\n",
    )
    .expect("the child fixture");
    fs::write(
        scratch.join("facade-topology-open.rs"),
        "#[path = \"facade-child.rs\"]\npub mod child;\n",
    )
    .expect("the open topology fixture");
    fs::write(
        scratch.join("facade-topology-denying.rs"),
        format!(
            "#![deny({})]\n#[path = \"facade-child.rs\"]\npub mod child;\n",
            denied.join(", ")
        ),
    )
    .expect("the denying topology fixture");
    let facade_allow = format!("#![allow({})]\n", MODELLED_ANCESTOR_ALLOW.join(", "));
    let root = |allow: &str, topology_file: &str| {
        format!(
            "{allow}#[path = \"{topology_file}\"]\npub mod topology;\n\
             pub fn conductor(p: &std::path::Path) {{\n\
             \x20   let _ = upstroke::util::write_json(p, &1_u8);\n\
             }}\n"
        )
    };
    let codes = |diagnostics: &[(String, String)]| -> Vec<String> {
        let mut codes: Vec<String> = diagnostics.iter().map(|(code, _)| code.clone()).collect();
        codes.sort();
        codes
    };
    let naming = |diagnostics: &[(String, String)], needle: &str| -> usize {
        diagnostics
            .iter()
            .filter(|(_, message)| message.contains(needle))
            .count()
    };

    // No attribute anywhere: all four reaches are refused, so the fixture
    // sees everything the two shapes below can hide.
    let (ok, control) = lint_fixture(
        &scratch,
        "facade_control",
        &root("", "facade-topology-open.rs"),
    );
    assert!(
        ok,
        "the control shape must compile with warnings only: {control:#?}"
    );
    assert_eq!(
        codes(&control),
        vec![
            "clippy::disallowed_macros",
            "clippy::disallowed_methods",
            "clippy::disallowed_methods",
            "clippy::disallowed_types",
        ],
        "{control:#?}"
    );
    assert_eq!(
        naming(&control, "upstroke::util::write_json"),
        1,
        "{control:#?}"
    );
    assert_eq!(
        naming(&control, "upstroke::util::write_text"),
        1,
        "{control:#?}"
    );

    // An ancestor's allow with nothing below it: the child's reach into a
    // denied wrapper goes unrefused, and no file wrote the allow that let it.
    let (ok, inherited) = lint_fixture(
        &scratch,
        "facade_inherit",
        &root(&facade_allow, "facade-topology-open.rs"),
    );
    assert!(
        ok,
        "the inherited shape must compile with warnings only: {inherited:#?}"
    );
    assert_eq!(
        codes(&inherited),
        vec!["clippy::disallowed_macros", "clippy::disallowed_types"],
        "a module-level allow on an ancestor did not reach the topology child, so the deny \
         this test holds guards nothing: {inherited:#?}"
    );
    assert_eq!(
        naming(&inherited, "upstroke::util::write_text"),
        0,
        "{inherited:#?}"
    );

    // This tree's root under that ancestor: the root's deny makes the child's
    // three reaches build errors again, and the ancestor's own call stays
    // under the allow it wrote.
    let (ok, tree) = lint_fixture(
        &scratch,
        "facade_tree",
        &root(&facade_allow, "facade-topology-denying.rs"),
    );
    assert!(
        !ok,
        "the topology root's deny must make the child's reach a build error: {tree:#?}"
    );
    assert_eq!(
        codes(&tree),
        vec![
            "clippy::disallowed_macros",
            "clippy::disallowed_methods",
            "clippy::disallowed_types",
        ],
        "{tree:#?}"
    );
    assert_eq!(
        naming(&tree, "upstroke::util::write_text"),
        1,
        "the topology child reached a denied wrapper under an ancestor's allow and was not \
         refused: {tree:#?}"
    );
    assert_eq!(
        naming(&tree, "upstroke::util::write_json"),
        0,
        "the ancestor's own call is what its allow is for: {tree:#?}"
    );
}

const ENGINE_FACADE: &str = "src/engine/mod.rs";

const FACADE_ALLOW_OF_306: &str = "#![allow(clippy::disallowed_methods)]\n";

struct EngineModule {
    path: String,
    parent: String,
    test_only: bool,
    inherited: BTreeSet<String>,
    own: BTreeSet<String>,
    denied: BTreeSet<String>,
    source: String,
    inline: Vec<crate::effects::census_domain::ScannedInlineModule>,
}

impl EngineModule {
    fn in_effect(&self) -> BTreeSet<String> {
        self.inherited
            .difference(&self.denied)
            .cloned()
            .chain(self.own.iter().cloned())
            .collect()
    }
}

fn governed_lints_in_use() -> BTreeSet<String> {
    USED_GOVERNED_LINTS
        .iter()
        .filter_map(|lint| normalize_lint(lint))
        .map(str::to_owned)
        .collect()
}

fn recorded_allows(list: &Allowlist) -> BTreeMap<&str, BTreeSet<String>> {
    list.funnel
        .iter()
        .chain(list.legacy.iter())
        .map(|entry| {
            let allows = entry
                .allows
                .iter()
                .filter_map(|lint| normalize_lint(lint))
                .map(str::to_owned)
                .collect();
            (entry.path.as_str(), allows)
        })
        .collect()
}

fn engine_module_tree() -> Vec<EngineModule> {
    use crate::effects::census_domain::{candidates_for, scan_modules, sole_present};

    let root = repo_root();
    let roots = crate_roots();
    let mut pending: Vec<(String, String, bool, BTreeSet<String>)> = vec![(
        ENGINE_FACADE.to_owned(),
        String::new(),
        false,
        BTreeSet::new(),
    )];
    let mut walked: Vec<EngineModule> = Vec::new();
    while let Some((path, parent, test_only, inherited)) = pending.pop() {
        assert!(
            walked.iter().all(|module| module.path != path),
            "{path} is declared twice under {ENGINE_FACADE}"
        );
        let source = fs::read_to_string(root.join(&path)).expect("a module the walk resolved");
        let own: BTreeSet<String> = governed_allows(&source)
            .iter()
            .filter(|allow| allow.inner && allow.module_level)
            .flat_map(|allow| allow.lints.iter().cloned())
            .collect();
        let denied: BTreeSet<String> = USED_GOVERNED_LINTS
            .iter()
            .filter(|lint| file_level_denies(&source, lint))
            .filter_map(|lint| normalize_lint(lint))
            .map(str::to_owned)
            .collect();
        let scanned = scan_modules(&source).unwrap_or_else(|refusal| panic!("{path}: {refusal}"));
        let module = EngineModule {
            path: path.clone(),
            parent,
            test_only,
            inherited,
            own,
            denied,
            source,
            inline: scanned.inline,
        };
        let in_effect = module.in_effect();
        let declared_in = root.join(&path);
        for declaration in scanned.declared {
            let candidates = candidates_for(
                roots,
                &declared_in,
                &declaration.inline_path,
                &declaration.name,
            )
            .unwrap_or_else(|refusal| panic!("{path}: {refusal}"));
            let file = sole_present(&candidates, &|candidate: &Path| candidate.is_file())
                .unwrap_or_else(|present| {
                    panic!(
                        "`mod {};` in {path} resolves to {present} files among {candidates:?}; the \
                         tree has to be readable for this guard to walk it",
                        declaration.name
                    )
                });
            let child = file
                .strip_prefix(&root)
                .expect("under the manifest")
                .to_string_lossy()
                .replace('\\', "/");
            pending.push((
                child,
                path.clone(),
                test_only || declaration.test_only,
                in_effect.clone(),
            ));
        }
        walked.push(module);
    }
    walked
}

#[test]
fn every_child_the_engine_facade_declares_re_denies_or_records_what_it_inherits() {
    // #306, round 3 (`PR306-FACADE-ALLOW-ESCAPES-TO-SIBLINGS`): the guard
    // above holds the deny on `src/engine/topology.rs` and walks only the
    // files under it. `src/engine/mod.rs` declares nine other children, and a
    // lint level inherits into each of them just the same: when the facade
    // allowed `disallowed_methods`, `assembly`, `classify`, `options`,
    // `preflight` and `report` wrote no attribute, so they inherited the
    // allow, the placement scan recorded nothing, and the third review proved
    // it -- a `pub(super) fn` in `engine/assembly.rs` calling `std::fs::write`,
    // referenced from a production body of `engine::topology::integrate`,
    // passed clippy and the whole suite. So the boundary is walked from the
    // facade's own `mod` declarations, recursively, never from a list
    // (`engine_module_tree`): every module carries forward the allows in
    // effect at its parent, and for each lint it inherits it must deny that
    // lint at file level or write its own module-level allow that
    // `effects/allowlist.toml` records. The facade's allow is gone since
    // 2026-09-20, and the fences #306 wrote are held anyway, whatever the
    // facade writes: a module the facade declares itself states its own level
    // -- the whole three-lint fence the topology root wrote first, or a
    // recorded allow of its own -- so that the day an allow is written above
    // them again, by anyone, it reaches nothing. Then the review's witness is
    // compiled: an ancestor with the allow #306 wrote, a sibling reaching
    // `std::fs::write`, a denying topology module referencing the sibling --
    // open, the reach is unreported and the crate builds; fenced with the
    // attribute the children write, it is a build error again.
    const FACADE: &str = ENGINE_FACADE;
    const GOVERNED: [&str; 3] = [
        "clippy::disallowed_methods",
        "clippy::disallowed_types",
        "clippy::disallowed_macros",
    ];
    const SIBLING: &str = "pub(crate) fn r2_unrecorded_inherited_effect(\n\
         \x20   p: &std::path::Path,\n\
         ) -> std::io::Result<()> {\n\
         \x20   std::fs::write(p, b\"r2 effect\")\n\
         }\n";

    let governed = governed_lints_in_use();
    let list = allowlist();
    let recorded = recorded_allows(&list);

    let tree = engine_module_tree();
    let mut fenced: Vec<&str> = Vec::new();
    let mut recording: Vec<&str> = Vec::new();
    for module in &tree {
        let EngineModule {
            path,
            parent,
            inherited,
            own,
            denied,
            ..
        } = module;
        let rows = recorded.get(path.as_str());
        for lint in inherited {
            let records = own.contains(lint) && rows.is_some_and(|allows| allows.contains(lint));
            assert!(
                denied.contains(lint) || records,
                "{path}, declared by {parent}, neither denies `{lint}` at file level nor records its \
                 own allow of it in {ALLOWLIST_TOML}, so it is exempt by inheritance from {parent}'s \
                 allow and the placement scan cannot see it -- the third review's sibling witness \
                 (`PR306-FACADE-ALLOW-ESCAPES-TO-SIBLINGS`, #306); it writes {own:?} and the \
                 allowlist records {rows:?}"
            );
        }
        let declared_by_the_facade = parent == FACADE;
        if inherited.is_empty() && !declared_by_the_facade {
            continue;
        }
        if own.is_empty() {
            assert_eq!(
                denied, &governed,
                "{path}, declared by {parent}, writes no allow of its own, so it must carry the \
                 whole fence `src/engine/topology.rs` wrote first -- every governed lint denied at \
                 file level -- whether or not {parent} allows anything today: it inherits \
                 {inherited:?}, and a module that leans on its parent's level is exempt the day \
                 that level changes, which is what #306 did to it"
            );
            fenced.push(path);
        } else {
            assert!(
                rows.is_some_and(|allows| own.is_subset(allows)),
                "{path}, declared by {parent}, writes {own:?} and {ALLOWLIST_TOML} records {rows:?}"
            );
            recording.push(path);
        }
    }
    assert!(
        tree.len() > 40,
        "only {} modules walked from {FACADE}: {:?}",
        tree.len(),
        tree.iter().map(|module| &module.path).collect::<Vec<_>>()
    );
    assert!(
        fenced.len() > 1 && recording.len() > 1,
        "the walk from {FACADE} found {fenced:?} fenced and {recording:?} recording, which is not \
         the tree this guard was written against"
    );

    // The review's witness, compiled: a sibling reaching `std::fs::write`, a
    // topology module carrying the root's deny and referencing the sibling,
    // and an ancestor allowing what this tree's facade allowed on #306.
    let scratch = scratch_dir("siblings");
    let fence = format!("#![deny({})]\n", GOVERNED.join(", "));
    fs::write(scratch.join("sibling-open.rs"), SIBLING).expect("the open sibling fixture");
    fs::write(
        scratch.join("sibling-fenced.rs"),
        format!("{fence}{SIBLING}"),
    )
    .expect("the fenced sibling fixture");
    fs::write(
        scratch.join("sibling-topology.rs"),
        format!(
            "{fence}pub fn park(p: &std::path::Path) -> bool {{\n\
             \x20   crate::sibling::r2_unrecorded_inherited_effect(p).is_ok()\n\
             }}\n"
        ),
    )
    .expect("the topology fixture");
    let facade_allow = FACADE_ALLOW_OF_306;
    let root_of = |allow: &str, sibling: &str| {
        format!(
            "{allow}#[path = \"{sibling}\"]\nmod sibling;\n\
             #[path = \"sibling-topology.rs\"]\npub mod topology;\n\
             pub fn conductor(p: &std::path::Path) {{\n\
             \x20   let _ = upstroke::util::write_json(p, &1_u8);\n\
             }}\n"
        )
    };
    let codes = |diagnostics: &[(String, String)]| -> Vec<String> {
        let mut codes: Vec<String> = diagnostics.iter().map(|(code, _)| code.clone()).collect();
        codes.sort();
        codes
    };
    let naming = |diagnostics: &[(String, String)], needle: &str| -> usize {
        diagnostics
            .iter()
            .filter(|(_, message)| message.contains(needle))
            .count()
    };

    // No attribute anywhere: the sibling's reach and the facade's own are
    // both reported, so the fixture sees what the two shapes below can hide.
    let (ok, control) = lint_fixture(
        &scratch,
        "siblings_control",
        &root_of("", "sibling-open.rs"),
    );
    assert!(
        ok,
        "the control shape must compile with warnings only: {control:#?}"
    );
    assert_eq!(
        codes(&control),
        vec!["clippy::disallowed_methods", "clippy::disallowed_methods"],
        "{control:#?}"
    );
    assert_eq!(naming(&control, "std::fs::write"), 1, "{control:#?}");
    assert_eq!(
        naming(&control, "upstroke::util::write_json"),
        1,
        "{control:#?}"
    );

    // The allow #306 wrote over an open sibling, the topology root denying: the
    // sibling's reach goes unreported, the crate builds, and no file wrote the
    // allow that let it -- the hole the third review executed.
    let (ok, hole) = lint_fixture(
        &scratch,
        "siblings_inherited",
        &root_of(facade_allow, "sibling-open.rs"),
    );
    assert!(
        ok,
        "the inherited shape must compile with warnings only: {hole:#?}"
    );
    assert!(
        hole.is_empty(),
        "an allow above the open sibling did not reach it, so the fence this test holds guards \
         nothing: {hole:#?}"
    );

    // The same ancestor over this tree's sibling: it carries the fence the
    // children write, its reach is a build error again, and the ancestor's own
    // call stays under the allow it wrote.
    let (ok, tree) = lint_fixture(
        &scratch,
        "siblings_fenced",
        &root_of(facade_allow, "sibling-fenced.rs"),
    );
    assert!(
        !ok,
        "the sibling's fence must make its reach a build error: {tree:#?}"
    );
    assert_eq!(
        codes(&tree),
        vec!["clippy::disallowed_methods"],
        "{tree:#?}"
    );
    assert_eq!(
        naming(&tree, "std::fs::write"),
        1,
        "the sibling reached a denied primitive under an ancestor's allow and was not refused: \
         {tree:#?}"
    );
    assert_eq!(
        naming(&tree, "upstroke::util::write_json"),
        0,
        "the ancestor's own call is what its allow is for: {tree:#?}"
    );
}

#[test]
fn the_engine_facade_allows_no_governed_lint_and_refuses_both_escape_routes() {
    use crate::effects::lint_levels::leading_inner_attributes;

    const INLINE_ROUTE: &str = "mod r3_inline_child {\n\
         \x20   pub(super) fn r3_inline_inherited_effect(\n\
         \x20       p: &std::path::Path,\n\
         \x20   ) -> std::io::Result<()> {\n\
         \x20       std::fs::write(p, b\"r3 inline effect\")\n\
         \x20   }\n\
         }\n";
    const DIRECT_ROUTE: &str = "fn r3_unclassified_facade_effect(\n\
         \x20   p: &std::path::Path,\n\
         ) -> std::io::Result<()> {\n\
         \x20   std::fs::write(p, b\"r3 facade effect\")\n\
         }\n";

    let governed = governed_lints_in_use();
    let tree = engine_module_tree();
    let by_path: BTreeMap<&str, &EngineModule> = tree
        .iter()
        .map(|module| (module.path.as_str(), module))
        .collect();

    let mut above_topology: BTreeSet<&str> = BTreeSet::new();
    let mut topology_modules = 0;
    for module in &tree {
        if topology_modules_among(&[module.path.as_str()]).is_empty() {
            continue;
        }
        topology_modules += 1;
        let mut at = module.parent.as_str();
        while let Some(parent) = by_path.get(at) {
            above_topology.insert(at);
            at = parent.parent.as_str();
        }
    }
    assert!(
        topology_modules > 30,
        "only {topology_modules} topology modules were walked from {ENGINE_FACADE}"
    );
    assert!(
        above_topology.contains(ENGINE_FACADE)
            && above_topology
                .iter()
                .any(|path| topology_modules_among(&[*path]).is_empty()),
        "the walk found no module outside the topology that a topology module descends from, \
         which is the one thing this test exists to hold: {above_topology:?}"
    );
    for path in &above_topology {
        let module = by_path.get(path).expect("a walked module");
        let written = governed_allows(&module.source);
        assert!(
            written.is_empty(),
            "{path} writes an allow of a governed lint, and a topology module descends from it. \
             Everything in that file -- a private fn, an inline `mod x {{ .. }}` at any depth -- is \
             visible to that topology module and covered by the allow, and nothing classifies \
             it: the two routes the fourth review of #306 executed \
             (`PR306-FACADE-INLINE-ESCAPE`). Move what needs the allow into a module that may \
             carry one; found {written:#?}"
        );
        assert!(
            module.in_effect().is_empty(),
            "{path} has {:?} allowed in effect by inheritance from {}, and a topology module \
             descends from it",
            module.in_effect(),
            module.parent
        );
    }
    for path in &above_topology {
        if !topology_modules_among(&[*path]).is_empty() {
            continue;
        }
        let module = by_path.get(path).expect("a walked module");
        let beyond = items_beyond_declarations(&module.source);
        assert!(
            beyond.is_empty(),
            "{path} holds something other than its leading attributes, `mod x;` declarations and \
             `use` re-exports, and a topology module descends from it. Whatever it holds is \
             visible to that topology module; an inline module, a function, a macro or an \
             `include!` can carry an allow that no scan of this file reads -- the review of \
             409a6138 brought one in through `include!` and one through a macro that substitutes \
             `mod` and `allow` (`PR309-FACADE-EXPANSION-ESCAPE`). Put it in a module this one \
             declares; found (line, item): {beyond:#?}"
        );
    }
    let facade = by_path
        .get(ENGINE_FACADE)
        .expect("the walk starts at the facade");
    assert_eq!(
        facade.denied, governed,
        "{ENGINE_FACADE} no longer denies every governed lint at file level. No walked module \
         stands above it, so without that deny its level -- and the level of every inline module \
         and item in it -- is whatever the crate root and the command line say"
    );

    let scratch = scratch_dir("routes");
    let fence = format!("#![deny({})]\n", USED_GOVERNED_LINTS.join(", "));
    let header = leading_inner_attributes(&facade.source);
    assert!(
        header.contains("#![deny("),
        "the facade's leading attributes were read as {header:?}, which carries no deny; the \
         compiled shape below would not be this tree's"
    );
    let routes = [
        (
            "inline",
            INLINE_ROUTE,
            "crate::r3_inline_child::r3_inline_inherited_effect",
        ),
        (
            "direct",
            DIRECT_ROUTE,
            "crate::r3_unclassified_facade_effect",
        ),
    ];
    for (route, held_by_the_facade, reach) in routes {
        fs::write(
            scratch.join(format!("routes-topology-{route}.rs")),
            format!(
                "{fence}pub fn park(p: &std::path::Path) -> bool {{\n\
                 \x20   {reach}(p).is_ok()\n\
                 }}\n"
            ),
        )
        .expect("the topology fixture");
        let facade_of = |attributes: &str| {
            format!(
                "{attributes}\n{held_by_the_facade}\
                 #[path = \"routes-topology-{route}.rs\"]\npub mod topology;\n"
            )
        };
        let naming = |diagnostics: &[(String, String)]| -> usize {
            diagnostics
                .iter()
                .filter(|(code, message)| {
                    code == "clippy::disallowed_methods" && message.contains("std::fs::write")
                })
                .count()
        };

        let (ok, control) =
            lint_fixture(&scratch, &format!("routes_{route}_control"), &facade_of(""));
        assert!(
            ok,
            "{route}: the control shape must compile with warnings only: {control:#?}"
        );
        assert_eq!(
            (control.len(), naming(&control)),
            (1, 1),
            "{route}: {control:#?}"
        );

        let (ok, hole) = lint_fixture(
            &scratch,
            &format!("routes_{route}_under_the_allow_of_306"),
            &facade_of(FACADE_ALLOW_OF_306),
        );
        assert!(
            ok && hole.is_empty(),
            "{route}: the allow #306 wrote on the facade did not cover what the facade holds, so \
             the refusal below proves nothing: ok={ok} {hole:#?}"
        );

        let (ok, tree_shape) = lint_fixture(
            &scratch,
            &format!("routes_{route}_this_tree"),
            &facade_of(header),
        );
        assert!(
            !ok,
            "{route}: under {ENGINE_FACADE}'s own attributes the reach must be a build error, and \
             the fixture built: {tree_shape:#?}"
        );
        assert_eq!(
            (tree_shape.len(), naming(&tree_shape)),
            (1, 1),
            "{route}: a topology module reached `std::fs::write` through what the facade holds \
             and was not refused for exactly that: {tree_shape:#?}"
        );
    }
}

fn items_beyond_declarations(source: &str) -> Vec<(usize, String)> {
    use crate::effects::lint_levels::leading_inner_attributes;

    let blanked = blank_comments_and_strings(source);
    let is_ident = |word: &str| {
        !word.is_empty()
            && word
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    };
    let mut beyond = Vec::new();
    let mut at = leading_inner_attributes(source).len();
    while let Some(rest) = blanked.get(at..) {
        let from = at + (rest.len() - rest.trim_start().len());
        let Some(text) = blanked.get(from..).filter(|text| !text.is_empty()) else {
            break;
        };
        let length = text.find(';').map_or(text.len(), |semicolon| semicolon + 1);
        let item = text.get(..length).unwrap_or(text);
        let written = item.trim_end_matches(';').trim();
        let written = written
            .strip_prefix("#[cfg(test)]")
            .map_or(written, str::trim_start);
        let words: Vec<&str> = written.split_whitespace().collect();
        let declares_a_module = match words.as_slice() {
            ["mod", name] => is_ident(name),
            [visibility, "mod", name] => {
                visibility.starts_with("pub") && !visibility.contains('!') && is_ident(name)
            }
            _ => false,
        };
        let re_exports = match words.as_slice() {
            ["use", ..] => true,
            [visibility, "use", ..] => visibility.starts_with("pub"),
            _ => false,
        } && written.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || byte.is_ascii_whitespace()
                || matches!(byte, b'_' | b':' | b'{' | b'}' | b',' | b'*' | b'(' | b')')
        });
        if !(declares_a_module || re_exports) {
            let line = blanked
                .get(..from)
                .map_or(0, |before| before.matches('\n').count())
                + 1;
            let shown: String = written.split_whitespace().collect::<Vec<_>>().join(" ");
            beyond.push((line, shown.chars().take(96).collect()));
        }
        at = from + length;
    }
    beyond
}

fn includes_a_file(source: &str) -> bool {
    let blanked = blank_comments_and_strings(source);
    let bytes = blanked.as_bytes();
    blanked.match_indices("include").any(|(at, word)| {
        let glued = at
            .checked_sub(1)
            .and_then(|before| bytes.get(before))
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
        let after = blanked
            .get(at + word.len()..)
            .unwrap_or_default()
            .trim_start();
        !glued && after.starts_with('!')
    })
}

#[test]
fn no_scanned_source_includes_a_file_no_scan_reads() {
    let mut scanned = 0;
    for (path, source) in scanned_sources() {
        scanned += 1;
        assert!(
            !includes_a_file(&source),
            "{path} uses `include!`. What it includes is Rust that no scan here reads -- not the \
             placement scan, not the module walk, not the classification census, each of which \
             reads `.rs` sources -- so an allow, a module or a function arrives unread; the \
             review of 409a6138 brought an allowed inline module into the engine facade that way \
             (`PR309-FACADE-EXPANSION-ESCAPE`)"
        );
    }
    assert!(scanned > 150, "only {scanned} sources were scanned");
    for (text, includes) in [
        ("include!(\"x.inc\");\n", true),
        ("include ! { \"x.inc\" }\n", true),
        ("const TEXT: &str = include_str!(\"x.txt\");\n", false),
        ("const BYTES: &[u8] = include_bytes!(\"x.bin\");\n", false),
        (
            "// include!(\"prose.inc\");\nconst S: &str = \"include!(quoted)\";\n",
            false,
        ),
        ("fn preinclude() {}\n", false),
    ] {
        assert_eq!(includes_a_file(text), includes, "{text:?}");
    }
}

fn inline_module_openers(source: &str) -> usize {
    let blanked = blank_comments_and_strings(source);
    let bytes = blanked.as_bytes();
    let word = |byte: &u8| byte.is_ascii_alphanumeric() || *byte == b'_';
    let mut found = 0;
    for (at, _) in blanked.match_indices("mod") {
        let glued = at
            .checked_sub(1)
            .and_then(|before| bytes.get(before))
            .is_some_and(|byte| word(byte) || *byte == b'#');
        if glued {
            continue;
        }
        let mut cursor = at + "mod".len();
        let spaced = bytes.get(cursor).is_some_and(u8::is_ascii_whitespace);
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let name_from = cursor;
        while bytes.get(cursor).is_some_and(word) {
            cursor += 1;
        }
        if !spaced || cursor == name_from {
            continue;
        }
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'{') {
            found += 1;
        }
    }
    found
}

const DECLARATION_ONLY_MODULES: &[&str] = &[
    ENGINE_FACADE,
    "src/lib.rs",
    "src/agent/mod.rs",
    "src/runner/mod.rs",
];

#[test]
fn a_declaring_module_holds_declarations_and_re_exports_and_nothing_else() {
    for path in DECLARATION_ONLY_MODULES {
        let source = fs::read_to_string(repo_root().join(path)).expect(path);
        assert_eq!(items_beyond_declarations(&source), Vec::new(), "{path}");
        assert!(
            source.matches("mod ").count() >= 4,
            "{path} no longer declares modules, so the empty answer above says nothing"
        );
        let with_a_body = format!("{source}\nfn rf_probe_body() {{}}\n");
        assert_eq!(
            items_beyond_declarations(&with_a_body).len(),
            1,
            "{path}: a function appended to a declaring module is refused"
        );
    }
    let facade = fs::read_to_string(repo_root().join(ENGINE_FACADE)).expect(ENGINE_FACADE);
    assert!(
        facade.contains("pub use "),
        "the facade no longer re-exports, so the table below exercises nothing"
    );

    let holds = |addition: &str| -> Vec<String> {
        items_beyond_declarations(&format!("{facade}\n{addition}\n"))
            .into_iter()
            .map(|(_, item)| item)
            .collect()
    };
    for (addition, refused) in [
        ("mod plain;", 0),
        ("pub(crate) mod visible;", 0),
        ("#[cfg(test)]\nmod more_tests;", 0),
        (
            "pub use report::{one, two};\nuse crate::error::UpstrokeError;",
            0,
        ),
        ("mod inline_child {}", 1),
        ("mod inline_child { pub(super) fn f() -> u8 { 1 } }", 1),
        ("fn private() {}", 1),
        ("const LIMIT: usize = 3;", 1),
        ("include!(\"rf_review_include.inc\");", 1),
        (
            "#[allow(clippy::disallowed_methods)]\nmod assembly_again;",
            1,
        ),
        ("#[path = \"elsewhere.rs\"]\nmod elsewhere;", 1),
        (
            "macro_rules! rf_generate { ($kind:ident, $level:ident) => { \
             #[$level(clippy::disallowed_methods)] $kind rf_generated {} }; }\n\
             rf_generate!(mod, allow);",
            2,
        ),
    ] {
        assert_eq!(
            holds(addition).len(),
            refused,
            "{addition:?}: {:#?}",
            holds(addition)
        );
    }
    for separator in super::RUSTC_WHITESPACE {
        let declaration = format!("pub(crate) mod{separator}rf4_facade_child;");
        let walked = crate::effects::census_domain::scan_modules(&declaration)
            .expect("one declaration scans")
            .declared;
        assert_eq!(
            (holds(&declaration).len(), walked.len()),
            (0, 1),
            "U+{:04X}: the whitelist admits a declaration and the walk has to read the same one, \
             or the facade declares a child nobody judges -- with U+000B, U+0085, U+2028 and \
             U+2029 the whitelist admitted it and the walk did not (the review of 84123789)",
            u32::from(separator)
        );
        assert_eq!(
            holds(&format!("#[rustfmt::skip]\n{declaration}")).len(),
            1,
            "U+{:04X}: the attribute that silences `cargo fmt --check` is not a declaration",
            u32::from(separator)
        );
    }
}

#[test]
fn every_inline_module_under_the_engine_facade_is_walked_and_answered_for() {
    use crate::effects::lint_levels::leading_inner_attributes;

    let list = allowlist();
    let recorded = recorded_allows(&list);
    let classified: BTreeSet<&str> = super::CLASSIFIED_MODULES.iter().copied().collect();

    let tree = engine_module_tree();
    let mut visited = 0;
    let mut deepest = 0;
    let mut files_holding_one = 0;
    let mut exempt_in_production: Vec<&str> = Vec::new();
    for module in &tree {
        let path = module.path.as_str();
        assert_eq!(
            module.inline.len(),
            inline_module_openers(&module.source),
            "{path}: `scan_modules` reports {} inline modules and the text opens {} -- one of the \
             two readings has gone quiet, and an inline module nobody reports is one nobody \
             judges: {:?}",
            module.inline.len(),
            inline_module_openers(&module.source),
            module
                .inline
                .iter()
                .map(|inline| (inline.line, inline.name.as_str()))
                .collect::<Vec<_>>()
        );
        let rows = recorded.get(path);
        let in_the_file = module.in_effect();
        if !in_the_file.is_empty() && !module.test_only {
            exempt_in_production.push(path);
            assert!(
                classified.contains(path),
                "{path} has {in_the_file:?} allowed in effect in production code and is not in \
                 `CLASSIFIED_MODULES`, so no census classifies what it holds and nothing denies \
                 it to a topology module: the direct-facade route of `PR306-FACADE-INLINE-ESCAPE`"
            );
        }
        if !module.inline.is_empty() {
            files_holding_one += 1;
        }
        for inline in &module.inline {
            visited += 1;
            deepest = deepest.max(inline.inline_path.len() + 1);
            let own_attributes = format!(
                "{}\n{}\n",
                inline.outer_attributes,
                leading_inner_attributes(&inline.body)
            );
            let written: BTreeSet<String> = governed_allows(&own_attributes)
                .iter()
                .flat_map(|allow| allow.lints.iter().cloned())
                .collect();
            let named = if inline.inline_path.is_empty() {
                inline.name.clone()
            } else {
                format!("{}::{}", inline.inline_path.join("::"), inline.name)
            };
            for lint in in_the_file.iter().chain(&written) {
                assert!(
                    rows.is_some_and(|allows| allows.contains(lint)),
                    "{path}:{}: the inline module `{named}` has `{lint}` allowed in effect (the \
                     file has {in_the_file:?} in effect and the module writes {written:?}) and \
                     {ALLOWLIST_TOML} records {rows:?} for {path}; an inline module is answered \
                     for by its file's row or by nothing",
                    inline.line
                );
            }
            let exempt = !in_the_file.is_empty() || !written.is_empty();
            if exempt && !(module.test_only || inline.test_only) {
                assert!(
                    classified.contains(path),
                    "{path}:{}: the inline module `{named}` is production code with a governed \
                     lint allowed in effect, and {path} is not in `CLASSIFIED_MODULES`, so nothing \
                     classifies the fns it holds: the inline route of \
                     `PR306-FACADE-INLINE-ESCAPE`",
                    inline.line
                );
            }
        }
    }
    assert!(
        visited > 10 && files_holding_one > 4 && deepest > 1,
        "the walk from {ENGINE_FACADE} visited {visited} inline modules in {files_holding_one} \
         files, the deepest at depth {deepest}; this tree holds more than that, nested ones \
         included, so the derivation has stopped finding them"
    );
    assert!(
        exempt_in_production.len() > 2,
        "only {exempt_in_production:?} were found with a governed lint allowed in effect in \
         production, so the classification half of this test judged nothing"
    );
}

#[test]
fn every_separator_rustc_reads_is_one_every_reader_here_reads() {
    use crate::effects::census_domain::{ScanRefusal, scan_modules};
    use crate::effects::lint_levels::file_level_lint_state;

    const ATTEMPT: &str = "src/engine/attempt.rs";
    let hex = |separator: char| format!("U+{:04X}", u32::from(separator));

    assert_eq!(
        super::RUSTC_WHITESPACE.map(u32::from),
        [
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x20, 0x85, 0x200E, 0x200F, 0x2028, 0x2029
        ],
        "the one definition is rustc's eleven, by value: compiled once, outside the suite, in the \
         round's evidence, because a fixture compiled here is a long-lived child of the test \
         process"
    );

    let attempt = fs::read_to_string(repo_root().join(ATTEMPT)).expect(ATTEMPT);
    for separator in super::RUSTC_WHITESPACE {
        let at = hex(separator);

        let source =
            format!("mod{separator}a; // c{separator}\nconst S: &str = \"{separator}\";\n");
        let blanked = blank_comments_and_strings(&source);
        assert_eq!(blanked.len(), source.len(), "{at}: byte offsets moved");
        assert_eq!(
            blanked.matches('\n').count(),
            source.matches('\n').count(),
            "{at}: line numbers moved"
        );
        assert!(
            blanked
                .chars()
                .all(|read| !super::is_rustc_whitespace(read) || read.is_ascii_whitespace()),
            "{at}: the tokenizer hands its readers a separator they do not read: {blanked:?}"
        );

        let declaring =
            format!("{attempt}\n#[rustfmt::skip]\npub(super) mod{separator}rf4_child;\n");
        let declared = scan_modules(&declaring)
            .unwrap_or_else(|refusal| panic!("{at}: {ATTEMPT}: {refusal}"))
            .declared;
        assert_eq!(
            declared
                .iter()
                .filter(|declaration| declaration.name == "rf4_child")
                .count(),
            1,
            "{at}: `mod`, the separator, `rf4_child;` in {ATTEMPT} is a child rustc compiles \
             under that file's recorded allow, and the walk from the facade did not read it"
        );

        assert!(
            includes_a_file(&format!("include{separator}!(\"x.inc\");\n")),
            "{at}: `include`, the separator, `!` includes a file"
        );
        assert_eq!(
            file_level_lint_state(
                &format!("{separator}#![deny({separator}clippy::disallowed_methods)]\n"),
                "clippy::disallowed_methods"
            ),
            Some("deny"),
            "{at}: a file-level deny behind the separator is a deny"
        );
        assert!(
            matches!(
                scan_modules(&format!(
                    "#[{separator}path = \"elsewhere.rs\"]\nmod{separator}elsewhere;\n"
                )),
                Err(ScanRefusal::UnsupportedPathAttribute { .. })
            ),
            "{at}: a `path` attribute behind the separator sends the walk to the wrong file"
        );
        let allowed = governed_allows(&format!(
            "#![allow({separator}clippy::disallowed_methods)]\n"
        ));
        assert_eq!(
            allowed
                .iter()
                .flat_map(|allow| allow.lints.iter().map(String::as_str))
                .collect::<Vec<_>>(),
            vec!["disallowed_methods"],
            "{at}: an allow whose lint follows the separator is an allow of that lint"
        );
    }
}

fn what_rustc_reads_between_tokens() -> Vec<(String, String)> {
    super::RUSTC_WHITESPACE
        .iter()
        .map(|separator| {
            (
                format!("U+{:04X}", u32::from(*separator)),
                separator.to_string(),
            )
        })
        .chain(
            [
                ("a block comment", "/* c */"),
                ("a nested block comment", "/* a /* b */ c */"),
                ("a block comment holding `/**`", "/* /** */ */"),
                ("a line comment", " // c\n"),
                ("a run of separators", " \u{200E}\n\t\u{2029} "),
            ]
            .map(|(what, gap)| (what.to_owned(), gap.to_owned())),
        )
        .collect()
}

type ReadAllow = (
    bool,
    bool,
    Vec<String>,
    Vec<String>,
    Vec<&'static str>,
    bool,
);

fn allows_as_read(source: &str) -> Vec<ReadAllow> {
    governed_allows(source)
        .into_iter()
        .map(|allow| {
            (
                allow.inner,
                allow.module_level,
                allow.lints,
                allow.written,
                allow.keywords,
                allow.reasoned,
            )
        })
        .collect()
}

#[test]
fn the_placement_census_reads_an_attribute_whatever_rustc_reads_between_its_tokens() {
    type Shape = fn(&str, &str, &str, &str) -> String;
    let shapes: [(&str, Shape, ReadAllow); 6] = [
        (
            "an inner allow in the prologue",
            |h, b, k, p| {
                format!("#{h}!{b}[allow{k}(clippy{p}::{p}disallowed_methods)]\nfn go() {{}}\n")
            },
            (
                true,
                true,
                vec!["disallowed_methods".to_owned()],
                vec!["clippy::disallowed_methods".to_owned()],
                vec!["allow"],
                false,
            ),
        ),
        (
            "an outer allow on a module",
            |h, _, k, p| format!("#{h}[allow{k}(clippy{p}::{p}disallowed_types)]\nmod m {{}}\n"),
            (
                false,
                true,
                vec!["disallowed_types".to_owned()],
                vec!["clippy::disallowed_types".to_owned()],
                vec!["allow"],
                false,
            ),
        ),
        (
            "an outer expect with a reason on a statement",
            |h, _, k, p| {
                format!(
                    "fn go() {{\n    #{h}[expect{k}(clippy{p}::{p}disallowed_macros, reason = \
                     \"r\")]\n    let _ = 1;\n}}\n"
                )
            },
            (
                false,
                false,
                vec!["disallowed_macros".to_owned()],
                vec!["clippy::disallowed_macros".to_owned()],
                vec!["expect"],
                true,
            ),
        ),
        (
            "an allow a `cfg_attr` applies to a declared module",
            |h, _, k, p| {
                format!(
                    "#{h}[cfg_attr(not(test), allow{k}(clippy{p}::{p}disallowed_methods))]\n\
                     pub(crate) mod m;\n"
                )
            },
            (
                false,
                true,
                vec!["disallowed_methods".to_owned()],
                vec!["clippy::disallowed_methods".to_owned()],
                vec!["allow"],
                false,
            ),
        ),
        (
            "the second of two inner attributes",
            |h, b, k, p| {
                format!(
                    "#{h}!{b}[deny(clippy::disallowed_types)]\n\
                     #{h}!{b}[allow{k}(clippy{p}::{p}disallowed_methods)]\n"
                )
            },
            (
                true,
                true,
                vec!["disallowed_methods".to_owned()],
                vec!["clippy::disallowed_methods".to_owned()],
                vec!["allow"],
                false,
            ),
        ),
        (
            "an inner allow inside an inline module's braces",
            |h, b, k, p| {
                format!("mod m {{\n    #{h}!{b}[allow{k}(clippy{p}::{p}disallowed_methods)]\n}}\n")
            },
            (
                true,
                false,
                vec!["disallowed_methods".to_owned()],
                vec!["clippy::disallowed_methods".to_owned()],
                vec!["allow"],
                false,
            ),
        ),
    ];
    let gaps = what_rustc_reads_between_tokens();
    for (what, shape, expected) in &shapes {
        let joined = shape("", "", "", "");
        assert_eq!(
            allows_as_read(&joined),
            std::slice::from_ref(expected),
            "{what}, joined: the control reads as it always has: {joined:?}"
        );
        for (between, gap) in &gaps {
            for (position, spelled) in [
                ("after `#`", shape(gap, "", "", "")),
                ("after `!`", shape("", gap, "", "")),
                ("after the keyword", shape("", "", gap, "")),
                ("around `::`", shape("", "", "", gap)),
                ("everywhere", shape(gap, gap, gap, gap)),
            ] {
                assert_eq!(
                    allows_as_read(&spelled),
                    std::slice::from_ref(expected),
                    "{what}, {between} {position}: rustc applies this attribute exactly as it \
                     applies the joined one, and the placement census read something else: \
                     {spelled:?}"
                );
            }
        }
    }

    for (between, gap) in &gaps {
        for item in [
            format!("pub{gap}({gap}crate{gap}){gap}mod{gap}m {{}}"),
            format!("pub{gap}(in{gap}crate::a){gap}mod{gap}m;"),
            format!("pub{gap}(self){gap}mod{gap}m;"),
            format!("pub{gap}mod{gap}m;"),
            format!("mod{gap}m {{}}"),
        ] {
            let source = format!("#[allow(clippy::disallowed_methods)]{gap}{item}\n");
            assert!(
                allows_as_read(&source)
                    .iter()
                    .all(|(_, module_level, ..)| *module_level),
                "{between}: an allow on a module is module-level however the module's visibility \
                 and keyword are spaced: {source:?}"
            );
            assert_eq!(allows_as_read(&source).len(), 1, "{between}: {source:?}");
        }
    }

    for (what, source) in [
        (
            "an allow on a function",
            "#[allow(clippy::disallowed_methods)]\nfn go() {}\n",
        ),
        (
            "an item whose name begins with `mod`",
            "#[allow(clippy::disallowed_methods)]\nfn module() {}\n",
        ),
        (
            "an inner allow after the first item",
            "fn go() {}\n# ![allow(clippy::disallowed_methods)]\n",
        ),
    ] {
        assert!(
            allows_as_read(source)
                .iter()
                .all(|(_, module_level, ..)| !*module_level),
            "{what} is below module level: {source:?}"
        );
    }

    let read_as_an_allowance: Vec<(&str, Vec<ReadAllow>)> = [
        (
            "a doc comment between `#` and `[`, which rustc refuses",
            "#/** d */[allow(clippy::disallowed_methods)]\nmod m {}\n",
        ),
        (
            "an outer line doc comment between `#` and `[`",
            "#/// d\n[allow(clippy::disallowed_methods)]\nmod m {}\n",
        ),
        (
            "an inner doc comment between `!` and `[`",
            "#!/*! d */[allow(clippy::disallowed_methods)]\n",
        ),
        (
            "a doc comment between the keyword and its list",
            "#[allow/** d */(clippy::disallowed_methods)]\nmod m {}\n",
        ),
        (
            "a keyword that is only the start of a longer word",
            "#[allowed (clippy::disallowed_methods)]\nmod m {}\n",
        ),
        (
            "the attribute inside a string literal",
            "const S: &str = \"# [allow (clippy::disallowed_methods)]\";\n",
        ),
        (
            "the attribute inside a comment",
            "// # ! [allow (clippy::disallowed_methods)]\n/* # [allow(clippy::disallowed_types)] */\n",
        ),
        (
            "a `#` and a `[` with a literal between them",
            "const S: &str = stringify!(# \"x\" [allow(clippy::disallowed_methods)]);\n",
        ),
    ]
    .into_iter()
    .map(|(what, source)| (what, allows_as_read(source)))
    .filter(|(_, read)| !read.is_empty())
    .collect();
    assert!(
        read_as_an_allowance.is_empty(),
        "each is no allowance rustc applies, and the placement census read one: \
         {read_as_an_allowance:#?}"
    );
}

#[test]
fn the_placement_census_names_a_lint_as_clippy_does_and_no_further() {
    for (entry, named) in [
        ("clippy::disallowed_methods", Some("disallowed_methods")),
        ("clippy :: disallowed_methods", Some("disallowed_methods")),
        ("disallowed_types", Some("disallowed_types")),
        ("clippy::r#disallowed_methods", Some("disallowed_methods")),
        ("r#clippy::disallowed_types", Some("disallowed_types")),
        ("r#clippy::r#disallowed_macros", Some("disallowed_macros")),
        ("clippy::disallowed_method", Some("disallowed_methods")),
        ("clippy::disallowed_type", Some("disallowed_types")),
        ("clippy::r#disallowed_method", Some("disallowed_methods")),
        ("clippy_all", Some("all")),
        ("clippy_style", Some("style")),
        ("r#clippy_all", Some("all")),
        ("clippy::r#all", Some("all")),
        ("clippy::r#style", Some("style")),
        ("disallowed_method", None),
        ("clippy::clippy_all", None),
        ("clippy::DISALLOWED_METHODS", None),
        ("clippy::disallowed_methods::x", None),
        ("r# disallowed_methods", None),
        ("clippy::too_many_arguments", None),
    ] {
        assert_eq!(normalize_lint(entry), named, "`{entry}`");
        let read: Vec<String> = governed_allows(&format!("#[allow({entry})]\nmod m {{}}\n"))
            .into_iter()
            .flat_map(|allow| allow.lints)
            .collect();
        assert_eq!(
            read,
            named.map(str::to_owned).into_iter().collect::<Vec<_>>(),
            "`{entry}`: the placement census names what `normalize_lint` names"
        );
    }
}

#[test]
fn the_module_walk_reads_an_attribute_whatever_rustc_reads_between_its_tokens() {
    use crate::effects::census_domain::{ScanRefusal, scan_modules};

    const ATTEMPT: &str = "src/engine/attempt.rs";
    let attempt = fs::read_to_string(repo_root().join(ATTEMPT)).expect(ATTEMPT);
    let refuses_the_path = |source: &str| {
        matches!(
            scan_modules(source),
            Err(ScanRefusal::UnsupportedPathAttribute { .. })
        )
    };
    let test_only = |source: &str| {
        scan_modules(source).is_ok_and(|scanned| {
            scanned.declared.len() == 1
                && scanned
                    .declared
                    .iter()
                    .all(|declared| declared.name == "rf_tests" && declared.test_only)
        })
    };
    let refuses_the_inner_cfg = |source: &str| {
        matches!(
            scan_modules(source),
            Err(ScanRefusal::UnsupportedInnerCfg { .. })
        )
    };

    for (spelled, what) in [
        ("#[path = \"elsewhere.rs\"]\nmod rf_child;\n", "joined"),
        (
            "#[r#path = \"elsewhere.rs\"]\nmod rf_child;\n",
            "a raw name",
        ),
    ] {
        assert!(
            refuses_the_path(&format!("{attempt}\n{spelled}")),
            "{what}: a `path` attribute sends rustc to another file than the walk reads, in \
             {ATTEMPT}: {spelled:?}"
        );
    }
    assert!(test_only("#[cfg(test)]\nmod rf_tests;\n"), "joined");
    assert!(test_only("#[r#cfg(test)]\nmod rf_tests;\n"), "a raw name");
    assert!(
        refuses_the_inner_cfg("#![cfg(test)]\nmod rf_tests;\n"),
        "joined"
    );
    assert!(
        refuses_the_inner_cfg("#![r#cfg(test)]\nmod rf_tests;\n"),
        "a raw name"
    );

    for (between, gap) in what_rustc_reads_between_tokens() {
        for spelled in [
            format!("#{gap}[path = \"elsewhere.rs\"]\nmod rf_child;\n"),
            format!("#{gap}[{gap}path{gap}={gap}\"elsewhere.rs\"{gap}]\nmod rf_child;\n"),
            format!("#{gap}[cfg_attr(all(), path = \"elsewhere.rs\")]\nmod rf_child;\n"),
        ] {
            assert!(
                refuses_the_path(&format!("{attempt}\n{spelled}")),
                "{between}: rustc reads this `path` attribute and the walk read a plain \
                 declaration in {ATTEMPT}: {spelled:?}"
            );
        }
        let gated = format!("#{gap}[cfg{gap}(test)]\nmod{gap}rf_tests;\n");
        assert!(
            test_only(&gated),
            "{between}: the walk did not read the gate rustc applies: {gated:?}"
        );
        let inner = format!("#{gap}!{gap}[cfg(test)]\nmod rf_tests;\n");
        assert!(
            refuses_the_inner_cfg(&inner),
            "{between}: an inner `cfg` gates the module it is written in, and the walk read \
             none: {inner:?}"
        );
    }

    let read_as_an_attribute: Vec<(&str, usize)> = [
        (
            "a doc comment between `#` and `[`, which rustc refuses",
            "#/** d */[path = \"elsewhere.rs\"]\nmod rf_child;\n",
        ),
        (
            "a `path` attribute quoted in a string",
            "const S: &str = \"# [path = \\\"elsewhere.rs\\\"]\";\nmod rf_child;\n",
        ),
    ]
    .into_iter()
    .map(|(what, source)| {
        let plainly = scan_modules(source).map_or(0, |scanned| scanned.declared.len());
        (what, plainly)
    })
    .filter(|(_, plainly)| *plainly != 1)
    .collect();
    assert!(
        read_as_an_attribute.is_empty(),
        "each is no attribute rustc applies, and the walk did not read one plain \
         declaration: {read_as_an_attribute:#?}"
    );
}

#[test]
fn the_prologue_readers_read_an_inner_attribute_whatever_rustc_reads_between_its_tokens() {
    use crate::effects::lint_levels::{
        Resolution, file_level_lint_resolution, leading_inner_attributes,
    };

    const LINT: &str = "clippy::disallowed_methods";
    let allowed = Resolution {
        level: Some("allow"),
        refused_downgrade: false,
        undecided: false,
    };
    let refused = Resolution {
        level: Some("forbid"),
        refused_downgrade: true,
        undecided: false,
    };
    let joined = "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::disallowed_methods)]";
    let source = format!("{joined}\nfn go() {{}}\n");
    assert_eq!(leading_inner_attributes(&source), joined, "joined");
    assert_eq!(file_level_lint_resolution(&source, LINT), allowed, "joined");

    for (between, gap) in what_rustc_reads_between_tokens() {
        for (level, wanted) in [("deny", allowed), ("forbid", refused)] {
            let prologue = format!(
                "#{gap}!{gap}[{level}(clippy::disallowed_methods)]\n\
                 #{gap}!{gap}[allow{gap}(clippy::disallowed_methods)]"
            );
            let source = format!("{prologue}\nfn go() {{}}\n");
            assert_eq!(
                leading_inner_attributes(&source),
                prologue,
                "{between}: the prologue is both attributes"
            );
            assert_eq!(
                file_level_lint_resolution(&source, LINT),
                wanted,
                "{between}: rustc applies the allow under `{level}` as it applies the joined \
                 spelling: {source:?}"
            );
        }
    }
}

fn lint_fixture(dir: &Path, tag: &str, body: &str) -> (bool, Vec<(String, String)>) {
    let (deps, rlib) = crate_under_test();
    let source = dir.join(format!("{tag}.rs"));
    fs::write(&source, body).expect("the fixture");
    let out = dir.join(format!("{tag}-out"));
    fs::create_dir_all(&out).expect("an output directory");
    let mut command = std::process::Command::new(clippy_driver());
    command
        .env("CLIPPY_CONF_DIR", repo_root())
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "--error-format=json",
        ])
        .arg("--out-dir")
        .arg(&out)
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("upstroke={}", rlib.display()));
    for (name, path) in extern_dependencies(&deps) {
        command
            .arg("--extern")
            .arg(format!("{name}={}", path.display()));
    }
    let output = command
        .arg(&source)
        .output()
        .expect("clippy-driver runs; the lint gate uses the same binary");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut diagnostics = Vec::new();
    for line in stderr.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(code) = value
            .get("code")
            .and_then(|code| code.get("code"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if !code.starts_with("clippy::disallowed") {
            continue;
        }
        let message = value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        diagnostics.push((code.to_owned(), message));
    }
    (output.status.success(), diagnostics)
}

fn clippy_outcome(
    dir: &Path,
    tag: &str,
    source: &str,
    cfgs: &[&str],
) -> (bool, Vec<(String, String)>) {
    let file = dir.join(format!("{tag}.rs"));
    fs::write(&file, source).expect("the fixture");
    let out = dir.join("out");
    fs::create_dir_all(&out).expect("an output directory");
    let mut command = std::process::Command::new(clippy_driver());
    command
        .env("CLIPPY_CONF_DIR", repo_root())
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "--error-format=json",
        ])
        .arg("--out-dir")
        .arg(&out);
    for cfg in cfgs {
        command.arg("--cfg").arg(cfg);
    }
    let output = command
        .arg(&file)
        .output()
        .expect("clippy-driver runs; the lint gate uses the same binary");
    let mut diagnostics = Vec::new();
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(code) = value
            .get("code")
            .and_then(|code| code.get("code"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let level = value
            .get("level")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        diagnostics.push((level.to_owned(), code.to_owned()));
    }
    (output.status.success(), diagnostics)
}

fn clippy_outcomes(
    dir: &Path,
    tag: &str,
    cases: &[impl AsRef<str>],
    cfgs: &[&str],
) -> Vec<(bool, Vec<(String, String)>)> {
    let mut source = String::new();
    let mut case_lines = Vec::with_capacity(cases.len());
    for (index, case) in cases.iter().enumerate() {
        let first = source.matches('\n').count() + 1;
        let case = case.as_ref();
        source.push_str(&format!("pub mod case_{index} {{\n{case}\n}}\n"));
        case_lines.push(first..=source.matches('\n').count());
    }
    let file = dir.join(format!("{tag}.rs"));
    fs::write(&file, &source).expect("the batched fixture");
    let out = dir.join("out");
    fs::create_dir_all(&out).expect("an output directory");
    let mut command = std::process::Command::new(clippy_driver());
    command
        .env("CLIPPY_CONF_DIR", repo_root())
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "--error-format=json",
        ])
        .arg("--out-dir")
        .arg(&out);
    for cfg in cfgs {
        command.arg("--cfg").arg(cfg);
    }
    let output = command
        .arg(&file)
        .output()
        .expect("clippy-driver runs; the lint gate uses the same binary");
    let mut outcomes: Vec<(bool, Vec<(String, String)>)> = vec![(true, Vec::new()); cases.len()];
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let level = value
            .get("level")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let code = value
            .get("code")
            .and_then(|code| code.get("code"))
            .and_then(serde_json::Value::as_str);
        let primary = value
            .get("spans")
            .and_then(serde_json::Value::as_array)
            .and_then(|spans| {
                spans.iter().find(|span| {
                    span.get("is_primary").and_then(serde_json::Value::as_bool) == Some(true)
                })
            });
        let in_this_file = primary
            .filter(|span| {
                span.get("file_name")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|name| Path::new(name).file_name() == file.file_name())
            })
            .and_then(|span| span.get("line_start"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|line| usize::try_from(line).ok());
        let case =
            in_this_file.and_then(|line| case_lines.iter().position(|lines| lines.contains(&line)));
        let Some(outcome) = case.and_then(|case| outcomes.get_mut(case)) else {
            assert!(
                primary.is_none() && code.is_none(),
                "`{tag}`: clippy-driver reported {level} {code:?} at no case's lines: {line}"
            );
            continue;
        };
        if level == "error" {
            outcome.0 = false;
        }
        if let Some(code) = code {
            outcome.1.push((level.to_owned(), code.to_owned()));
        }
    }
    assert_eq!(
        output.status.success(),
        outcomes.iter().all(|(built, _)| *built),
        "`{tag}`: clippy-driver's exit status disagrees with the errors attributed to the cases, \
         so a case's outcome cannot be read from this batch: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    outcomes
}

fn clippy_driver() -> &'static Path {
    static DRIVER: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DRIVER.get_or_init(|| {
        let sysroot = std::process::Command::new("rustc")
            .arg("--print")
            .arg("sysroot")
            .output()
            .expect("rustc runs; it built this test");
        let sysroot = PathBuf::from(String::from_utf8_lossy(&sysroot.stdout).trim().to_owned());
        let name = if cfg!(windows) {
            "clippy-driver.exe"
        } else {
            "clippy-driver"
        };
        let in_sysroot = sysroot.join("bin").join(name);
        if in_sysroot.is_file() {
            return in_sysroot;
        }
        PathBuf::from(name)
    })
}

mod ci_model;
mod workflow;

use ci_model::{
    CI_TARGETS, CI_WORKFLOW, GOLDEN_IMAGE_TOOLCHAIN, MSRV_COMMAND, MSRV_JOB, OVERRIDING_REPO_FILES,
    QUEUE_LANE, RUSTFLAGS_KEY, TEST_COMMAND, TEST_WINDOWS_LABELS, TEST_WINDOWS_PLATFORM,
    TEST_WINDOWS_RUNS_ON, WINDOWS_TEST_FLOOR, WINDOWS_TEST_WITNESS,
};
use workflow::{
    WORKFLOW_ESCAPES, ci_msrv_job_complaints, ci_test_job_complaints,
    ci_test_windows_job_complaints, ci_windows_build_witness_complaints, ci_workflow_text,
    complaint_codes, declared_msrv_toolchain, declared_rust_version, field, field_names,
    mutate_workflow, parse_workflow, rustflags_complaints, scalar, steps_of, three_component,
    workflow_complaints,
};

#[test]
fn the_workflow_parser_rejects_duplicate_keys_and_reads_on_as_a_string() {
    let clean = "jobs:\n  lint:\n    runs-on: ubuntu-latest\n";
    let parsed = parse_workflow(clean).expect("the control document parses");
    assert_eq!(
        field(&parsed, "jobs")
            .and_then(|jobs| field(jobs, "lint"))
            .and_then(|lint| scalar(lint, "runs-on")),
        Some("ubuntu-latest"),
        "the control parsed but did not read back"
    );

    for (shape, document) in [
        (
            "a duplicated top-level key",
            "jobs:\n  a: 1\njobs:\n  b: 2\n",
        ),
        (
            "a duplicated key inside a job",
            "jobs:\n  lint:\n    runs-on: ubuntu-latest\n    runs-on: windows-latest\n",
        ),
    ] {
        let refused = parse_workflow(document);
        assert!(
            refused.is_err(),
            "{shape} was accepted. Last-one-wins makes every structural equality in this \
             section read the winning entry while a mutation hides in the loser."
        );
    }

    let doc = parse_workflow(&ci_workflow_text()).expect(CI_WORKFLOW);
    assert!(
        field_names(&doc).contains("on"),
        "the workflow's `on:` key did not read back as the string `on`: {:?}",
        field_names(&doc)
    );
}

#[test]
fn the_workflow_shape_oracle_refuses_every_escape_the_ledger_names() {
    let text = ci_workflow_text();

    let doc = parse_workflow(&text).expect(CI_WORKFLOW);
    let clean = workflow_complaints(&doc);
    assert!(
        clean.is_empty(),
        "the unmutated workflow does not satisfy its own contract:\n{}",
        clean.join("\n")
    );

    let mut refused: BTreeSet<&str> = BTreeSet::new();
    for escape in WORKFLOW_ESCAPES {
        let mutated = mutate_workflow(&text, escape.job, escape.anchor, escape.replacement);
        assert_ne!(
            mutated, text,
            "{}: the mutation changed nothing, so it measures nothing",
            escape.name
        );
        let complaints = match parse_workflow(&mutated) {
            Ok(document) => workflow_complaints(&document),
            Err(error) => vec![error],
        };
        let codes = complaint_codes(&complaints);
        assert!(
            codes.contains(escape.refused_as),
            "{} was not refused as `{}` -- {}\nComplaints: {:#?}",
            escape.name,
            escape.refused_as,
            escape.escape,
            complaints
        );
        refused.insert(escape.name);
    }
    assert_eq!(
        refused.len(),
        WORKFLOW_ESCAPES.len(),
        "two escapes share a name, so one of them was never measured"
    );
}

#[test]
fn the_workflow_that_runs_these_tests_installs_the_compiler_they_need() {
    let doc = parse_workflow(&ci_workflow_text()).expect(CI_WORKFLOW);
    let complaints = ci_test_job_complaints(&doc);
    assert!(
        complaints.is_empty(),
        "the `test` job does not run these fixtures the way they need:\n{}",
        complaints.join("\n")
    );
}

#[test]
fn the_windows_leg_runs_these_fixtures_on_the_runner_each_lane_pins() {
    let doc = parse_workflow(&ci_workflow_text()).expect(CI_WORKFLOW);
    let complaints = ci_test_windows_job_complaints(&doc);
    assert!(
        complaints.is_empty(),
        "the Windows leg does not run these fixtures the way the contract pins:\n{}",
        complaints.join("\n")
    );
}

#[test]
fn the_windows_leg_routes_each_lane_to_the_runner_its_install_step_is_written_for() {
    let labels = TEST_WINDOWS_LABELS
        .iter()
        .map(|label| format!("\"{label}\""))
        .collect::<Vec<_>>()
        .join(", ");
    assert_eq!(
        TEST_WINDOWS_RUNS_ON,
        format!("${{{{ {QUEUE_LANE} && '{TEST_WINDOWS_PLATFORM}' || fromJSON('[{labels}]') }}}}"),
        "the pinned `runs-on:` is not the lane test, the hosted platform and the self-hosted \
         labels this contract names, so the install step's `if:` and the runner it is written \
         for can disagree while both pins hold"
    );
    assert!(
        CI_TARGETS
            .iter()
            .any(|target| target.runner == TEST_WINDOWS_PLATFORM),
        "the hosted lane's runner `{TEST_WINDOWS_PLATFORM}` is not one the cfg census models, \
         so what its compilations set is not decided here"
    );
}

#[test]
fn the_hosted_windows_leg_still_links_every_test_binary() {
    let doc = parse_workflow(&ci_workflow_text()).expect(CI_WORKFLOW);
    let complaints = ci_windows_build_witness_complaints(&doc);
    assert!(
        complaints.is_empty(),
        "no hosted leg code-generates and links the Windows tree the way the contract pins:\n{}",
        complaints.join("\n")
    );
}

#[test]
fn no_repository_file_overrides_what_ci_compiles_or_runs() {
    let root = repo_root();
    let present: Vec<&str> = OVERRIDING_REPO_FILES
        .iter()
        .copied()
        .filter(|name| root.join(name).exists())
        .collect();
    assert!(
        present.is_empty(),
        "these files outrank `{CI_WORKFLOW}` and this contract reads only the workflow: \
         {present:?}. A toolchain file replaces the compiler every leg runs; a Cargo config \
         can bind a target runner that reports success without executing a test binary. \
         Adding one is a deliberate act: extend this contract in the same change."
    );
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml"))
            .expect("Cargo.toml parses");
    assert!(
        manifest.get("workspace").is_none(),
        "Cargo.toml declares a workspace, so `--all-targets --all-features` no longer selects \
         this crate: `default-members` decides, and a member with no tests makes every CI \
         command succeed without running this suite."
    );
}

#[test]
fn the_windows_leg_counts_the_tests_it_ran() {
    let cargo_lines = WINDOWS_TEST_WITNESS
        .lines()
        .filter(|line| line.starts_with(TEST_COMMAND))
        .count();
    assert_eq!(
        cargo_lines, 1,
        "the Windows leg's step does not run `{TEST_COMMAND}` on exactly one line, so the \
         suite it witnesses is not the suite the other legs run"
    );
    let version = GOLDEN_IMAGE_TOOLCHAIN.replace('.', "\\.");
    for (variable, tool, asked) in [
        ("rustc", "rustc", "the compiler Cargo will run"),
        ("path_rustc", "rustc", "the `rustc` on the step's PATH"),
        ("cargo", "cargo", "the `cargo` on PATH"),
    ] {
        assert!(
            WINDOWS_TEST_WITNESS.contains(&format!("${variable} -notmatch '^{tool} {version} '")),
            "the Windows leg's step does not refuse {asked} being other than the image's \
             `{GOLDEN_IMAGE_TOOLCHAIN}` before running the suite, so the toolchain the install \
             step pins and the one the suite runs on can differ with every pin matching"
        );
    }
    assert!(
        WINDOWS_TEST_WITNESS.find("-notmatch") < WINDOWS_TEST_WITNESS.find(TEST_COMMAND),
        "the compiler check comes after the suite, so the suite has already run on the wrong \
         compiler by the time the step refuses"
    );
    assert!(
        WINDOWS_TEST_WITNESS.contains(&format!("-lt {WINDOWS_TEST_FLOOR}")),
        "the Windows leg's step does not test the count against \
         {WINDOWS_TEST_FLOOR}, so the floor this contract documents is not the floor it runs"
    );
}

#[test]
fn the_msrv_leg_checks_the_floor_the_manifest_publishes_on_every_platform() {
    assert_eq!(three_component("1.85"), "1.85.0");
    assert_eq!(three_component("1.85.0"), "1.85.0");
    assert_eq!(
        three_component("nightly"),
        "nightly",
        "a manifest value this does not understand must reach the equality below unchanged \
         and fail there with both strings quoted, not be normalised into agreement"
    );

    let doc = parse_workflow(&ci_workflow_text()).expect(CI_WORKFLOW);
    let complaints = ci_msrv_job_complaints(&doc);
    assert!(
        complaints.is_empty(),
        "the `{MSRV_JOB}` job does not check the floor the way the documents publish it:\n{}",
        complaints.join("\n")
    );

    let installed: Vec<&str> = field(&doc, "jobs")
        .and_then(|jobs| field(jobs, MSRV_JOB))
        .map(steps_of)
        .unwrap_or_default()
        .iter()
        .filter_map(|step| field(step, "with").and_then(|with| scalar(with, "toolchain")))
        .collect();
    let expected = declared_msrv_toolchain();
    assert_eq!(
        installed,
        vec![expected.as_str()],
        "`Cargo.toml` publishes `rust-version = \"{}\"`, whose toolchain name is \
         `{expected}`; the `{MSRV_JOB}` leg installs {installed:?}",
        declared_rust_version()
    );

    let steps = field(&doc, "jobs")
        .and_then(|jobs| field(jobs, MSRV_JOB))
        .map(steps_of)
        .unwrap_or_default();
    let install_at = steps.iter().position(|step| {
        scalar(step, "uses").is_some_and(|uses| uses.starts_with("dtolnay/rust-toolchain@"))
            && field(step, "with").and_then(|with| scalar(with, "toolchain"))
                == Some(expected.as_str())
    });
    let check_at = steps
        .iter()
        .position(|step| scalar(step, "run") == Some(MSRV_COMMAND));
    assert!(
        matches!((install_at, check_at), (Some(install), Some(check)) if install < check),
        "the `{MSRV_JOB}` leg installs toolchain `{expected}` at step {install_at:?} and runs \
         `{MSRV_COMMAND}` at step {check_at:?}. The install has to come first: it selects the \
         toolchain for the steps that follow it, and a check above it compiles on whatever \
         the runner image shipped."
    );
}

#[test]
fn the_workflow_scope_rustflags_pin_refuses_weakening_and_every_override() {
    fn probe(header: &str, job_body: &str) -> String {
        format!("{header}jobs:\n  probe:\n{job_body}")
    }
    const PLAIN: &str = "    runs-on: ubuntu-latest\n    steps:\n      - run: cargo check\n";
    const PINNED: &str = "env:\n  RUSTFLAGS: -D warnings\n";

    let doc = parse_workflow(&ci_workflow_text()).expect(CI_WORKFLOW);
    let live = rustflags_complaints(&doc);
    assert!(
        live.is_empty(),
        "the real workflow does not satisfy the `{RUSTFLAGS_KEY}` contract:\n{}",
        live.join("\n")
    );

    let control = parse_workflow(&probe(PINNED, PLAIN)).expect("the control document parses");
    let refused_control = rustflags_complaints(&control);
    assert!(
        refused_control.is_empty(),
        "the conforming probe is refused, so no refusal below is evidence of anything:\n{}",
        refused_control.join("\n")
    );

    for (shape, document, code) in [
        ("no workflow `env:` at all", probe("", PLAIN), "rustflags"),
        (
            "an `env:` that binds other names but not this one",
            probe("env:\n  CARGO_TERM_COLOR: always\n", PLAIN),
            "rustflags",
        ),
        (
            "warnings allowed instead of denied",
            probe("env:\n  RUSTFLAGS: -A warnings\n", PLAIN),
            "rustflags",
        ),
        (
            "an allow appended after the deny, which every `contains` reading accepts",
            probe(
                "env:\n  RUSTFLAGS: -D warnings -A clippy::disallowed_methods\n",
                PLAIN,
            ),
            "rustflags",
        ),
        (
            "a value YAML does not read as a string",
            probe("env:\n  RUSTFLAGS: true\n", PLAIN),
            "rustflags",
        ),
        (
            "the encoded form at workflow scope, which Cargo reads first",
            probe(
                "env:\n  RUSTFLAGS: -D warnings\n  CARGO_ENCODED_RUSTFLAGS: ''\n",
                PLAIN,
            ),
            "rustflags",
        ),
        (
            "a job-level rebinding",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    env:\n      RUSTFLAGS: -A warnings\n    \
                 steps:\n      - run: cargo check\n",
            ),
            "rustflags-override",
        ),
        (
            "a job-level binding of the name Cargo prefers",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    env:\n      CARGO_ENCODED_RUSTFLAGS: ''\n    \
                 steps:\n      - run: cargo check\n",
            ),
            "rustflags-override",
        ),
        (
            "a step-level rebinding",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    steps:\n      - run: cargo check\n        \
                 env:\n          RUSTFLAGS: -A warnings\n",
            ),
            "rustflags-override",
        ),
        (
            "a step-level binding of the name Cargo prefers",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    steps:\n      - run: cargo check\n        \
                 env:\n          CARGO_ENCODED_RUSTFLAGS: ''\n",
            ),
            "rustflags-override",
        ),
        (
            "a job-level rebinding in lower case, which is `RUSTFLAGS` on Windows",
            probe(
                PINNED,
                "    runs-on: windows-latest\n    env:\n      rustflags: -A warnings\n    \
                 steps:\n      - run: cargo check\n",
            ),
            "rustflags-override",
        ),
        (
            "a step-level rebinding in mixed case",
            probe(
                PINNED,
                "    runs-on: windows-latest\n    steps:\n      - run: cargo check\n        \
                 env:\n          RustFlags: -A warnings\n",
            ),
            "rustflags-override",
        ),
        (
            "a case variant beside the pinned line at workflow scope",
            probe(
                "env:\n  RUSTFLAGS: -D warnings\n  Rustflags: -A warnings\n",
                PLAIN,
            ),
            "rustflags",
        ),
        (
            "the encoded name in lower case at workflow scope",
            probe(
                "env:\n  RUSTFLAGS: -D warnings\n  cargo_encoded_rustflags: ''\n",
                PLAIN,
            ),
            "rustflags",
        ),
        (
            "a bash write to the job environment file",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    steps:\n      - run: echo \
                 \"RUSTFLAGS=-A warnings\" >> \"$GITHUB_ENV\"\n",
            ),
            "rustflags-persisted",
        ),
        (
            "the same write through the `github.env` expression",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    steps:\n      - run: echo \
                 \"CARGO_ENCODED_RUSTFLAGS=\" >> ${{ github.env }}\n",
            ),
            "rustflags-persisted",
        ),
        (
            "the PowerShell form, which shares no syntax with the bash one",
            probe(
                PINNED,
                "    runs-on: windows-latest\n    steps:\n      - run: Add-Content -Path \
                 $env:GITHUB_ENV -Value \"RUSTFLAGS=-A warnings\"\n",
            ),
            "rustflags-persisted",
        ),
        (
            "the cmd form, where the file is reached as a percent variable",
            probe(
                PINNED,
                "    runs-on: windows-latest\n    steps:\n      - run: echo \
                 RUSTFLAGS=-A warnings>>%GITHUB_ENV%\n        shell: cmd\n",
            ),
            "rustflags-persisted",
        ),
        (
            "a heredoc, with the name and the redirection on different lines",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    steps:\n      - run: |\n          cat >> \
                 \"$GITHUB_ENV\" <<'EOF'\n          RUSTFLAGS=-A warnings\n          EOF\n",
            ),
            "rustflags-persisted",
        ),
        (
            "flags scoped to one command, with no env file in sight",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    steps:\n      - run: RUSTFLAGS=-A warnings \
                 cargo build\n",
            ),
            "rustflags-in-script",
        ),
    ] {
        let parsed = parse_workflow(&document).expect(shape);
        let complaints = rustflags_complaints(&parsed);
        let codes = complaint_codes(&complaints);
        assert!(
            codes.contains(code),
            "{shape} was not refused as `{code}`. Document:\n{document}\nComplaints: {:#?}",
            complaints
        );
    }

    for (shape, document) in [
        (
            "an unrelated variable whose name contains the guarded one",
            probe(
                "env:\n  RUSTFLAGS: -D warnings\n  RUSTFLAGS_EXTRA: -C debuginfo=0\n",
                PLAIN,
            ),
        ),
        (
            "an unrelated variable the guarded one is a prefix of, at job scope",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    env:\n      MY_RUSTFLAGS: -A warnings\n      \
                 RUST_FLAGS: -A warnings\n    steps:\n      - run: cargo check\n",
            ),
        ),
        (
            "a script that writes the env file without touching the policy",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    steps:\n      - run: echo \
                 \"CARGO_TERM_COLOR=never\" >> \"$GITHUB_ENV\"\n",
            ),
        ),
        (
            "a script naming a variable the guarded one is only a prefix of",
            probe(
                PINNED,
                "    runs-on: ubuntu-latest\n    steps:\n      - run: echo \
                 \"RUSTFLAGS_EXTRA=1\" >> \"$GITHUB_ENV\"\n",
            ),
        ),
    ] {
        let parsed = parse_workflow(&document).expect(shape);
        let complaints = rustflags_complaints(&parsed);
        assert!(
            complaints.is_empty(),
            "{shape} was refused, so this scan reads substrings rather than whole names. \
             Document:\n{document}\nComplaints: {complaints:#?}"
        );
    }
}

pub(crate) mod cfg;

use cfg::{
    CFG_CENSUS_CONTROL, CFG_ESCAPES, CFG_GATE_FLOOR, CONTROL_GATES, CfgForm, CfgSite,
    NO_CI_RUNNER_COMPILES, WHOLE_FILE_TEST_MODULES, cfg_regions, compiled_by, parse_cfg,
};

#[test]
fn the_cfg_census_evaluates_effective_predicates_against_the_valuations_ci_sets() {
    for (text, expected, why) in CFG_ESCAPES {
        let pred = parse_cfg(text, false).unwrap_or_else(|error| {
            panic!("the census cannot read `cfg({text})`, which it must: {error}")
        });
        assert_eq!(
            pred.render(),
            text,
            "`cfg({text})` did not round-trip through the parser"
        );
        let expected: BTreeSet<&str> = expected.iter().copied().collect();
        let compiled = compiled_by(&pred)
            .unwrap_or_else(|error| panic!("`cfg({text})` is undecidable: {error}"));
        assert_eq!(compiled, expected, "`cfg({text})` -- {why}");
    }

    let unmodelled = parse_cfg("feature = \"unshipped\"", false).expect("a parseable predicate");
    let refused = compiled_by(&unmodelled);
    assert!(
        refused.is_err(),
        "a cfg key no valuation models was decided anyway, as {refused:?}"
    );

    let mut domain = scanned_sources();
    let real = domain.len();
    let fixture = "fixtures/cfg-census-control.rs";
    domain.push((fixture.to_owned(), CFG_CENSUS_CONTROL.to_owned()));
    let (sites, unreadable) = cfg_regions(&domain);
    assert!(
        unreadable.is_empty(),
        "the census could not read {} occurrence(s):\n{}",
        unreadable.len(),
        unreadable.join("\n")
    );
    let gates: Vec<&CfgSite> = sites
        .iter()
        .filter(|site| site.form == CfgForm::Gate)
        .collect();
    assert!(
        real > 30 && gates.len() > CFG_GATE_FLOOR,
        "the control was scanned inside a truncated domain: {real} files, {} gates",
        gates.len()
    );

    let injected: Vec<&CfgSite> = sites.iter().filter(|site| site.path == fixture).collect();
    let rendered: Vec<&str> = injected
        .iter()
        .filter(|site| site.form == CfgForm::Gate)
        .map(|site| site.rendered.as_str())
        .collect();
    assert_eq!(
        rendered, CONTROL_GATES,
        "the control fixture produced the wrong gates. `haiku` or `plan9` among them is a \
         non-gating form counted as a gate; a missing `all(...)` is a stacked attribute or a \
         module guard the scan did not conjoin; `android` is a `let` binding read as a \
         predicate, and a `cfg(` from `fn cfg(bits: u32)` is a parameter list read as one."
    );

    let by_form: BTreeMap<CfgForm, Vec<&str>> =
        injected
            .iter()
            .fold(BTreeMap::new(), |mut acc: BTreeMap<_, Vec<&str>>, site| {
                acc.entry(site.form)
                    .or_default()
                    .push(site.written.as_str());
                acc
            });
    assert_eq!(
        by_form.get(&CfgForm::Attribute).map(Vec::as_slice),
        Some(["target_os = \"haiku\""].as_slice()),
        "`#[cfg_attr(P, attr)]` applies an attribute conditionally; the item is compiled \
         everywhere and it is not a platform demand"
    );
    assert_eq!(
        by_form.get(&CfgForm::Macro).map(Vec::as_slice),
        Some(["target_os = \"plan9\""].as_slice()),
        "`cfg!(P)` is an expression: both arms around it compile on every platform"
    );

    let stacked = injected
        .iter()
        .find(|site| site.rendered == "all(unix, target_os = \"macos\")")
        .expect("the stacked control");
    assert_eq!(
        stacked.written, "all(unix, target_os = \"macos\")",
        "stacked `#[cfg]`s are one item's predicate, not two items'"
    );
    let nested = injected
        .iter()
        .find(|site| site.rendered == "all(windows, test)")
        .expect("the nested control");
    assert_eq!(
        nested.written, "test",
        "the nested item writes only `test`; `windows` comes from the module around it"
    );
    assert_eq!(
        compiled_by(&nested.pred).expect("decidable"),
        BTreeSet::from(["windows-latest"]),
        "an item inside a `#[cfg(windows)] mod` is not compiled by the Linux leg, whatever \
         its own attribute says"
    );
}

#[test]
fn every_platform_this_crate_configures_for_has_a_clippy_gate_the_aggregate_requires() {
    let sources = scanned_sources();
    let (sites, unreadable) = cfg_regions(&sources);
    assert!(
        unreadable.is_empty(),
        "the census could not read {} occurrence(s):\n{}",
        unreadable.len(),
        unreadable.join("\n")
    );
    let gates: Vec<&CfgSite> = sites
        .iter()
        .filter(|site| site.form == CfgForm::Gate)
        .collect();
    assert!(
        gates.len() > CFG_GATE_FLOOR,
        "only {} gating cfg attribute(s) found across {} files; the census is reading the \
         wrong shape",
        gates.len(),
        sources.len()
    );
    assert!(
        gates
            .iter()
            .any(|site| site.written == "not(any(target_os = \"linux\", target_os = \"macos\"))"),
        "the census did not find the nested negated predicate this tree is known to carry, \
         so it is reading a narrower grammar than the tree uses"
    );
    let under_a_file_guard: BTreeSet<&str> = gates
        .iter()
        .filter(|site| site.rendered.starts_with("all(test,") || site.rendered == "test")
        .map(|site| site.path.as_str())
        .collect();
    assert!(
        under_a_file_guard.len() >= WHOLE_FILE_TEST_MODULES.len(),
        "only {} file(s) carry a `test` guard the census resolved, and \
         `the_whole_file_test_modules_are_resolved_from_the_declarations_not_the_file_names` \
         resolves {} whole-file test modules on its own",
        under_a_file_guard.len(),
        WHOLE_FILE_TEST_MODULES.len()
    );

    let mut uncovered: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for site in &gates {
        let compiled = compiled_by(&site.pred).unwrap_or_else(|error| {
            panic!(
                "{}:{}: `cfg({})` cannot be decided: {error}",
                site.path, site.line, site.rendered
            )
        });
        if compiled.is_empty() {
            uncovered
                .entry(site.rendered.as_str())
                .or_default()
                .push(format!("{}:{}", site.path, site.line));
        }
    }
    let acknowledged: BTreeSet<&str> = NO_CI_RUNNER_COMPILES
        .iter()
        .map(|(pred, _)| *pred)
        .collect();
    let found: BTreeSet<&str> = uncovered.keys().copied().collect();
    assert_eq!(
        found, acknowledged,
        "the set of effective predicates no CI runner compiles moved. Every such body is \
         outside the effect denylist's reach on every job CI runs: add the platform's Clippy \
         leg, or add a row to `NO_CI_RUNNER_COMPILES` saying why the body is unreachable on \
         purpose.\n{uncovered:#?}"
    );

    for target in &CI_TARGETS {
        let only = BTreeSet::from([target.runner]);
        let witness = gates
            .iter()
            .find(|site| compiled_by(&site.pred).is_ok_and(|compiled| compiled == only));
        assert!(
            witness.is_some(),
            "no body in this tree is compiled by `{}` alone, so nothing here establishes \
             that its Clippy leg is needed",
            target.runner
        );
    }

    let doc = parse_workflow(&ci_workflow_text()).expect(CI_WORKFLOW);
    let complaints = workflow_complaints(&doc);
    assert!(
        complaints.is_empty(),
        "{CI_WORKFLOW} does not wire the gates its own cfg census requires:\n{}",
        complaints.join("\n\n")
    );
}

fn crate_under_test() -> (PathBuf, PathBuf) {
    let exe = std::env::current_exe().expect("the test executable");
    let deps = exe
        .parent()
        .expect("the test executable is in a directory")
        .to_path_buf();
    let mut rlibs: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(&deps)
        .expect("the deps directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_str()?;
            (name.starts_with("libupstroke-") && name.ends_with(".rlib")).then(|| {
                let stamp = path
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                (stamp, path)
            })
        })
        .collect();
    rlibs.sort();
    let rlib = rlibs
        .pop()
        .unwrap_or_else(|| {
            panic!(
                "no libupstroke-*.rlib beside the test executable in {}",
                deps.display()
            )
        })
        .1;
    (deps, rlib)
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("upstroke-effects-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

#[test]
fn every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified() {
    checks::reachable_fns_are_classified();
}

#[test]
fn every_name_more_than_one_callable_bears_is_pinned_by_its_count() {
    checks::shared_names_are_pinned();
}

#[test]
fn a_second_callable_under_a_classified_name_is_refused_and_a_renamed_one_is_unclassified() {
    checks::the_collision_witness_and_its_renamed_control();
}

#[test]
fn a_fn_behind_any_separator_is_unclassified_and_a_legal_escape_hides_no_brace() {
    checks::the_separator_and_escape_witnesses_and_their_controls();
}

#[test]
fn an_effectful_name_is_shared_only_by_bearers_of_one_path() {
    checks::the_shared_effectful_pin_witness_and_its_controls();
}

#[test]
fn a_shared_effectful_name_is_placed_by_its_braces_and_not_by_what_a_header_spells() {
    checks::the_spelling_defeats_and_their_controls();
}

#[test]
fn the_owner_reading_places_each_header_or_leaves_it_unread() {
    checks::the_owner_reading_places_each_header_or_leaves_it_unread();
}

#[test]
fn every_effectful_wrapper_is_on_the_disallowed_list() {
    checks::effectful_wrappers_are_denied();
}

#[test]
fn only_the_binary_crate_root_leaves_its_crate_path_empty() {
    checks::crate_paths_name_the_modules();
}

#[test]
fn every_funnel_classified_fn_names_a_site() {
    checks::funnel_rows_name_a_site();
}

#[test]
fn every_libc_item_the_tree_names_is_classified_and_the_effects_are_denied() {
    checks::libc_items_are_classified_and_denied();
}

mod artifacts;

use artifacts::{
    SAMPLING_N, SITES_WITHOUT_A_FUNNEL, artifact_content, funnel_module, funnel_module_record,
    residue_record,
};

#[test]
fn the_checked_in_effect_sites_json_is_what_the_enums_generate() {
    let generated = format!(
        "{}\n",
        effect_sites_json().expect("the inventory serializes")
    );
    let path = repo_root().join(EFFECT_SITES_JSON);
    if std::env::var_os(REGENERATE).is_some() {
        fs::write(&path, &generated).expect("write the inventory");
    }
    let on_disk = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{EFFECT_SITES_JSON} is missing; run with {REGENERATE}=1"));
    let on_disk = artifact_content(&on_disk);
    assert_eq!(
        on_disk, generated,
        "{EFFECT_SITES_JSON} is stale; regenerate with {REGENERATE}=1"
    );
    assert_eq!(effect_sites().len(), EffectSiteId::all().len());
    assert!(on_disk.contains("\"site\": \"Event.OpenLog\""));
    assert!(on_disk.contains("\"site\": \"Object.CandidateCommitTree\""));
}

#[test]
fn the_checked_in_funnel_module_record_states_where_the_bodies_are() {
    let generated = funnel_module_record();
    let path = repo_root().join(FUNNEL_MODULES_JSON);
    if std::env::var_os(REGENERATE).is_some() {
        fs::write(&path, &generated).expect("write the funnel-module record");
    }
    let on_disk =
        artifact_content(&fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!("{FUNNEL_MODULES_JSON} is missing; run with {REGENERATE}=1")
        }));
    assert_eq!(
        on_disk, generated,
        "{FUNNEL_MODULES_JSON} is stale; regenerate with {REGENERATE}=1"
    );

    let parsed: serde_json::Value = serde_json::from_str(&on_disk).expect("the record parses");
    assert_eq!(
        parsed["sites_checked"].as_u64().expect("a count"),
        EffectSiteId::all().len() as u64,
        "the record must cover the whole inventory; a record over a corner of it          would report agreement it never looked for"
    );
    let disagreements: Vec<&str> = parsed["disagreements"]
        .as_array()
        .expect("an array")
        .iter()
        .map(|entry| entry["site"].as_str().expect("a site name"))
        .collect();
    assert_eq!(
        disagreements,
        [
            "Answer.StageWrite",
            "Answer.PublishRename",
            "Answer.Ingest",
            "Report.Write"
        ],
        "the set of sites whose funnel bodies are not where the inventory says          moved. Each one is a claim a gate report carries about this tree."
    );
    for entry in parsed["disagreements"].as_array().expect("an array") {
        let inventory_module = if entry["group"] == "Report" {
            "src/util.rs"
        } else {
            "src/interaction.rs"
        };
        assert_eq!(entry["inventory_module"], inventory_module);
        assert_eq!(entry["funnel_module"], "src/rundir.rs");
    }
}

#[test]
fn every_site_the_inventory_declares_has_a_funnel_that_names_it_or_is_recorded_absent() {
    let list = allowlist();
    let funnel: BTreeMap<&str, &AllowlistEntry> = list
        .funnel
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();

    let mut modules: BTreeSet<String> = BTreeSet::new();
    for site in EffectSiteId::all() {
        modules.insert(site.module().to_owned());
    }
    assert_eq!(modules.len(), 7, "{modules:?}");
    for module in &modules {
        assert!(
            funnel.contains_key(module.as_str()),
            "`{module}` is a funnel module the inventory names and the allowlist's \
             funnel section does not list it"
        );
    }

    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    let mut unimplemented = Vec::new();
    let mut mechanisms: BTreeMap<&str, &str> = BTreeMap::new();
    for site in EffectSiteId::all() {
        let group = site.group().name();
        let module = funnel_module(site);
        let entry = funnel[module];
        if entry.absent {
            unimplemented.push(site.name());
            continue;
        }
        let source = sources.entry(module.to_owned()).or_insert_with(|| {
            let text = fs::read_to_string(repo_root().join(module)).expect("read funnel module");
            blank_comments_and_strings(&production_region(&text))
        });
        let variant = format!("{group}Site::{}", site.variant());
        let parameter = format!(": {group}Site");
        if source.contains(&variant) {
            mechanisms.insert(group, "variant");
        } else if source.contains(&parameter) {
            mechanisms.insert(group, "parameter");
        } else {
            unimplemented.push(site.name());
        }
    }
    let distinct: BTreeSet<&str> = mechanisms.values().copied().collect();
    assert_eq!(distinct.len(), 2, "{mechanisms:?}");

    let expected: BTreeSet<String> = SITES_WITHOUT_A_FUNNEL
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let actual: BTreeSet<String> = unimplemented.into_iter().collect();
    assert_eq!(
        actual, expected,
        "the set of sites no funnel names moved. Each one is a row of the site \
         inventory in reconciliation-D.md and needs a reason."
    );
}

#[test]
fn no_production_api_exports_a_writable_process_command() {
    fn command_returning_public_signatures(source: &str) -> Vec<String> {
        let code = blank_comments_and_strings(&production_region(source));
        let mut found = Vec::new();
        for (at, _) in code.match_indices("pub") {
            let before_ok = at == 0
                || !(code.as_bytes()[at - 1].is_ascii_alphanumeric()
                    || code.as_bytes()[at - 1] == b'_');
            let tail = &code[at..];
            if !before_ok {
                continue;
            }
            let after_pub = tail["pub".len()..].trim_start();
            let item = if let Some(restricted) = after_pub.strip_prefix('(') {
                let Some(close) = restricted.find(')') else {
                    continue;
                };
                if restricted[..close].trim() != "crate" {
                    continue;
                }
                restricted[close + 1..].trim_start()
            } else {
                after_pub
            };
            if !item.starts_with("fn ") {
                continue;
            }
            let end = tail.find(['{', ';']).unwrap_or(tail.len());
            let signature = &tail[..end];
            let Some(arrow) = signature.find("->") else {
                continue;
            };
            let returns = &signature[arrow + 2..];
            let names_command = returns
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .any(|token| token == "Command");
            if names_command {
                found.push(signature.split_whitespace().collect::<Vec<_>>().join(" "));
            }
        }
        found
    }

    let mut escapes = Vec::new();
    for (path, source) in scanned_sources() {
        for signature in command_returning_public_signatures(&source) {
            escapes.push(format!("{path}: {signature}"));
        }
    }
    assert!(escapes.is_empty(), "writable Command escapes: {escapes:#?}");

    assert_eq!(
        command_returning_public_signatures(
            "pub fn renamed() -> std::process::Command { todo!() }\n\
             pub ( crate )\n fn pointer() -> fn() -> Command { todo!() }\n\
             fn private() -> Command { todo!() }\n\
             pub fn consumes(_: Command) -> ProcessOutput { todo!() }"
        )
        .len(),
        2,
        "the structural control must catch direct and function-pointer returns only"
    );
}

#[test]
fn the_checked_in_residue_class_record_is_what_the_enums_generate() {
    let generated = residue_record();
    let path = repo_root().join(RESIDUE_CLASSES_JSON);
    if std::env::var_os(REGENERATE).is_some() {
        fs::write(&path, &generated).expect("write the residue record");
    }
    let on_disk = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{RESIDUE_CLASSES_JSON} is missing; run with {REGENERATE}=1"));
    let on_disk = artifact_content(&on_disk);
    assert_eq!(
        on_disk, generated,
        "{RESIDUE_CLASSES_JSON} is stale; regenerate with {REGENERATE}=1"
    );

    let harness = fs::read_to_string(repo_root().join("src/workspace_manager/tests.rs"))
        .expect("src/workspace_manager/tests.rs");
    assert!(
        harness.contains(&format!("const SAMPLING_N: u32 = {SAMPLING_N};")),
        "the sampling harness no longer runs N = {SAMPLING_N}"
    );
    assert!(on_disk.contains(&format!("\"sampling_n\": {SAMPLING_N}")));
}

#[test]
fn every_file_durability_barrier_in_a_funnel_module_goes_through_one_call() {
    const BARRIERS: &[(&str, &str, usize)] = &[
        ("src/util.rs", "fsync_file", 1),
        ("src/util.rs", "fsync_dir", 1),
    ];
    let util = artifact_content(
        &fs::read_to_string(repo_root().join("src/util.rs")).expect("src/util.rs"),
    );
    for (file, function, expected) in BARRIERS {
        let body = util
            .split_once(&format!("fn {function}("))
            .unwrap_or_else(|| panic!("{file} no longer defines `{function}`"))
            .1;
        let body = &body[..body.find("\n}\n").expect("the function ends")];
        let calls = body.matches(".sync_all()").count();
        assert_eq!(
            calls, *expected,
            "`{function}` makes {calls} durability syscall(s), not {expected}; deleting \
             the barrier from inside it is exactly PR5-CONF-012's surviving mutation"
        );
    }

    const FUNNELS: &[&str] = &[
        "src/rundir.rs",
        "src/workspace_manager.rs",
        "src/events/log.rs",
        "src/runner/container.rs",
    ];
    for path in FUNNELS {
        let source =
            fs::read_to_string(repo_root().join(path)).unwrap_or_else(|_| panic!("{path}"));
        let production = blank_comments_and_strings(&production_region(&source));
        assert_eq!(
            production.matches(".sync_all()").count(),
            0,
            "{path} calls `sync_all` directly; the file barrier is `util::fsync_file` \
             and the directory barrier is `util::fsync_dir`"
        );
    }

    let log = fs::read_to_string(repo_root().join("src/events/log.rs")).expect("src/events/log.rs");
    assert_eq!(
        blank_comments_and_strings(&production_region(&log))
            .matches(".sync_data()")
            .count(),
        1,
        "the log's own barrier is one `sync_data`; \
         `events::log::tests::the_event_log_is_written_in_exactly_one_module` \
         is the census that owns it"
    );
}

mod source_oracles;

use source_oracles::oracles;

#[test]
fn no_site_enums_row_mapping_has_a_wildcard_arm() {
    oracles::site_row_mappings_have_no_wildcard_arm();
}

#[test]
fn the_row_mapping_census_reads_the_declared_production_module() {
    oracles::the_row_mapping_census_domain_is_the_declared_module();
}

#[test]
fn no_topology_module_calls_a_funnel_in_production() {
    oracles::topology_production_names_no_funnel();
}

#[test]
fn the_reachable_fn_parser_finds_each_shape_this_tree_uses() {
    oracles::the_reachable_fn_parser_finds_every_shape();
}

#[test]
fn a_configured_item_above_a_production_fn_does_not_hide_it_from_the_domain() {
    oracles::the_domain_reaches_past_a_configured_item();
}

#[test]
fn every_classified_module_that_declares_a_visible_fn_has_a_domain() {
    oracles::every_classified_module_that_declares_a_visible_fn_has_a_domain();
}

#[test]
fn the_comment_blanker_models_raw_strings_and_still_blanks_comments() {
    oracles::the_comment_blanker_models_raw_strings();
}

#[test]
fn the_two_blankers_each_carry_their_own_contract_in_the_notes() {
    oracles::the_notes_give_each_blanker_its_own_contract();
}

#[test]
fn a_multi_byte_char_literal_does_not_desync_the_blanker() {
    oracles::a_multi_byte_char_literal_keeps_the_blankers_phase();
}

#[test]
fn a_region_that_cannot_find_an_items_end_blanks_the_attribute_not_the_file() {
    oracles::an_unfindable_item_end_blanks_the_attribute();
}

mod contract_mappings;

use contract_mappings::mappings;

#[test]
fn every_test_the_container_fault_row_names_is_a_test_in_this_tree() {
    mappings::every_fault_row_name_is_a_test_in_the_tree();
}

#[test]
fn the_container_fault_row_predicate_refuses_a_name_that_is_only_prose() {
    mappings::the_presence_predicate_refuses_a_non_test_shape();
}

#[test]
fn the_view_directory_has_one_definition_in_the_tree() {
    let container: Vec<(String, String)> = scanned_sources()
        .into_iter()
        .filter(|(path, _)| {
            path.starts_with("src/runner/container") && !path.ends_with("/tests.rs")
        })
        .collect();
    let modules: BTreeSet<&str> = container.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(
        modules,
        BTreeSet::from([
            "src/runner/container.rs",
            "src/runner/container/census.rs",
            "src/runner/container/env.rs",
            "src/runner/container/exec.rs",
            "src/runner/container/fake.rs",
            "src/runner/container/intent.rs",
            "src/runner/container/resolve.rs",
            "src/runner/container/runtime.rs",
            "src/runner/container/view.rs",
        ]),
        "the container substrate's production modules moved; the seam this test \
         pins may no longer be inside the scanned set"
    );

    let mut sites = Vec::new();
    let mut located = Vec::new();
    for (path, source) in &container {
        let code = blank_comments(&production_region(source));
        for (index, _) in code.match_indices("\"views\"") {
            let line = code[..index].matches('\n').count() + 1;
            sites.push(path.clone());
            located.push(format!("{path}:{line}"));
        }
    }
    assert_eq!(
        sites,
        vec!["src/runner/container/census.rs".to_owned()],
        "the R19 view directory segment is declared in more than one production \
         site. `census::VIEWS_DIR` is the one definition and `exec::view_dir` \
         delegates to `census::view_path`; a second literal is a path that can \
         drift away from the census that has to find it, and no behavioural test \
         crosses the two halves. Sites found: {located:?}"
    );

    let (_, census) = container
        .iter()
        .find(|(path, _)| path == "src/runner/container/census.rs")
        .expect("the census module is in the scanned set");
    assert!(
        blank_comments(&production_region(census))
            .contains("pub const VIEWS_DIR: &str = \"views\";"),
        "the scan cannot see the declaration it is counting"
    );

    let root = Path::new("/private/root");
    let name = crate::runner::container::intent::ContainerName::from_parts(
        "repokey",
        "run01",
        "inc01",
        "0123456789abcdef",
    )
    .expect("a well-formed container name");
    assert_eq!(
        crate::runner::container::exec::view_dir(root, &name),
        crate::runner::container::census::view_path(root, &name),
        "the runner mounts the view somewhere the census does not look"
    );
}

#[test]
fn the_whole_file_test_modules_are_resolved_from_the_declarations_not_the_file_names() {
    oracles::the_whole_file_modules_are_read_from_the_declarations();
}

#[test]
fn the_module_scan_reads_ancestry_and_visibility_rather_than_text_after_an_attribute() {
    use crate::effects::census_domain::{
        Predicate, ScannedDeclaration, entails_test, parse_predicate, scan_module_declarations,
    };

    fn scan(source: &str) -> Vec<ScannedDeclaration> {
        scan_module_declarations(source)
            .unwrap_or_else(|refusal| panic!("the fixture is readable: {refusal}"))
    }
    fn only(source: &str) -> ScannedDeclaration {
        let mut found = scan(source);
        assert_eq!(found.len(), 1, "{source:?} -> {found:#?}");
        found.remove(0)
    }

    let plain = only("#[cfg(test)]\nmod tests;\n");
    assert_eq!(plain.name, "tests");
    assert!(plain.inline_path.is_empty());
    assert_eq!(plain.guard, "test");
    assert!(plain.test_only);

    for written in [
        "#[cfg(test)]\npub mod helpers;\n",
        "#[cfg(test)]\npub(crate) mod helpers;\n",
        "#[cfg(test)]\npub(super) mod helpers;\n",
        "#[cfg(test)]\npub(in crate::a::b) mod helpers;\n",
    ] {
        let qualified = only(written);
        assert_eq!(qualified.name, "helpers", "{written:?}");
        assert!(qualified.test_only, "{written:?}");
    }
    assert!(!only("pub(crate) mod helpers;\n").test_only);

    let inherited =
        only("#[cfg(test)]\npub(crate) mod test_support {\n    pub(crate) mod readiness;\n}\n");
    assert_eq!(inherited.name, "readiness");
    assert_eq!(inherited.inline_path, vec!["test_support".to_owned()]);
    assert_eq!(inherited.guard, "test");
    assert!(inherited.test_only);
    let ungated = only("pub(crate) mod test_support {\n    pub(crate) mod readiness;\n}\n");
    assert_eq!(ungated.inline_path, vec!["test_support".to_owned()]);
    assert!(
        !ungated.test_only,
        "a declaration under an unguarded inline module is production code"
    );

    let deep =
        only("mod outer {\n    #[cfg(test)]\n    mod middle {\n        pub mod leaf;\n    }\n}\n");
    assert_eq!(deep.name, "leaf");
    assert_eq!(
        deep.inline_path,
        vec!["outer".to_owned(), "middle".to_owned()]
    );
    assert!(deep.test_only);

    let both = scan("#[cfg(test)]\nmod inner {\n    mod under;\n}\nmod beside;\n");
    assert_eq!(both.len(), 2, "{both:#?}");
    assert_eq!(both[0].name, "under");
    assert_eq!(both[0].inline_path, vec!["inner".to_owned()]);
    assert!(both[0].test_only);
    assert_eq!(both[1].name, "beside");
    assert!(both[1].inline_path.is_empty());
    assert!(
        !both[1].test_only,
        "a declaration after the guarded block inherited a guard that had closed"
    );

    let after_a_function = scan("#[cfg(test)]\nfn helper() {}\nmod plain;\n");
    assert_eq!(after_a_function.len(), 1, "{after_a_function:#?}");
    assert!(!after_a_function[0].test_only);
    assert!(
        scan("#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n").is_empty(),
        "an inline module with a body names no file"
    );

    for (written, expected) in [
        ("#[cfg(test)]\nmod x;\n", true),
        ("#[cfg(all(test, unix))]\nmod x;\n", true),
        ("#[cfg(all(unix, all(test, windows)))]\nmod x;\n", true),
        ("#[cfg(test)]\n#[cfg(unix)]\nmod x;\n", true),
        ("#[cfg(unix)]\nmod outer {\n#[cfg(test)]\nmod x;\n}\n", true),
        ("#[cfg(any(test, unix))]\nmod x;\n", false),
        ("#[cfg(not(test))]\nmod x;\n", false),
        ("#[cfg(unix)]\nmod x;\n", false),
        ("#[cfg(feature = \"slow\")]\nmod x;\n", false),
        ("mod x;\n", false),
        ("#[cfg_attr(not(test), cfg(test))]\nmod x;\n", true),
        (
            "#[cfg_attr(not(test), cfg_attr(all(), cfg(any())))]\nmod x;\n",
            true,
        ),
        (
            "#[cfg_attr(not(test), allow(dead_code), cfg(test))]\nmod x;\n",
            true,
        ),
        ("#[cfg_attr(unix, cfg(test))]\nmod x;\n", false),
        ("#[cfg_attr(test, cfg(any()))]\nmod x;\n", false),
        ("#[cfg_attr(not(test), allow(dead_code))]\nmod x;\n", false),
        (
            "#[cfg(all(feature = \"a\\\"\", test, feature = \"b\\\"\"))]\nmod x;\n",
            true,
        ),
        (
            "#[cfg(all(feature = \"\\\", test, y = \\\"\"))]\nmod x;\n",
            false,
        ),
    ] {
        assert_eq!(
            only(written).test_only,
            expected,
            "{written:?} was decided the other way"
        );
    }
    for refused in [
        "#![cfg_attr(not(test), cfg(test))]\nmod x;\n",
        "#[cfg_attr(not(te st), cfg(test))]\nmod x;\n",
    ] {
        assert!(
            scan_module_declarations(refused).is_err(),
            "{refused:?}: a generated `cfg` the scan cannot place or read was classified anyway"
        );
    }

    for written in ["test", "all(test, unix)", "not(any(not(test), unix))"] {
        let pred = parse_predicate(written).unwrap_or_else(|why| panic!("{written}: {why}"));
        assert!(entails_test(&pred), "`{written}` does not entail `test`");
    }
    for written in [
        "any(test, unix)",
        "not(test)",
        "unix",
        "target_os = \"linux\"",
        "all(unix, windows)",
    ] {
        let pred = parse_predicate(written).unwrap_or_else(|why| panic!("{written}: {why}"));
        assert!(
            !entails_test(&pred),
            "`{written}` was read as entailing `test`"
        );
    }
    assert_eq!(
        parse_predicate("all(test, unix)").map(|pred| pred.render()),
        Ok("all(test, unix)".to_owned())
    );
    assert_eq!(parse_predicate("test"), Ok(Predicate::Test));

    for prose in [
        "// #[cfg(test)] mod ghost;\n",
        "/* #[cfg(test)] mod ghost; */\n",
        "/// #[cfg(test)] mod ghost;\nfn documented() {}\n",
        "const S: &str = \"#[cfg(test)] mod ghost;\";\n",
        "const S: &str = r#\"#[cfg(test)] mod ghost;\"#;\n",
        "const S: &[u8] = b\"#[cfg(test)] mod ghost;\";\n",
    ] {
        assert!(scan(prose).is_empty(), "{prose:?} derived a declaration");
    }
    let after_a_brace_char = only("const C: char = '{';\n#[cfg(test)]\nmod real;\n");
    assert_eq!(after_a_brace_char.name, "real");
    assert!(after_a_brace_char.inline_path.is_empty());
    assert!(after_a_brace_char.test_only);

    assert!(scan("fn models() {}\nstruct modest;\n").is_empty());

    let past_a_macro = only("thread_local! {\n    static X: u8 = 0;\n}\n#[cfg(test)]\nmod real;\n");
    assert_eq!(past_a_macro.name, "real");
    assert!(past_a_macro.inline_path.is_empty());
    assert!(past_a_macro.test_only);
    let after_attributed_macro = only("#[cfg(test)]\nlazy! [ a, b ]\nmod plain;\n");
    assert_eq!(after_attributed_macro.name, "plain");
    assert!(
        !after_attributed_macro.test_only,
        "a `#[cfg(test)]` above a macro invocation carried to the next item"
    );
    let past_a_negation = only("fn f() { let _ = a != b; }\n#[cfg(test)]\nmod real;\n");
    assert_eq!(past_a_negation.name, "real");
    assert!(past_a_negation.test_only);

    for tokens in [
        "macro_rules! m {\n    (mod $n:ident) => {\n        ()\n    };\n}\n",
        "m! { mod }\n",
        "outer! { inner! { mod } }\n",
    ] {
        assert_eq!(
            scan_module_declarations(tokens).map(|found| found.len()),
            Ok(0),
            "{tokens:?} was read as items rather than discarded"
        );
    }
    let beside_a_macro = only(
        "macro_rules! m {\n    (mod $n:ident) => {\n        ()\n    };\n}\n#[cfg(test)]\nmod real;\n",
    );
    assert_eq!(beside_a_macro.name, "real");
    assert!(beside_a_macro.test_only);

    for spaced in [
        "vec ! [1, 2];\n#[cfg(test)]\nmod real;\n",
        "assert /* sic */ ! (a == b);\n#[cfg(test)]\nmod real;\n",
        "macro_rules ! m {\n    () => {\n        fn go() {}\n    };\n}\n#[cfg(test)]\nmod real;\n",
        "macro_rules ! m {\n    (mod $n:ident) => {\n        ()\n    };\n}\n#[cfg(test)]\nmod real;\n",
        "macro_rules /* named next */ ! m {\n    (mod $n:ident) => {\n        ()\n    };\n}\n#[cfg(test)]\nmod real;\n",
    ] {
        let past = only(spaced);
        assert_eq!(past.name, "real", "{spaced:?}");
        assert!(past.test_only, "{spaced:?}");
    }

    let inside_a_negated_block = only(
        "#[cfg(test)]\nmod outer {\n    fn f() {\n        if !ready { }\n    }\n    mod inner;\n}\n",
    );
    assert_eq!(inside_a_negated_block.name, "inner");
    assert_eq!(
        inside_a_negated_block.inline_path,
        vec!["outer".to_owned()],
        "a negated condition was read as a macro and swallowed the block"
    );
    assert!(inside_a_negated_block.test_only);
    for negation in [
        "fn f() { if !ready { } }\n#[cfg(test)]\nmod real;\n",
        "fn f() { while !done { } }\n#[cfg(test)]\nmod real;\n",
        "fn f() { let _ = !flag; }\n#[cfg(test)]\nmod real;\n",
    ] {
        let past = only(negation);
        assert_eq!(past.name, "real", "{negation:?}");
        assert!(past.test_only, "{negation:?}");
    }
    let inside_a_negated_block = only(
        "#[cfg(test)]\nmod outer {\n    fn f() {\n        if !ready {\n            mod local;\n        }\n    }\n}\n",
    );
    assert_eq!(inside_a_negated_block.name, "local");
    assert_eq!(
        inside_a_negated_block.inline_path,
        vec!["outer".to_owned()],
        "the negated block was skipped as a macro body and its declaration lost"
    );
    assert!(inside_a_negated_block.test_only);
    let inside_a_negated_loop = only(
        "mod outer {\n    fn f() {\n        while !done {\n            mod local;\n        }\n    }\n}\n",
    );
    assert_eq!(inside_a_negated_loop.name, "local");
    assert!(!inside_a_negated_loop.test_only);

    for negated_group in [
        "#[cfg(test)]\nmod outer {\n    fn f() -> bool {\n        if !({ mod local {} true }) { false } else { true }\n    }\n}\n",
        "mod outer {\n    fn f() -> bool {\n        !({ mod local {} true })\n    }\n}\n",
        "mod outer {\n    fn f() {\n        while !({ mod local {} false }) { }\n    }\n}\n",
        "mod outer {\n    fn f() -> bool {\n        return !({ mod local {} true });\n    }\n}\n",
    ] {
        let read = scan_module_declarations(negated_group)
            .unwrap_or_else(|refusal| panic!("{negated_group:?} was refused: {refusal}"));
        assert!(
            read.is_empty(),
            "an inline `mod local {{}}` names no file, so it is a scope and not a declaration: \
             {negated_group:?} -> {read:#?}"
        );
    }
    let through_a_negated_group = only(
        "#[cfg(test)]\nmod outer {\n    fn f() -> bool {\n        if !({ mod local; true }) { false } else { true }\n    }\n}\n",
    );
    assert_eq!(through_a_negated_group.name, "local");
    assert_eq!(
        through_a_negated_group.inline_path,
        vec!["outer".to_owned()],
        "the negated group was skipped as a macro body and its declaration lost"
    );
    assert!(through_a_negated_group.test_only);

    for (written, expected) in [
        ("#[cfg(test)]\nmod r#type;\n", "type"),
        ("#[cfg(test)]\npub(crate) mod r#fn;\n", "fn"),
        ("#[cfg(test)]\nmod r#tests;\n", "tests"),
    ] {
        let raw = only(written);
        assert_eq!(raw.name, expected, "{written:?}");
        assert!(raw.test_only, "{written:?}");
    }
    assert!(scan("struct r#mod;\nfn f() { let raw = 1; }\n").is_empty());
    let beside_a_raw_word = only("fn raw() {}\n#[cfg(test)]\nmod real;\n");
    assert_eq!(beside_a_raw_word.name, "real");

    let raw_binding = "fn f() { let r#mod = 1; }\n#[cfg(test)]\nmod tests;\n";
    let read = scan_module_declarations(raw_binding).unwrap_or_else(|refusal| {
        panic!("`let r#mod = 1;` is valid Rust and was refused: {refusal}")
    });
    assert_eq!(read.len(), 1, "{read:#?}");
    assert_eq!(read[0].name, "tests");
    assert!(read[0].test_only);

    let raw_in_a_use = "#[cfg(test)]\nmod harness {\n    use std::r#mod as tests;\n}\n";
    assert_eq!(
        scan_module_declarations(raw_in_a_use),
        Ok(Vec::new()),
        "`use std::r#mod as tests;` declares no module, and the text inside `r#mod` is not an \
         item"
    );

    for source in [
        "fn f() { let r#mod = 1; }\n#[cfg(test)]\nmod real;\n",
        "fn f() { let r#type = 1; }\n#[cfg(test)]\nmod real;\n",
        "fn f() { let r = 1; }\n#[cfg(test)]\nmod real;\n",
        "fn f() { let raw = 1; }\n#[cfg(test)]\nmod real;\n",
    ] {
        assert_eq!(only(source).name, "real", "{source:?}");
        assert_eq!(
            scan_module_declarations(&source.replace('\n', "\r\n")),
            scan_module_declarations(source),
            "CRLF: {source:?}"
        );
    }

    let past_a_raw_macro = only("r#if! { let _ = 1; }\n#[cfg(test)]\nmod real;\n");
    assert_eq!(past_a_raw_macro.name, "real");
    assert!(past_a_raw_macro.test_only);

    for fixture in [
        "#[cfg(test)]\npub(crate) mod test_support {\n    pub(crate) mod readiness;\n}\n",
        "mod outer {\n    #[cfg(test)]\n    mod middle {\n        pub mod leaf;\n    }\n}\n",
        "macro_rules ! m {\n    (mod $n:ident) => {\n        ()\n    };\n}\n#[cfg(test)]\nmod real;\n",
        "#[cfg(test)]\nmod r#type;\n",
    ] {
        let lf = scan_module_declarations(fixture);
        let crlf = scan_module_declarations(&fixture.replace('\n', "\r\n"));
        assert_eq!(lf, crlf, "CRLF changed the derivation for {fixture:?}");
        assert!(lf.is_ok_and(|found| found.len() == 1));
    }
    for refused in [
        "macro_rules! m {\n    () => {\n        #[cfg(test)]\n        mod x;\n    };\n}\n",
        "#[cfg(test)]\nmod tests;\n#[cfg(test)]\nmod tests;\n",
    ] {
        assert_eq!(
            scan_module_declarations(refused).is_err(),
            scan_module_declarations(&refused.replace('\n', "\r\n")).is_err(),
            "CRLF changed whether {refused:?} is refused"
        );
        assert!(scan_module_declarations(refused).is_err());
    }
}

#[test]
fn the_module_scan_reports_inline_modules_at_every_depth_with_what_they_write() {
    use crate::effects::census_domain::{scan_module_declarations, scan_modules};
    use crate::effects::lint_levels::leading_inner_attributes;

    let source = concat!(
        "//! docs\n",
        "#![deny(clippy::disallowed_methods)]\n",
        "\n",
        "mod plain;\n",
        "\n",
        "#[cfg(test)]\n",
        "#[allow(clippy::disallowed_methods)]\n",
        "pub(crate) mod outer {\n",
        "    #![allow(clippy::disallowed_types)]\n",
        "    // prose that says mod fake { } is not a module\n",
        "    const TEXT: &str = \"mod also_fake { }\";\n",
        "\n",
        "    pub mod inner {\n",
        "        mod leaf;\n",
        "        pub(super) mod deepest {}\n",
        "    }\n",
        "}\n",
        "\n",
        "#[derive(Debug)]\n",
        "struct Carrier;\n",
        "mod after_an_attributed_item {}\n",
        "\n",
        "#[allow(clippy::disallowed_macros)]\n",
        "mod declared;\n",
        "mod after_an_attributed_declaration {}\n",
    );
    let scanned = scan_modules(source).expect("the fixture scans");

    assert_eq!(
        scanned.declared,
        scan_module_declarations(source).expect("the fixture scans")
    );
    let declared: Vec<(&str, Vec<&str>)> = scanned
        .declared
        .iter()
        .map(|declaration| {
            (
                declaration.name.as_str(),
                declaration.inline_path.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        declared,
        vec![
            ("plain", vec![]),
            ("leaf", vec!["outer", "inner"]),
            ("declared", vec![]),
        ]
    );

    let inline: Vec<(&str, Vec<&str>, usize, bool)> = scanned
        .inline
        .iter()
        .map(|module| {
            (
                module.name.as_str(),
                module.inline_path.iter().map(String::as_str).collect(),
                module.line,
                module.test_only,
            )
        })
        .collect();
    assert_eq!(
        inline,
        vec![
            ("outer", vec![], 8, true),
            ("inner", vec!["outer"], 13, true),
            ("deepest", vec!["outer", "inner"], 15, true),
            ("after_an_attributed_item", vec![], 21, false),
            ("after_an_attributed_declaration", vec![], 25, false),
        ],
        "every inline module, at every depth, in source order, and nothing a comment or a \
         string spells"
    );
    assert_eq!(
        inline_module_openers(source),
        scanned.inline.len(),
        "the cruder reading the engine guard checks the scan against disagrees with it on the \
         fixture that pins both"
    );
    for separator in super::RUSTC_WHITESPACE {
        let written = format!(
            "#[rustfmt::skip]\npub(super) mod{separator}declared_child{separator};\n\
             mod{separator}inline_child{separator}{{}}\n"
        );
        let read = scan_modules(&written).expect("the separator fixture scans");
        let declared: Vec<&str> = read.declared.iter().map(|d| d.name.as_str()).collect();
        let inline: Vec<&str> = read.inline.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            (declared, inline, inline_module_openers(&written)),
            (vec!["declared_child"], vec!["inline_child"], 1),
            "U+{:04X} separates tokens for rustc, so `mod`, it, and a name is a module rustc \
             compiles; the review of 84123789 executed `mod`, U+200E, a child of \
             `src/engine/attempt.rs` that no walk judged: {written:?}",
            u32::from(separator)
        );
    }

    let written = |module: &crate::effects::census_domain::ScannedInlineModule| -> Vec<String> {
        let own = format!(
            "{}\n{}\n",
            module.outer_attributes,
            leading_inner_attributes(&module.body)
        );
        governed_allows(&own)
            .iter()
            .flat_map(|allow| allow.lints.iter().cloned())
            .collect()
    };
    let by_name = |name: &str| {
        scanned
            .inline
            .iter()
            .find(|module| module.name == name)
            .expect("an inline module the fixture writes")
    };
    assert_eq!(
        written(by_name("outer")),
        vec![
            "disallowed_methods".to_owned(),
            "disallowed_types".to_owned()
        ],
        "an inline module's own attributes are the ones outside its braces and the ones inside"
    );
    assert!(
        by_name("outer").outer_attributes.contains("#[cfg(test)]"),
        "{:?}",
        by_name("outer").outer_attributes
    );
    for unattributed in [
        "inner",
        "deepest",
        "after_an_attributed_item",
        "after_an_attributed_declaration",
    ] {
        assert_eq!(
            (
                by_name(unattributed).outer_attributes.as_str(),
                written(by_name(unattributed)),
            ),
            ("", Vec::new()),
            "`{unattributed}` writes no attribute, and was handed one that belongs to a \
             neighbour"
        );
    }

    assert_eq!(
        leading_inner_attributes(source),
        "//! docs\n#![deny(clippy::disallowed_methods)]",
        "the leading attributes of a file are read up to its first item and no further"
    );
    assert_eq!(
        leading_inner_attributes("fn first() {}\n#![allow(x)]\n"),
        ""
    );
    assert_eq!(
        leading_inner_attributes("#![a]\r\n#![b(\r\n  c\r\n)]\r\nmod m;\r\n"),
        "#![a]\r\n#![b(\r\n  c\r\n)]"
    );
}

fn is_the_literal_mod_tests_form(name: &str, inline_path: &[String], guard: &str) -> bool {
    name == "tests" && inline_path.is_empty() && guard == "test"
}

#[test]
fn a_narrowed_cfg_guard_is_test_only_but_is_not_the_literal_mod_tests_form() {
    use crate::effects::census_domain::{ScannedDeclaration, scan_module_declarations};

    fn only(source: &str) -> ScannedDeclaration {
        let mut found = scan_module_declarations(source)
            .unwrap_or_else(|refusal| panic!("the fixture is readable: {refusal}"));
        assert_eq!(found.len(), 1, "{source:?} -> {found:#?}");
        found.remove(0)
    }
    fn literal(declaration: &ScannedDeclaration) -> bool {
        is_the_literal_mod_tests_form(
            &declaration.name,
            &declaration.inline_path,
            &declaration.guard,
        )
    }

    let plain = only("#[cfg(test)]\nmod tests;\n");
    assert_eq!(plain.guard, "test");
    assert!(plain.test_only);
    assert!(literal(&plain), "{plain:#?}");

    for narrowed in [
        "#[cfg(all(test, unix))]\nmod tests;\n",
        "#[cfg(test)]\n#[cfg(unix)]\nmod tests;\n",
    ] {
        let declaration = only(narrowed);
        assert_eq!(declaration.name, plain.name);
        assert_eq!(declaration.inline_path, plain.inline_path);
        assert!(
            declaration.test_only,
            "a narrowed guard still entails `test`, so the file is still a whole-file test \
             module and still belongs in the census domain: {declaration:#?}"
        );
        assert_ne!(
            declaration.guard, plain.guard,
            "the guard is the only field that differs, so it is the only field that can \
             distinguish them"
        );
        assert!(
            !literal(&declaration),
            "{narrowed:?} is not the literal `#[cfg(test)] mod tests;` form: rustc compiles no \
             such module where the narrowing is false, and a census that counted it as the plain \
             form would skip a file that is not there and lose the module on that platform in \
             silence: {declaration:#?}"
        );
    }

    let inherited = only("#[cfg(test)]\nmod test_support {\n    pub(crate) mod readiness;\n}\n");
    assert_eq!(
        (inherited.guard.as_str(), inherited.name.as_str()),
        ("test", "readiness")
    );
    assert!(
        inherited.test_only && !literal(&inherited),
        "{inherited:#?}"
    );
    let other_name = only("#[cfg(test)]\nmod scaffold;\n");
    assert_eq!(other_name.guard, "test");
    assert!(other_name.inline_path.is_empty());
    assert!(
        other_name.test_only && !literal(&other_name),
        "{other_name:#?}"
    );
}

#[test]
fn the_module_resolver_refuses_every_shape_it_cannot_resolve() {
    use crate::effects::census_domain::{
        CandidateRefusal, ScanRefusal, candidates_for, contained_in, declaration_cycle,
        module_directory, parse_predicate, scan_module_declarations, sole_present,
    };

    fn refusal(source: &str) -> ScanRefusal {
        scan_module_declarations(source).expect_err("this source is refused")
    }

    assert_eq!(
        refusal("#[cfg(test)\nmod tests;\n"),
        ScanRefusal::UnclosedAttribute { line: 1 }
    );
    assert_eq!(
        refusal("mod a { }\n}\n"),
        ScanRefusal::UnbalancedBraces { line: 2 }
    );
    for malformed in ["mod ;\n", "mod x = 3;\n", "mod trailing\n"] {
        assert!(
            matches!(refusal(malformed), ScanRefusal::MalformedDeclaration { .. }),
            "{malformed:?} was read as a declaration"
        );
    }

    for unreadable in [
        "#[cfg(sometimes(test))]\nmod x;\n",
        "#[cfg(test]\nmod x;\n",
        "#[cfg()]\nmod x;\n",
        "#[cfg(not(test, unix))]\nmod x;\n",
        "#[cfg(feature =)]\nmod x;\n",
    ] {
        assert!(
            matches!(refusal(unreadable), ScanRefusal::UnreadablePredicate { .. }),
            "{unreadable:?} was decided rather than refused"
        );
    }
    for unreadable in [
        "",
        "all(test",
        "not(test, unix)",
        "maybe(test)",
        "all(test) extra",
    ] {
        assert!(
            parse_predicate(unreadable).is_err(),
            "`{unreadable}` parsed"
        );
    }

    for pathed in [
        "#[path = \"elsewhere.rs\"]\nmod x;\n",
        "#[cfg_attr(unix, path = \"elsewhere.rs\")]\nmod x;\n",
    ] {
        assert!(
            matches!(
                refusal(pathed),
                ScanRefusal::UnsupportedPathAttribute { .. }
            ),
            "{pathed:?} was resolved"
        );
    }
    assert!(
        scan_module_declarations("#[path = \"x\"]\nstruct S;\nmod y;\n").is_ok(),
        "a `path` attribute on a non-module item is not a module path attribute"
    );

    for shaped in [
        "macro_rules! m {\n    () => {\n        mod x;\n    };\n}\n",
        "macro_rules! m {\n    () => {\n        #[cfg(test)]\n        mod x;\n    };\n}\n",
        "quote! { mod x; }\n",
        "paste!( mod x { } );\n",
        "items![ pub(crate) mod x; ]\n",
        "outer! { inner! { mod x; } }\n",
        "macro_rules! r#mod {\n    () => {\n        mod x;\n    };\n}\n",
        "macro_rules ! r#type {\n    () => {\n        #[cfg(test)]\n        mod x;\n    };\n}\n",
        "quote! { mod r#type; }\n",
        "r#if! { mod r#fn { } }\n",
    ] {
        assert!(
            matches!(refusal(shaped), ScanRefusal::ModuleShapedMacroBody { .. }),
            "{shaped:?} was read rather than refused"
        );
    }
    for spaced in [
        "macro_rules ! m {\n    () => {\n        mod x;\n    };\n}\n",
        "macro_rules\n! m {\n    () => {\n        #[cfg(test)]\n        mod x;\n    };\n}\n",
        "macro_rules /* named next */ ! m {\n    () => {\n        mod x;\n    };\n}\n",
        "#[rustfmt::skip]\nmacro_rules  !  m  {\n    () => {\n        mod x;\n    };\n}\n",
        "quote ! { mod x; }\n",
        "quote // why\n! { mod x; }\n",
        "quote /* why */ ! { pub(crate) mod x; }\n",
        "items\n    ![ mod x { } ]\n",
    ] {
        assert!(
            matches!(refusal(spaced), ScanRefusal::ModuleShapedMacroBody { .. }),
            "{spaced:?} was read rather than refused"
        );
    }

    for ordinary in [
        "vec![1, 2, 3];\n",
        "assert!(a == b, \"mod x; is prose here\");\n",
        "macro_rules! m {\n    () => {\n        fn go() {}\n    };\n}\n",
        "modify!(x);\n",
    ] {
        assert_eq!(
            scan_module_declarations(ordinary).map(|found| found.len()),
            Ok(0),
            "{ordinary:?} was not discarded cleanly"
        );
    }

    assert!(matches!(
        refusal("#![cfg(test)]\nmod x;\n"),
        ScanRefusal::UnsupportedInnerCfg { .. }
    ));

    assert!(matches!(
        refusal("#[cfg(test)]\nmod tests;\n#[cfg(test)]\nmod tests;\n"),
        ScanRefusal::DuplicateDeclaration { .. }
    ));
    assert!(
        scan_module_declarations("mod a {\n    mod x;\n}\nmod b {\n    mod x;\n}\n").is_ok(),
        "two parents each declaring `x` are not a duplicate"
    );

    let roots = crate::effects::tests::crate_roots();
    let root = repo_root();
    let named = |file: &str, inline: &[String], name: &str| {
        candidates_for(roots, &root.join(file), inline, name)
    };
    assert_eq!(
        named(
            "src/agent/proc.rs",
            &["test_support".to_owned()],
            "readiness"
        ),
        Ok([
            root.join("src/agent/proc/test_support/readiness.rs"),
            root.join("src/agent/proc/test_support/readiness/mod.rs"),
        ])
    );
    assert_eq!(
        named("src/agent/proc.rs", &[], "readiness"),
        Ok([
            root.join("src/agent/proc/readiness.rs"),
            root.join("src/agent/proc/readiness/mod.rs"),
        ])
    );
    for flattened in named("src/agent/proc.rs", &[], "readiness").expect("inside the package") {
        assert!(
            !flattened.is_file(),
            "{} exists, so the flattening mutation would resolve instead of refusing",
            flattened.display()
        );
    }

    assert_eq!(
        named("src/engine/mod.rs", &[], "tests").map(|pair| pair[0].clone()),
        Ok(root.join("src/engine/tests.rs"))
    );
    assert_eq!(
        named("src/lib.rs", &[], "effects").map(|pair| pair[0].clone()),
        Ok(root.join("src/effects.rs"))
    );
    assert_eq!(
        named("src/main.rs", &[], "tests").map(|pair| pair[0].clone()),
        Ok(root.join("src/tests.rs"))
    );
    assert!(
        roots.is_root(&root.join("examples/probe.rs")),
        "`examples/probe.rs` is a target of this package: {:?}",
        roots.roots().collect::<Vec<_>>()
    );
    assert_eq!(
        named("examples/probe.rs", &[], "helper").map(|pair| pair[0].clone()),
        Ok(root.join("examples/helper.rs"))
    );
    assert_eq!(
        named("src/a/lib.rs", &[], "tests").map(|pair| pair[0].clone()),
        Ok(root.join("src/a/lib/tests.rs"))
    );
    assert_eq!(
        named("src/a/b/main.rs", &[], "tests").map(|pair| pair[0].clone()),
        Ok(root.join("src/a/b/main/tests.rs"))
    );
    assert_eq!(
        module_directory(roots, &root.join("src/a/mod.rs")),
        Ok(root.join("src/a"))
    );
    assert_eq!(
        module_directory(roots, &root.join("src/a/other.rs")),
        Ok(root.join("src/a/other")),
        "an ordinary module owns a directory named after it, never its parent"
    );
    let elsewhere = std::env::temp_dir().join("upstroke-not-this-package/src/lib.rs");
    assert_eq!(
        module_directory(roots, &elsewhere),
        Err(CandidateRefusal::OutsideThePackage {
            declared_in: elsewhere.clone(),
            package_dir: root.clone(),
        })
    );
    assert!(
        CandidateRefusal::OutsideThePackage {
            declared_in: elsewhere,
            package_dir: root.clone(),
        }
        .to_string()
        .contains("does not say whether it is a crate root"),
        "the refusal says what it could not decide"
    );

    let pair = named("src/a.rs", &[], "b").expect("an ordinary module");
    assert_eq!(sole_present(&pair, &|_| false), Err(0));
    assert_eq!(sole_present(&pair, &|_| true), Err(2));
    assert_eq!(sole_present(&pair, &|at| at == pair[0]), Ok(&pair[0]));
    assert_eq!(sole_present(&pair, &|at| at == pair[1]), Ok(&pair[1]));

    let base = Path::new("src/agent");
    assert!(contained_in(
        base,
        Path::new("src/agent/proc/test_support/readiness.rs")
    ));
    assert!(
        !contained_in(base, base),
        "a directory does not contain itself"
    );
    assert!(!contained_in(base, Path::new("src/effects.rs")));
    assert!(
        !contained_in(base, Path::new("src/agent/../effects.rs")),
        "a `..` component escapes and must not read as contained"
    );

    let edge = |from: &str, to: &str| (PathBuf::from(from), PathBuf::from(to));
    let forest = vec![edge("a.rs", "a/b.rs"), edge("a/b.rs", "a/b/c.rs")];
    assert_eq!(declaration_cycle(&forest), None);
    assert!(
        declaration_cycle(&[edge("a.rs", "a.rs")]).is_some(),
        "a file declaring itself is a cycle"
    );
    assert!(
        declaration_cycle(&[edge("a.rs", "b.rs"), edge("b.rs", "a.rs")]).is_some(),
        "a two-file loop is a cycle"
    );
    let branching = vec![
        edge("a.rs", "a/b.rs"),
        edge("a.rs", "a/c.rs"),
        edge("a/c.rs", "a.rs"),
    ];
    let closed = declaration_cycle(&branching).expect("the second edge closes a loop");
    assert_eq!(
        closed.first(),
        closed.last(),
        "a reported cycle must start and end at the same node: {closed:?}"
    );
    assert!(
        closed.contains(&PathBuf::from("a/c.rs")),
        "the reported cycle does not name the branch that closes it: {closed:?}"
    );
    assert_eq!(
        declaration_cycle(&[edge("a.rs", "a/b.rs"), edge("a.rs", "a/c.rs")]),
        None
    );
    let deferred = vec![
        edge("a.rs", "a/b.rs"),
        edge("a/b.rs", "a/b/leaf.rs"),
        edge("a.rs", "a/c.rs"),
        edge("a/c.rs", "a/d.rs"),
        edge("a/d.rs", "a/c.rs"),
    ];
    assert!(
        declaration_cycle(&deferred).is_some(),
        "a cycle two branches deep was not reached"
    );
}

#[test]
#[should_panic(expected = "does not describe the tree this census was handed")]
fn a_census_handed_a_source_root_the_manifest_does_not_describe_is_refused() {
    let elsewhere = std::env::temp_dir().join("upstroke-not-this-package");
    let _ = crate::effects::census_domain::declared_whole_file_test_modules(&elsewhere, &[]);
}

#[test]
fn the_cfg_census_resolves_module_directories_through_the_target_inventory() {
    for (file, directory) in [
        ("src/lib.rs", "src"),
        ("src/main.rs", "src"),
        ("examples/probe.rs", "examples"),
        ("src/engine/mod.rs", "src/engine"),
        ("src/effects.rs", "src/effects"),
        ("src/a/lib.rs", "src/a/lib"),
        ("src/a/main.rs", "src/a/main"),
    ] {
        assert_eq!(cfg::module_dir(file), directory, "`{file}`");
    }
}

#[test]
fn the_crate_roots_come_from_the_manifest_and_an_arbitrary_bin_path_is_one() {
    use crate::effects::census_domain::{CrateRoots, InventoryRefusal, module_directory};

    fn by_stem(file: &Path) -> PathBuf {
        let parent = file.parent().expect("a directory").to_path_buf();
        let stem = file.file_stem().expect("a name");
        if stem == "mod" || stem == "lib" || stem == "main" {
            parent
        } else {
            parent.join(stem)
        }
    }

    let scratch = scratch_dir("inventory");
    fs::write(
        scratch.join("Cargo.toml"),
        "[package]\n\
         name = \"upstroke-inventory-fixture\"\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\
         \n\
         [lib]\n\
         path = \"src/lib.rs\"\n\
         \n\
         [[bin]]\n\
         name = \"odd\"\n\
         path = \"src/tools/odd.rs\"\n\
         \n\
         [[bin]]\n\
         name = \"nested\"\n\
         path = \"src/deep/nest/main.rs\"\n\
         \n\
         [workspace]\n",
    )
    .expect("the fixture manifest");

    let inventory = crate_roots_of(&scratch).expect("cargo reads the fixture manifest");
    assert_eq!(inventory.package_dir(), scratch.as_path());
    assert_eq!(
        inventory.roots().collect::<Vec<_>>(),
        vec![
            scratch.join("src/deep/nest/main.rs").as_path(),
            scratch.join("src/lib.rs").as_path(),
            scratch.join("src/tools/odd.rs").as_path(),
        ],
        "the inventory is exactly the manifest's three targets"
    );

    for (file, owns, stem_says) in [
        ("src/tools/odd.rs", "src/tools", "src/tools/odd"),
        ("src/deep/nest/main.rs", "src/deep/nest", "src/deep/nest"),
        ("src/a/lib.rs", "src/a/lib", "src/a"),
    ] {
        let declared_in = scratch.join(file);
        assert_eq!(
            module_directory(&inventory, &declared_in),
            Ok(scratch.join(owns)),
            "`{file}` owns `{owns}`"
        );
        assert_eq!(
            by_stem(&declared_in),
            scratch.join(stem_says),
            "the stem rule's answer for `{file}` is recorded, not guessed"
        );
    }
    let disagreements = ["src/tools/odd.rs", "src/a/lib.rs"]
        .into_iter()
        .filter(|file| {
            let declared_in = scratch.join(file);
            module_directory(&inventory, &declared_in) != Ok(by_stem(&declared_in))
        })
        .count();
    assert_eq!(
        disagreements, 2,
        "the manifest and the stem rule must disagree on the arbitrary bin path and on the \
         nested `lib.rs`, or this control measures nothing"
    );

    let missing = scratch.join("no-such-package");
    assert!(
        matches!(
            crate_roots_of(&missing),
            Err(InventoryRefusal::Failed { .. })
        ),
        "a manifest that does not exist is a refusal, not an empty inventory"
    );
    let manifest = scratch.join("Cargo.toml");
    let refusals: Vec<InventoryRefusal> = [
        "this is not json",
        "{}",
        "{\"packages\":[]}",
        "{\"packages\":[{\"manifest_path\":\"/somewhere/else/Cargo.toml\",\"targets\":[{\"src_path\":\"/somewhere/else/src/lib.rs\"}]}]}",
        "{\"packages\":[{\"manifest_path\":\"PLACEHOLDER\",\"targets\":[]}]}",
        "{\"packages\":[{\"manifest_path\":\"PLACEHOLDER\",\"targets\":[{\"name\":\"x\"}]}]}",
    ]
    .into_iter()
    .map(|document| {
        let document = document.replace(
            "PLACEHOLDER",
            &manifest.display().to_string().replace('\\', "\\\\"),
        );
        CrateRoots::from_metadata_json(&document, &manifest).expect_err("this document is refused")
    })
    .collect();
    assert!(
        matches!(refusals[0], InventoryRefusal::Unreadable { .. }),
        "{:?}",
        refusals[0]
    );
    assert!(
        matches!(refusals[1], InventoryRefusal::Unreadable { .. }),
        "{:?}",
        refusals[1]
    );
    assert!(
        matches!(refusals[2], InventoryRefusal::NoPackage { .. }),
        "{:?}",
        refusals[2]
    );
    assert!(
        matches!(refusals[3], InventoryRefusal::NoPackage { .. }),
        "a document describing a different package is refused rather than adopted: {:?}",
        refusals[3]
    );
    assert!(
        matches!(refusals[4], InventoryRefusal::NoTargets { .. }),
        "{:?}",
        refusals[4]
    );
    assert!(
        matches!(refusals[5], InventoryRefusal::Unreadable { .. }),
        "a target with no `src_path` is unreadable rather than skipped: {:?}",
        refusals[5]
    );
    for refusal in &refusals {
        assert!(
            refusal.to_string().contains("cargo metadata")
                || refusal.to_string().contains("declares no target"),
            "the refusal names the authority it could not reach: {refusal}"
        );
    }

    let live = crate::effects::tests::crate_roots();
    assert_eq!(live.package_dir(), repo_root().as_path());
    assert_eq!(
        live.roots().collect::<Vec<_>>(),
        vec![
            repo_root().join("examples/probe.rs").as_path(),
            repo_root().join("src/lib.rs").as_path(),
            repo_root().join("src/main.rs").as_path(),
        ],
        "this package's exact target inventory"
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn the_file_level_lint_reader_is_a_census_instrument_and_not_a_shipped_api() {
    fn absent_from_production(source: &str) -> Vec<String> {
        let production = crate::effects::production_code(source);
        let whole = blank_comments_and_strings(source);
        let mut wrong = Vec::new();
        for needle in [
            "fn file_level_lint_state(",
            "fn what_a_lint_path_names(",
            "mod lint_levels",
        ] {
            if !whole.contains(needle) {
                wrong.push(format!("`{needle}` is not in src/effects.rs at all"));
            }
            if production.contains(needle) {
                wrong.push(format!(
                    "`{needle}` survives into the production region, which makes it a shipped \
                     surface rather than a census instrument"
                ));
            }
        }
        wrong
    }

    let source = fs::read_to_string(repo_root().join("src/effects.rs")).expect("src/effects.rs");
    assert!(
        absent_from_production(&source).is_empty(),
        "{:#?}",
        absent_from_production(&source)
    );
    let crlf = source.replace('\n', "\r\n");
    assert!(
        absent_from_production(&crlf).is_empty(),
        "{:#?}",
        absent_from_production(&crlf)
    );

    assert!(
        blank_comments_and_strings(&source).contains("pub(crate) mod lint_levels"),
        "the lint reader's module is no longer `pub(crate)`"
    );
    assert!(
        !blank_comments_and_strings(&source).contains("pub mod lint_levels"),
        "the lint reader's module is `pub`, which is the surface this repair removed"
    );

    for prologue in [
        "#![deny(clippy::disallowed_types)]\n",
        "#![deny(clippy::disallowed_types)]\r\n",
        "//! docs\r\n#![allow(clippy::too_many_arguments)]\r\n#![forbid(clippy::disallowed_macros)]\r\n",
    ] {
        let wanted = if prologue.contains("forbid") {
            ("clippy::disallowed_macros", Some("forbid"))
        } else {
            ("clippy::disallowed_types", Some("deny"))
        };
        assert_eq!(
            crate::effects::lint_levels::file_level_lint_state(prologue, wanted.0),
            wanted.1,
            "{prologue:?}"
        );
    }
}

#[test]
fn the_file_level_lint_reader_answers_what_rustc_does() {
    use crate::effects::lint_levels::{
        Resolution, file_level_lint_resolution, file_level_lint_worlds,
    };

    const GOVERNED: [(&str, &str); 3] = [
        (
            "clippy::disallowed_methods",
            "pub fn go(p: &std::path::Path) { let _ = std::fs::write(p, \"x\"); }\n",
        ),
        (
            "clippy::disallowed_types",
            "pub fn go() -> Option<std::process::Command> { None }\n",
        ),
        (
            "clippy::disallowed_macros",
            "pub fn go() { eprintln!(\"x\"); }\n",
        ),
    ];

    fn predict_world(
        level: Option<&'static str>,
        refused_downgrade: bool,
    ) -> (bool, Vec<&'static str>, bool) {
        if refused_downgrade {
            return (false, Vec::new(), true);
        }
        match level {
            Some("allow" | "expect") => (true, Vec::new(), false),
            None | Some("warn") => (true, vec!["warning"], false),
            Some("deny" | "forbid") => (false, vec!["error"], false),
            other => panic!("the reader answered `{other:?}`, which nothing predicts"),
        }
    }

    fn predict(resolution: Resolution) -> (bool, Vec<&'static str>, bool) {
        assert!(
            !resolution.undecided,
            "a row of the decided table left the reader undecided: {resolution:?}"
        );
        predict_world(resolution.level, resolution.refused_downgrade)
    }

    enum Wants {
        Predicted(Resolution),
        Builds(bool),
        AnotherLintsOldName(Resolution),
    }

    fn starts_the_file(prologue: &str) -> bool {
        prologue.starts_with('\u{feff}')
            || (prologue.starts_with("#!") && !prologue.starts_with("#!["))
    }

    fn judge(
        lint: &str,
        tag: &str,
        wants: &Wants,
        (built, fired, rejected, diagnostics): (bool, Vec<String>, bool, Vec<(String, String)>),
        observed_shapes: &mut BTreeSet<(bool, Vec<String>, bool)>,
    ) {
        match *wants {
            Wants::Predicted(resolution) => {
                let (wants_build, wants_fired, wants_rejected) = predict(resolution);
                assert_eq!(
                    (built, fired.clone(), rejected),
                    (
                        wants_build,
                        wants_fired
                            .iter()
                            .map(|level| (*level).to_owned())
                            .collect(),
                        wants_rejected
                    ),
                    "`{tag}` for `{lint}` — the reader answered {resolution:?} and clippy-driver \
                     did something else: built={built} fired={fired:?} E0453={rejected}; all \
                     diagnostics {diagnostics:?}"
                );
                observed_shapes.insert((built, fired, rejected));
            }
            Wants::Builds(builds) => assert_eq!(
                built, builds,
                "`{tag}` for `{lint}`: clippy-driver did built={built} fired={fired:?} \
                 E0453={rejected}; all diagnostics {diagnostics:?}"
            ),
            Wants::AnotherLintsOldName(resolution) => assert!(
                resolution
                    == Resolution {
                        level: Some("deny"),
                        refused_downgrade: false,
                        undecided: false,
                    }
                    && !built
                    && fired == ["error"],
                "`{tag}` for `{lint}`: another lint's old name lowers nothing: \
                 {resolution:?}, built={built} fired={fired:?}; all diagnostics \
                 {diagnostics:?}"
            ),
        }
    }

    let table: &[(&str, &str)] = &[
        ("bare", ""),
        ("allow", "#![allow(clippy::disallowed_methods)]\n"),
        ("warn", "#![warn(clippy::disallowed_methods)]\n"),
        ("deny", "#![deny(clippy::disallowed_methods)]\n"),
        ("forbid", "#![forbid(clippy::disallowed_methods)]\n"),
        ("expect", "#![expect(clippy::disallowed_methods)]\n"),
        (
            "deny_then_allow",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "allow_then_deny",
            "#![allow(clippy::disallowed_methods)]\n#![deny(clippy::disallowed_methods)]\n",
        ),
        (
            "deny_then_warn",
            "#![deny(clippy::disallowed_methods)]\n#![warn(clippy::disallowed_methods)]\n",
        ),
        (
            "deny_then_expect",
            "#![deny(clippy::disallowed_methods)]\n#![expect(clippy::disallowed_methods)]\n",
        ),
        (
            "allow_warn_deny",
            "#![allow(clippy::disallowed_methods)]\n#![warn(clippy::disallowed_methods)]\n\
             #![deny(clippy::disallowed_methods)]\n",
        ),
        (
            "allow_then_forbid",
            "#![allow(clippy::disallowed_methods)]\n#![forbid(clippy::disallowed_methods)]\n",
        ),
        (
            "forbid_then_allow",
            "#![forbid(clippy::disallowed_methods)]\n#![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "forbid_then_warn",
            "#![forbid(clippy::disallowed_methods)]\n#![warn(clippy::disallowed_methods)]\n",
        ),
        (
            "forbid_then_deny",
            "#![forbid(clippy::disallowed_methods)]\n#![deny(clippy::disallowed_methods)]\n",
        ),
        (
            "deny_then_allow_bare",
            "#![deny(clippy::disallowed_methods)]\n#![allow(disallowed_methods)]\n",
        ),
        (
            "cfg_attr_not_test_forbid",
            "#![cfg_attr(not(test), forbid(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_not_test_deny",
            "#![cfg_attr(not(test), deny(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_test_allow",
            "#![cfg_attr(test, allow(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_test_forbid",
            "#![cfg_attr(test, forbid(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_not_test_forbid_then_allow",
            "#![cfg_attr(not(test), forbid(clippy::disallowed_methods))]\n\
             #![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "allow_then_cfg_attr_not_test_forbid",
            "#![allow(clippy::disallowed_methods)]\n\
             #![cfg_attr(not(test), forbid(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_not_test_two_attributes",
            "#![cfg_attr(not(test), allow(dead_code), forbid(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_not_test_deny_then_nested_allow",
            "#![cfg_attr(not(test), deny(clippy::disallowed_methods), \
             cfg_attr(not(test), allow(clippy::disallowed_methods)))]\n",
        ),
        (
            "cfg_attr_not_test_forbid_then_nested_allow",
            "#![cfg_attr(not(test), forbid(clippy::disallowed_methods), \
             cfg_attr(not(test), allow(clippy::disallowed_methods)))]\n",
        ),
        (
            "cfg_attr_nested_not_test_forbid",
            "#![cfg_attr(not(test), cfg_attr(not(test), forbid(clippy::disallowed_methods)))]\n",
        ),
        (
            "deny_then_cfg_attr_not_test_nested_test_allow",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(not(test), cfg_attr(test, allow(clippy::disallowed_methods)))]\n",
        ),
        (
            "deny_then_cfg_attr_test_nested_not_test_allow",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(test, cfg_attr(not(test), allow(clippy::disallowed_methods)))]\n",
        ),
        (
            "cfg_attr_not_test_deny_and_allow_in_one",
            "#![cfg_attr(not(test), deny(clippy::disallowed_methods), \
             allow(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_test_deny_then_allow",
            "#![cfg_attr(test, deny(clippy::disallowed_methods))]\n\
             #![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "cfg_attr_all_not_test_forbid",
            "#![cfg_attr(all(not(test)), forbid(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_any_test_or_not_test_forbid",
            "#![cfg_attr(any(test, not(test)), forbid(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_not_not_test_forbid",
            "#![cfg_attr(not(not(test)), forbid(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_all_empty_forbid",
            "#![cfg_attr(all(), forbid(clippy::disallowed_methods))]\n",
        ),
        (
            "cfg_attr_any_empty_forbid",
            "#![cfg_attr(any(), forbid(clippy::disallowed_methods))]\n",
        ),
        (
            "deny_then_cfg_attr_any_empty_allow",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(any(), allow(clippy::disallowed_methods))]\n",
        ),
        (
            "same_value_twice_is_one_condition",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(feature = \"x\", allow(clippy::disallowed_methods))]\n\
             #![cfg_attr(feature = \"x\", deny(clippy::disallowed_methods))]\n",
        ),
        (
            "reason_that_spells_the_lint",
            "#![deny(dead_code, reason = \"clippy::disallowed_methods, disallowed_methods\")]\n",
        ),
        (
            "prose_decoy",
            "//! `#![allow(clippy::disallowed_methods)]` is written here in prose.\n\
             #![deny(clippy::disallowed_methods)]\n",
        ),
        (
            "attribute_after_the_prologue",
            "#![deny(clippy::disallowed_methods)]\npub const S: &str = \
             \"#![allow(clippy::disallowed_methods)]\";\n",
        ),
        (
            "raw_allow",
            "#![deny(clippy::disallowed_methods)]\n#![r#allow(clippy::disallowed_methods)]\n",
        ),
        (
            "raw_cfg_attr",
            "#![deny(clippy::disallowed_methods)]\n\
             #![r#cfg_attr(not(test), allow(clippy::disallowed_methods))]\n",
        ),
        (
            "nested_raw_allow",
            "#![cfg_attr(not(test), deny(clippy::disallowed_methods), \
             r#allow(clippy::disallowed_methods))]\n",
        ),
        (
            "raw_tool_name",
            "#![deny(clippy::disallowed_methods)]\n#![allow(r#clippy::disallowed_methods)]\n",
        ),
        (
            "raw_forbid_then_allow",
            "#![r#forbid(clippy::disallowed_methods)]\n#![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "spaced_path",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy :: disallowed_methods)]\n",
        ),
        (
            "spaced_bang_deny",
            "# ![deny(clippy::disallowed_methods)]\n",
        ),
        (
            "spaced_bracket_forbid",
            "#! [forbid(clippy::disallowed_methods)]\n",
        ),
        (
            "commented_tokens_deny",
            "#/* a */!/* b */[deny(clippy::disallowed_methods)]\n",
        ),
        (
            "bang_and_bracket_on_two_lines_forbid_then_allow",
            "#!\n[forbid(clippy::disallowed_methods)]\n#![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "allow_clippy_all",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::all)]\n",
        ),
        (
            "allow_clippy_style",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::style)]\n",
        ),
        (
            "allow_prefixless_all",
            "#![deny(clippy::disallowed_methods)]\n#![allow(all)]\n",
        ),
        (
            "allow_prefixless_style",
            "#![deny(clippy::disallowed_methods)]\n#![allow(style)]\n",
        ),
        (
            "expect_clippy_all",
            "#![deny(clippy::disallowed_methods)]\n#![expect(clippy::all)]\n",
        ),
        (
            "warn_clippy_style",
            "#![deny(clippy::disallowed_methods)]\n#![warn(clippy::style)]\n",
        ),
        (
            "allow_then_deny_clippy_all",
            "#![allow(clippy::disallowed_methods)]\n#![deny(clippy::all)]\n",
        ),
        (
            "groups_without_the_lint",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::restriction, \
             clippy::pedantic, clippy::nursery, clippy::cargo, clippy::complexity, \
             clippy::correctness, clippy::perf, clippy::suspicious, restriction)]\n",
        ),
        (
            "forbid_by_a_group_then_allow",
            "#![forbid(clippy::all)]\n#![allow(clippy::disallowed_methods)]\n",
        ),
        ("forbid_by_a_group_alone", "#![forbid(clippy::style)]\n"),
        (
            "forbid_by_a_group_then_deny",
            "#![forbid(clippy::all)]\n#![deny(clippy::disallowed_methods)]\n",
        ),
        (
            "forbid_by_a_group_then_warn",
            "#![forbid(clippy::all)]\n#![warn(clippy::disallowed_methods)]\n",
        ),
        (
            "forbid_then_forbid_by_a_group_then_allow",
            "#![forbid(clippy::disallowed_methods)]\n#![forbid(clippy::all)]\n\
             #![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "forbid_by_a_group_then_forbid_then_allow",
            "#![forbid(clippy::all)]\n#![forbid(clippy::disallowed_methods)]\n\
             #![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "forbid_and_a_group_in_one_list_then_allow",
            "#![forbid(clippy::disallowed_methods, clippy::all)]\n\
             #![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "forbid_then_allow_by_a_group",
            "#![forbid(clippy::disallowed_methods)]\n#![allow(clippy::all)]\n",
        ),
        (
            "forbid_by_a_prefixless_group_then_allow",
            "#![forbid(all)]\n#![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "deny_then_allow_warnings",
            "#![deny(clippy::disallowed_methods)]\n#![allow(warnings)]\n",
        ),
        (
            "forbid_then_allow_warnings",
            "#![forbid(clippy::disallowed_methods)]\n#![allow(warnings)]\n",
        ),
        (
            "inner_doc_comment_between",
            "#![deny(clippy::disallowed_methods)]\n//! doc\n#![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "inner_block_doc_comment_between",
            "#![deny(clippy::disallowed_methods)]\n/*! doc */\n\
             #![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "byte_order_mark",
            "\u{feff}#![deny(clippy::disallowed_methods)]\n",
        ),
        (
            "shebang",
            "#!/usr/bin/env run-cargo-script\n#![deny(clippy::disallowed_methods)]\n",
        ),
        (
            "shebang_whose_next_token_is_a_doc_comment",
            "#!/** d */[allow(clippy::disallowed_methods)]\n#![deny(clippy::disallowed_methods)]\n",
        ),
        (
            "trailing_comma",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::disallowed_methods,)]\n",
        ),
        (
            "raw_reason",
            "#![deny(clippy::disallowed_methods)]\n\
             #![allow(clippy::disallowed_methods, r#reason = \"x\")]\n",
        ),
        (
            "empty_list",
            "#![deny(clippy::disallowed_methods)]\n#![allow()]\n",
        ),
        (
            "names_no_lint_resolves",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::no_such_lint_anywhere, \
             clippy::assign_ops, clippy::DISALLOWED_METHODS, rustdoc::disallowed_methods, \
             rustc::disallowed_methods)]\n",
        ),
        (
            "spaced_bang_allow",
            "#![deny(clippy::disallowed_methods)]\n# ![allow(clippy::disallowed_methods)]\n",
        ),
        (
            "spaced_bracket_allow",
            "#![deny(clippy::disallowed_methods)]\n#! [allow(clippy::disallowed_methods)]\n",
        ),
        (
            "commented_bracket_allow",
            "#![deny(clippy::disallowed_methods)]\n#!/**/[allow(clippy::disallowed_methods)]\n",
        ),
        (
            "comment_separated_allow",
            "#![deny(clippy::disallowed_methods)]\n\
             #/**/!/**/[allow(clippy::disallowed_methods)]\n",
        ),
        (
            "bang_and_bracket_on_two_lines_allow",
            "#![deny(clippy::disallowed_methods)]\n#!\n[allow(clippy::disallowed_methods)]\n",
        ),
        (
            "spaced_keyword_allow",
            "#![deny(clippy::disallowed_methods)]\n#![allow (clippy::disallowed_methods)]\n",
        ),
        (
            "separated_by_marks_allow",
            "#![deny(clippy::disallowed_methods)]\n\
             #\u{200E}!\u{2029}[allow\u{000B}(clippy::disallowed_methods)]\n",
        ),
        (
            "raw_lint_name_allow",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::r#disallowed_methods)]\n",
        ),
        (
            "raw_group_allow",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::r#all)]\n",
        ),
        (
            "prefixless_group_alias_allow",
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy_all)]\n",
        ),
        (
            "prefixless_group_alias_expect",
            "#![deny(clippy::disallowed_methods)]\n#![expect(clippy_style)]\n",
        ),
    ];

    let unread: &[(&str, &str, bool)] = &[
        (
            "warnings_lowered_under_warn",
            "#![warn(clippy::disallowed_methods)]\n#![allow(warnings)]\n",
            true,
        ),
        ("warnings_lowered_alone", "#![allow(warnings)]\n", true),
        ("warnings_raised_alone", "#![deny(warnings)]\n", false),
        (
            "three_segment_path",
            "#![deny(clippy::disallowed_methods)]\n\
             #![allow(clippy::disallowed_methods::x)]\n",
            false,
        ),
        (
            "doc_comment_inside",
            "#![deny(clippy::disallowed_methods)]\n\
             #![allow(/** d */ clippy::disallowed_methods)]\n",
            false,
        ),
        (
            "doc_comment_between_hash_and_bang",
            "#![deny(clippy::disallowed_methods)]\n#/** d */![allow(clippy::disallowed_methods)]\n",
            false,
        ),
        (
            "outer_doc_comment_before_an_inner_attribute",
            "/// doc\n#![deny(clippy::disallowed_methods)]\n",
            false,
        ),
        (
            "custom_inner_attribute",
            "#![deny(clippy::disallowed_methods)]\n#![clippy::allow(clippy::disallowed_methods)]\n",
            false,
        ),
        (
            "brackets_for_parentheses",
            "#![deny(clippy::disallowed_methods)]\n#![allow[clippy::disallowed_methods]]\n",
            false,
        ),
        (
            "literal_entry",
            "#![deny(clippy::disallowed_methods)]\n#![allow(\"clippy::disallowed_methods\")]\n",
            false,
        ),
        (
            "name_value_entry",
            "#![deny(clippy::disallowed_methods)]\n\
             #![allow(clippy::disallowed_methods = \"x\")]\n",
            false,
        ),
        (
            "reason_before_the_lint",
            "#![deny(clippy::disallowed_methods)]\n\
             #![allow(reason = \"r\", clippy::disallowed_methods)]\n",
            false,
        ),
        (
            "unknown_tool",
            "#![deny(clippy::disallowed_methods)]\n#![allow(footool::disallowed_methods)]\n",
            false,
        ),
        (
            "leading_path_separator",
            "#![deny(clippy::disallowed_methods)]\n#![allow(::clippy::disallowed_methods)]\n",
            false,
        ),
        (
            "hash_bang_opening_no_attribute",
            "#![deny(clippy::disallowed_methods)]\n#!/bin/sh\n",
            false,
        ),
    ];

    let undecided: &[(&str, &str, bool)] = &[
        (
            "deny_then_cfg_attr_unix_allow",
            "#![deny(clippy::disallowed_methods)]\n#![cfg_attr(unix, allow(clippy::disallowed_methods))]\n",
            true,
        ),
        (
            "deny_then_cfg_attr_windows_allow",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(windows, allow(clippy::disallowed_methods))]\n",
            true,
        ),
        (
            "cfg_attr_unix_forbid",
            "#![cfg_attr(unix, forbid(clippy::disallowed_methods))]\n",
            true,
        ),
        (
            "cfg_attr_unix_forbid_then_deny",
            "#![cfg_attr(unix, forbid(clippy::disallowed_methods))]\n\
             #![deny(clippy::disallowed_methods)]\n",
            true,
        ),
        (
            "forbid_then_cfg_attr_unix_allow",
            "#![forbid(clippy::disallowed_methods)]\n\
             #![cfg_attr(unix, allow(clippy::disallowed_methods))]\n",
            true,
        ),
        (
            "cfg_attr_feature_forbid",
            "#![cfg_attr(feature = \"not(test)\", forbid(clippy::disallowed_methods))]\n",
            true,
        ),
        (
            "cfg_attr_nested_unix_forbid",
            "#![cfg_attr(not(test), cfg_attr(unix, forbid(clippy::disallowed_methods)))]\n",
            true,
        ),
        (
            "cfg_attr_missing_predicate",
            "#![cfg_attr(, forbid(clippy::disallowed_methods))]\n",
            false,
        ),
        (
            "cfg_attr_split_token_predicate",
            "#![cfg_attr(not(te st), forbid(clippy::disallowed_methods))]\n",
            false,
        ),
        (
            "cfg_attr_not_with_two_arguments",
            "#![cfg_attr(not(test, unix), forbid(clippy::disallowed_methods))]\n",
            false,
        ),
        (
            "cfg_attr_unbalanced_predicate",
            "#![cfg_attr(not(test, forbid(clippy::disallowed_methods))]\n",
            false,
        ),
    ];
    let valued: &[(&str, &str, &[&str])] = &[
        (
            "target_os_values",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(target_os = \"linux\", allow(clippy::disallowed_methods))]\n\
             #![cfg_attr(target_os = \"windows\", deny(clippy::disallowed_methods))]\n",
            &[],
        ),
        (
            "target_os_values_nested",
            "#![cfg_attr(not(test), deny(clippy::disallowed_methods), \
             cfg_attr(target_os = \"linux\", allow(clippy::disallowed_methods)), \
             cfg_attr(target_os = \"windows\", deny(clippy::disallowed_methods)))]\n",
            &[],
        ),
        (
            "target_arch_values",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(target_arch = \"x86_64\", allow(clippy::disallowed_methods))]\n\
             #![cfg_attr(target_arch = \"aarch64\", deny(clippy::disallowed_methods))]\n",
            &[],
        ),
        (
            "feature_values",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(feature = \"one\", allow(clippy::disallowed_methods))]\n\
             #![cfg_attr(feature = \"two\", deny(clippy::disallowed_methods))]\n",
            &["feature=\"one\""],
        ),
        (
            "feature_values_reversed",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(feature = \"two\", allow(clippy::disallowed_methods))]\n\
             #![cfg_attr(feature = \"one\", deny(clippy::disallowed_methods))]\n",
            &["feature=\"one\""],
        ),
        (
            "linux_allow_macos_deny",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(target_os = \"linux\", allow(clippy::disallowed_methods))]\n\
             #![cfg_attr(target_os = \"macos\", deny(clippy::disallowed_methods))]\n",
            &[],
        ),
        (
            "linux_expect_macos_deny",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(target_os = \"linux\", expect(clippy::disallowed_methods))]\n\
             #![cfg_attr(target_os = \"macos\", deny(clippy::disallowed_methods))]\n",
            &[],
        ),
        (
            "linux_warn_macos_deny",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(target_os = \"linux\", warn(clippy::disallowed_methods))]\n\
             #![cfg_attr(target_os = \"macos\", deny(clippy::disallowed_methods))]\n",
            &[],
        ),
        (
            "feature_a_allow_b_deny",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(feature = \"a\", allow(clippy::disallowed_methods))]\n\
             #![cfg_attr(feature = \"b\", deny(clippy::disallowed_methods))]\n",
            &["feature=\"a\""],
        ),
        (
            "feature_a_expect_b_deny",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(feature = \"a\", expect(clippy::disallowed_methods))]\n\
             #![cfg_attr(feature = \"b\", deny(clippy::disallowed_methods))]\n",
            &["feature=\"a\""],
        ),
        (
            "feature_a_warn_b_deny",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(feature = \"a\", warn(clippy::disallowed_methods))]\n\
             #![cfg_attr(feature = \"b\", deny(clippy::disallowed_methods))]\n",
            &["feature=\"a\""],
        ),
        (
            "x86_64_allow_aarch64_deny",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(target_arch = \"x86_64\", allow(clippy::disallowed_methods))]\n\
             #![cfg_attr(target_arch = \"aarch64\", deny(clippy::disallowed_methods))]\n",
            &[],
        ),
        (
            "x86_64_expect_aarch64_deny",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(target_arch = \"x86_64\", expect(clippy::disallowed_methods))]\n\
             #![cfg_attr(target_arch = \"aarch64\", deny(clippy::disallowed_methods))]\n",
            &[],
        ),
        (
            "x86_64_warn_aarch64_deny",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(target_arch = \"x86_64\", warn(clippy::disallowed_methods))]\n\
             #![cfg_attr(target_arch = \"aarch64\", deny(clippy::disallowed_methods))]\n",
            &[],
        ),
        (
            "escaped_quote_values",
            "#![deny(clippy::disallowed_methods)]\n\
             #![cfg_attr(feature = \"a\\\", b\", allow(clippy::disallowed_methods))]\n\
             #![cfg_attr(feature = \"a\\\", c\", deny(clippy::disallowed_methods))]\n",
            &["feature=\"a\\\", b\""],
        ),
    ];
    let scratch = scratch_dir("levels");
    let mut observed_shapes: BTreeSet<(bool, Vec<String>, bool)> = BTreeSet::new();
    for (lint, body) in GOVERNED {
        let bare = normalize_lint(lint).expect("a governed lint");
        let spelled = |text: &str| text.replace("disallowed_methods", bare);
        let read = |built: bool, diagnostics: Vec<(String, String)>| {
            let fired: Vec<String> = diagnostics
                .iter()
                .filter(|(_, code)| code == lint)
                .map(|(level, _)| level.clone())
                .collect();
            let rejected = diagnostics.iter().any(|(_, code)| code == "E0453");
            (built, fired, rejected, diagnostics)
        };
        let outcome = |tag: &str, source: &str, cfgs: &[&str]| {
            let (built, diagnostics) =
                clippy_outcome(&scratch, &format!("{tag}_{bare}"), source, cfgs);
            read(built, diagnostics)
        };
        let mut passes: Vec<(String, String, Wants)> = Vec::new();
        let mut refused: Vec<(String, String, Wants)> = Vec::new();
        let mut alone: Vec<(String, String, Wants)> = Vec::new();

        for (tag, prologue) in table {
            let source = format!("{}{body}", spelled(prologue));
            let resolution = file_level_lint_resolution(&source, lint);
            assert_eq!(
                file_level_lint_resolution(&source.replace('\n', "\r\n"), lint),
                resolution,
                "`{tag}` for `{lint}` reads differently under CRLF"
            );
            let row = ((*tag).to_owned(), source, Wants::Predicted(resolution));
            if starts_the_file(prologue) {
                alone.push(row);
            } else if predict(resolution).2 {
                refused.push(row);
            } else {
                passes.push(row);
            }
        }

        let every_undecided = undecided
            .iter()
            .map(|(tag, prologue, compiles)| (*tag, *prologue, *compiles, &[][..]))
            .chain(
                valued
                    .iter()
                    .map(|(tag, prologue, cfgs)| (*tag, *prologue, true, *cfgs)),
            );
        for (tag, prologue, compiles, cfgs) in every_undecided {
            let source = format!("{}{body}", spelled(prologue));
            let resolution = file_level_lint_resolution(&source, lint);
            assert!(
                resolution.level.is_none() && !resolution.refused_downgrade,
                "`{tag}` for `{lint}`: the reader claimed a level no single production valuation \
                 decides: {resolution:?}"
            );
            assert!(
                resolution.undecided || !compiles,
                "`{tag}` for `{lint}`: a prologue rustc compiles under one valuation and not \
                 another is undecided, not silently one of them: {resolution:?}"
            );
            assert_eq!(
                file_level_lint_resolution(&source.replace('\n', "\r\n"), lint),
                resolution,
                "`{tag}` for `{lint}` reads differently under CRLF"
            );
            let worlds = file_level_lint_worlds(&source, lint);
            let (built, fired, rejected, diagnostics) = outcome(tag, &source, cfgs);
            if compiles {
                assert!(
                    worlds.len() > 1,
                    "`{tag}` for `{lint}`: the reader refused to decide a prologue every \
                     valuation agrees on: {worlds:?}"
                );
                assert!(
                    worlds.iter().any(|&(level, refused_downgrade)| {
                        predict_world(level, refused_downgrade)
                            == (built, fired.iter().map(String::as_str).collect(), rejected)
                    }),
                    "`{tag}` for `{lint}`: clippy-driver on this host with {cfgs:?} did \
                     built={built} fired={fired:?} E0453={rejected}, which no production \
                     valuation the reader enumerated predicts: {worlds:?}; all diagnostics \
                     {diagnostics:?}"
                );
            } else {
                assert!(
                    !built,
                    "`{tag}` for `{lint}`: clippy-driver compiled a predicate the reader could \
                     not read; the reader's refusal would have hidden a level: all diagnostics \
                     {diagnostics:?}"
                );
            }
        }

        for (tag, prologue, builds) in unread {
            let source = format!("{}{body}", spelled(prologue));
            let resolution = file_level_lint_resolution(&source, lint);
            assert!(
                resolution.undecided
                    && resolution.level.is_none()
                    && !resolution.refused_downgrade
                    && file_level_lint_worlds(&source, lint).is_empty(),
                "`{tag}` for `{lint}`: the prologue states or changes the lint in a way no census \
                 here reads or rustc refuses, and the reader claimed an answer: {resolution:?}"
            );
            assert_eq!(
                file_level_lint_resolution(&source.replace('\n', "\r\n"), lint),
                resolution,
                "`{tag}` for `{lint}` reads differently under CRLF"
            );
            let row = ((*tag).to_owned(), source, Wants::Builds(*builds));
            if *builds && !starts_the_file(prologue) {
                passes.push(row);
            } else {
                alone.push(row);
            }
        }

        for (renamed, to) in [
            ("disallowed_method", "disallowed_methods"),
            ("disallowed_type", "disallowed_types"),
        ] {
            let source = format!("#![deny({lint})]\n#![allow(clippy::{renamed})]\n{body}");
            let resolution = file_level_lint_resolution(&source, lint);
            let wants = if to == bare {
                Wants::Predicted(resolution)
            } else {
                Wants::AnotherLintsOldName(resolution)
            };
            passes.push((format!("renamed_{renamed}"), source, wants));
        }

        for (batch, rows) in [("passes", &passes), ("refused", &refused)] {
            let sources: Vec<&str> = rows.iter().map(|(_, source, _)| source.as_str()).collect();
            let outcomes = clippy_outcomes(&scratch, &format!("{bare}_{batch}"), &sources, &[]);
            for ((tag, _, wants), (built, diagnostics)) in rows.iter().zip(outcomes) {
                judge(
                    lint,
                    tag,
                    wants,
                    read(built, diagnostics),
                    &mut observed_shapes,
                );
            }
        }
        for (tag, source, wants) in &alone {
            judge(
                lint,
                tag,
                wants,
                outcome(tag, source, &[]),
                &mut observed_shapes,
            );
        }

        let deny_then_allow = spelled(
            "#![deny(clippy::disallowed_methods)]\n#![allow(clippy::disallowed_methods)]\n",
        );
        assert_eq!(
            file_level_lint_resolution(&format!("{deny_then_allow}{body}"), lint),
            Resolution {
                level: Some("allow"),
                refused_downgrade: false,
                undecided: false,
            },
            "deny then allow is effectively allow"
        );
        let forbid_then_allow = spelled(
            "#![forbid(clippy::disallowed_methods)]\n#![allow(clippy::disallowed_methods)]\n",
        );
        assert_eq!(
            file_level_lint_resolution(&format!("{forbid_then_allow}{body}"), lint),
            Resolution {
                level: Some("forbid"),
                refused_downgrade: true,
                undecided: false,
            },
            "a forbid cannot be weakened; the attempt is E0453 and not a level"
        );
    }

    assert!(
        observed_shapes.len() >= 4,
        "the fixtures produced only {} distinct compiler outcomes: {observed_shapes:?}",
        observed_shapes.len()
    );

    let mut restated = Vec::new();
    for (path, source) in scanned_sources() {
        let blanked = blank_comments_and_strings(&source);
        for lint in USED_GOVERNED_LINTS {
            let bare = normalize_lint(lint).expect("a governed lint");
            let stated = blanked
                .split("#![")
                .skip(1)
                .filter(|attribute| {
                    attribute
                        .split(']')
                        .next()
                        .is_some_and(|body| body.contains(bare))
                })
                .count();
            if stated > 1 {
                restated.push(format!(
                    "{path} states `{lint}` in {stated} inner attributes"
                ));
            }
        }
    }
    assert!(
        restated.is_empty(),
        "the ordered reading is exercised by fixtures only while this holds: {restated:#?}"
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn the_production_code_region_removes_a_configured_item_and_keeps_the_rest() {
    oracles::the_configured_item_is_removed_and_the_rest_kept();
}

#[test]
fn the_production_code_region_excludes_typed_test_functions() {
    oracles::typed_test_functions_are_removed_and_later_code_is_kept();
}

#[test]
fn a_configured_attribute_in_prose_removes_nothing() {
    oracles::a_configured_attribute_in_prose_is_inert();
}

#[test]
fn the_production_code_region_contains_the_truncated_one() {
    oracles::the_whole_region_contains_the_truncated_one();
}

#[test]
fn every_production_region_that_stops_early_stops_at_a_module() {
    oracles::every_early_stop_is_at_a_module();
}

#[test]
fn every_pr6_refusal_st16_variant_and_invariant_clause_names_a_test_or_an_owner() {
    mappings::every_promised_mapping_names_a_test_or_an_owner();
}
