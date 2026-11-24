//! Laying the repository's settings over the machine's (DESIGN.md §17).
//!
//! Settings have two scopes. The machine-level file, `upstroke.toml` beside the
//! pools file (`~/.upstroke/upstroke.toml`), holds what an operator wants in
//! every repository on the machine; the repository's own `upstroke.toml` holds
//! what that repository needs. Both are the same schema, and the repository
//! wins wherever both speak.
//!
//! The merge is over TOML tables, before either file is interpreted, and has
//! two rules:
//!
//! - **a table merges key by key**, recursively, so `[engine] max_per_pool`
//!   from the machine and `[engine] shell` from the repository both stand;
//! - **anything else replaces whole**: a scalar, and an array. `[[gates]]`,
//!   `[[pins]]` and `[[routing.overrides]]` are arrays, so a repository that
//!   writes any `[[gates]]` runs exactly its own and none of the machine's.
//!   Appending would make a repository's gate list depend on a file outside
//!   it, with no way for the repository to take one away.
//!
//! Everything here is a pure function over parsed values.

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::path::Path;

use crate::error::UpstrokeError;

/// `over` laid on `base`: tables merge, everything else replaces.
pub(super) fn merge(mut base: toml::Table, over: toml::Table) -> toml::Table {
    for (key, value) in over {
        let merged = match (base.remove(&key), value) {
            (Some(toml::Value::Table(beneath)), toml::Value::Table(above)) => {
                toml::Value::Table(merge(beneath, above))
            }
            (_, above) => above,
        };
        base.insert(key, merged);
    }
    base
}

/// The dotted keys `base` supplies to the merge: every leaf of `base` that
/// `over` leaves alone, in `base`'s own (sorted) order.
///
/// This is what the loader reports, so an operator reading a run's warnings
/// can see which of its settings came from outside the repository. An array
/// is one leaf, because it is replaced as one.
pub(super) fn supplied(base: &toml::Table, over: &toml::Table) -> Vec<String> {
    let mut out = Vec::new();
    collect(base, Some(over), "", &mut out);
    out
}

fn collect(base: &toml::Table, over: Option<&toml::Table>, prefix: &str, out: &mut Vec<String>) {
    for (key, value) in base {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match (value, over.and_then(|over| over.get(key))) {
            (toml::Value::Table(beneath), None) => collect(beneath, None, &path, out),
            (toml::Value::Table(beneath), Some(toml::Value::Table(above))) => {
                collect(beneath, Some(above), &path, out);
            }
            (_, None) => out.push(path),
            (_, Some(_)) => {}
        }
    }
}

/// Whether two paths name one file.
///
/// The machine-level settings file is looked for beside the pools file, and a
/// pools file kept in a repository's own root puts it at the repository's
/// `upstroke.toml`. That is one file, not two layers, and reading it twice
/// would report every key in it as machine-level. Compared as written first,
/// which is all two absent files can be compared by, and then by what the
/// filesystem resolves them to.
pub(super) fn same_file(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        (Ok(_), Err(_)) | (Err(_), Ok(_)) | (Err(_), Err(_)) => false,
    }
}

/// A refusal of the merged settings, saying that a second file is involved.
///
/// A merged configuration is refused under the repository file's name, since
/// the repository's keys are the ones that win. The cause can still be a key
/// the machine-level file supplied, or the two together, and an operator who
/// is told only about the file they are looking at will not find it.
pub(super) fn beneath(error: UpstrokeError, machine: &Path) -> UpstrokeError {
    match error {
        UpstrokeError::Config { path, message } => UpstrokeError::Config {
            path,
            message: format!(
                "{message} (the machine-level settings in {} are applied beneath this file, \
                 and a key it leaves unset is read from there)",
                machine.display()
            ),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> toml::Table {
        toml::from_str(text).expect("a TOML fixture")
    }

    #[test]
    fn a_table_merges_key_by_key_and_the_repository_wins_where_both_speak() {
        let machine = table(
            "[engine]\nmax_parallel = 4\nmax_per_pool = 2\n\n[routing.strategy]\nmode = \"value-max\"\n",
        );
        let repo = table("[engine]\nmax_per_pool = 1\nshell = \"bash\"\n");
        assert_eq!(
            merge(machine, repo),
            table(
                "[engine]\nmax_parallel = 4\nmax_per_pool = 1\nshell = \"bash\"\n\n\
                 [routing.strategy]\nmode = \"value-max\"\n"
            )
        );
    }

    #[test]
    fn an_inline_table_is_a_table_and_merges_like_one() {
        let machine = table("[routing]\nreview = { tier = \"mid\", cross_family = false }\n");
        let repo = table("[routing]\nreview = { timeout_secs = 100 }\n");
        assert_eq!(
            merge(machine, repo),
            table(
                "[routing]\nreview = { tier = \"mid\", cross_family = false, timeout_secs = 100 }\n"
            )
        );
    }

    #[test]
    fn an_array_is_replaced_whole_and_never_appended_to() {
        let machine = table(
            "notify = [\"cli\", \"desktop\"]\n\n[[gates]]\nname = \"lint\"\ncmd = \"make lint\"\n\n\
             [[gates]]\nname = \"test\"\ncmd = \"make test\"\n",
        );
        let repo =
            table("notify = [\"cli\"]\n\n[[gates]]\nname = \"check\"\ncmd = \"cargo check\"\n");
        assert_eq!(merge(machine.clone(), repo.clone()), repo);
        assert_eq!(
            merge(machine.clone(), toml::Table::new()),
            machine,
            "a repository that writes no gates takes the machine's"
        );
    }

    #[test]
    fn a_value_of_another_shape_replaces_what_was_there() {
        let machine = table("[budgets]\nrun_usd = 15.0\n");
        let repo = table("budgets = 3\n");
        assert_eq!(merge(machine.clone(), repo.clone()), repo);
        assert_eq!(
            merge(repo, machine.clone()),
            machine,
            "and the other way round: the layer on top is what stands"
        );
    }

    #[test]
    fn merging_a_file_with_nothing_changes_nothing() {
        let settings = table("[engine]\nmax_parallel = 2\n\n[[pins]]\ntier = \"mid\"\n");
        assert_eq!(merge(settings.clone(), toml::Table::new()), settings);
        assert_eq!(merge(toml::Table::new(), settings.clone()), settings);
        assert_eq!(merge(settings.clone(), settings.clone()), settings);
    }

    #[test]
    fn supplied_names_exactly_the_leaves_the_repository_leaves_alone() {
        let machine = table(
            "[engine]\nmax_parallel = 4\nmax_per_pool = 2\n\n[routing.strategy]\n\
             mode = \"value-max\"\nspend_down_after = 0.7\n\n[budgets]\nrun_usd = 15.0\n\n\
             [[gates]]\nname = \"test\"\ncmd = \"make test\"\n",
        );
        let repo = table(
            "[engine]\nmax_per_pool = 1\n\n[routing.strategy]\nmode = \"conserve\"\n\n\
             [[gates]]\nname = \"check\"\ncmd = \"cargo check\"\n",
        );
        assert_eq!(
            supplied(&machine, &repo),
            vec![
                "budgets.run_usd",
                "engine.max_parallel",
                "routing.strategy.spend_down_after",
            ]
        );
        assert_eq!(
            supplied(&machine, &toml::Table::new()),
            vec![
                "budgets.run_usd",
                "engine.max_parallel",
                "engine.max_per_pool",
                "gates",
                "routing.strategy.mode",
                "routing.strategy.spend_down_after",
            ]
        );
        assert!(supplied(&machine, &machine).is_empty());
        assert!(supplied(&toml::Table::new(), &repo).is_empty());
    }

    #[test]
    fn supplied_agrees_with_merge_about_what_the_machine_file_decided() {
        let machine = table(
            "[engine]\nmax_parallel = 4\n\n[routing]\nreview = { tier = \"mid\" }\n\n\
             [budgets]\nrun_usd = 15.0\n",
        );
        let repo =
            table("[routing]\nreview = { timeout_secs = 100 }\n\n[budgets]\nrun_usd = 3.0\n");
        let merged = merge(machine.clone(), repo.clone());
        for key in supplied(&machine, &repo) {
            let lookup = |table: &toml::Table| {
                let mut segments = key.split('.');
                let first = segments.next().expect("a key has a first segment");
                segments.try_fold(table.get(first)?.clone(), |value, segment| {
                    value.get(segment).cloned()
                })
            };
            assert_eq!(
                lookup(&merged),
                lookup(&machine),
                "`{key}` is reported as the machine's, so the merge must carry the machine's value"
            );
            assert_eq!(lookup(&repo), None, "`{key}` is also set by the repository");
        }
    }

    #[test]
    fn two_spellings_of_one_path_are_one_file_and_two_absent_files_are_not() {
        assert!(same_file(
            Path::new("a/upstroke.toml"),
            Path::new("a/upstroke.toml")
        ));
        assert!(!same_file(
            Path::new("upstroke-layers-absent-a/upstroke.toml"),
            Path::new("upstroke-layers-absent-b/upstroke.toml")
        ));
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            same_file(
                &manifest.join("Cargo.toml"),
                &manifest.join("src").join("..").join("Cargo.toml")
            ),
            "a file reached through `..` is the file it resolves to"
        );
        assert!(!same_file(
            &manifest.join("Cargo.toml"),
            &manifest.join("Cargo.lock")
        ));
    }

    #[test]
    fn a_refusal_of_merged_settings_names_the_machine_file_too() {
        let refused = beneath(
            UpstrokeError::Config {
                path: "repo/upstroke.toml".into(),
                message: "`max_parallel = 0` is not a ceiling".to_owned(),
            },
            Path::new("home/.upstroke/upstroke.toml"),
        );
        let text = refused.to_string();
        assert!(text.contains("repo/upstroke.toml"), "{text}");
        assert!(
            text.contains("`max_parallel = 0` is not a ceiling"),
            "{text}"
        );
        assert!(text.contains("home/.upstroke/upstroke.toml"), "{text}");

        let other = beneath(
            UpstrokeError::Refused {
                message: "not a config error".to_owned(),
            },
            Path::new("machine.toml"),
        );
        assert_eq!(other.to_string(), "not a config error");
    }
}
