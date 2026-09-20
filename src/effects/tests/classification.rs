//! Extended notes: `docs/internals/effects/tests/classification.md`

#![deny(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

#[cfg(test)]
pub(super) mod checks {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    use crate::effects::tests::{
        ClippyToml, ModuleClassification, Wrappers, denylist, denylist_from, repo_root,
        scanned_sources, wrappers, wrappers_from,
    };
    use crate::effects::{
        CLASSIFIED_MODULES, CLIPPY_TOML, WRAPPERS_TOML, blank_comments_and_strings,
        externally_reachable_fns, production_code, reachable_fn_multiplicity, reachable_fn_owners,
    };

    pub(in crate::effects::tests) fn reachable_fns_are_classified() {
        let record = wrappers();
        let recorded: BTreeMap<&str, &ModuleClassification> = record
            .module
            .iter()
            .map(|module| (module.path.as_str(), module))
            .collect();
        assert_eq!(
            recorded.len(),
            record.module.len(),
            "a module is recorded twice"
        );
        assert_eq!(
            recorded.keys().copied().collect::<BTreeSet<_>>(),
            CLASSIFIED_MODULES.iter().copied().collect::<BTreeSet<_>>(),
            "the record and CLASSIFIED_MODULES disagree about the domain"
        );

        let mut total = 0;
        let mut disagreements: Vec<String> = Vec::new();
        for path in CLASSIFIED_MODULES {
            let source = fs::read_to_string(repo_root().join(path))
                .unwrap_or_else(|_| panic!("{path} is in CLASSIFIED_MODULES and not in the tree"));
            let module = recorded[path];
            let (unclassified, invented, derived) = classification_disagreement(module, &source);
            if !unclassified.is_empty() || !invented.is_empty() {
                disagreements.push(format!(
                    "{path}\n    unclassified: {unclassified:?}\n    invented:     {invented:?}"
                ));
            }
            total += derived;
        }
        assert!(
            disagreements.is_empty(),
            "the classification and the modules disagree:\n{}",
            disagreements.join("\n")
        );
        assert!(
            total > 300,
            "only {total} functions were classified; the derivation is finding nothing"
        );
    }

    fn classification_disagreement(
        module: &ModuleClassification,
        source: &str,
    ) -> (Vec<String>, Vec<String>, usize) {
        let path = &module.path;
        let derived: BTreeSet<String> = externally_reachable_fns(source).into_iter().collect();
        let classified: Vec<&str> = module
            .funnel
            .iter()
            .chain(&module.effectful)
            .chain(&module.effectful_unnameable)
            .chain(&module.effect_free)
            .map(|name| name.rsplit("::").next().expect("a name"))
            .collect();
        let unique: BTreeSet<&str> = classified.iter().copied().collect();
        assert_eq!(
            names_in_two_classes(module),
            Vec::<String>::new(),
            "{path}: a name is in two classes"
        );
        let derived_refs: BTreeSet<&str> = derived.iter().map(String::as_str).collect();
        (
            derived_refs
                .difference(&unique)
                .map(|name| (*name).to_owned())
                .collect(),
            unique
                .difference(&derived_refs)
                .map(|name| (*name).to_owned())
                .collect(),
            derived.len(),
        )
    }

    fn names_in_two_classes(module: &ModuleClassification) -> Vec<String> {
        let mut seen = BTreeSet::new();
        module
            .funnel
            .iter()
            .chain(&module.effectful)
            .chain(&module.effectful_unnameable)
            .chain(&module.effect_free)
            .map(|name| name.rsplit("::").next().expect("a name"))
            .filter(|name| !seen.insert(*name))
            .map(str::to_owned)
            .collect()
    }

    fn bearers_of_one_path(owners: &[String]) -> bool {
        let dispatched_through = |owner: &str| -> Option<(String, String)> {
            let (above, last) = owner
                .rsplit_once(" > ")
                .map_or(("", owner), |(above, last)| (above, last));
            let of_trait = match last.strip_prefix("trait ") {
                Some(name) => name,
                None => last.strip_prefix("impl ")?.split_once(" for ")?.0,
            };
            Some((above.to_owned(), of_trait.to_owned()))
        };
        let Some(first) = owners.first() else {
            return true;
        };
        owners.iter().all(|owner| owner == first)
            || dispatched_through(first).is_some_and(|through| {
                owners
                    .iter()
                    .all(|owner| dispatched_through(owner).as_ref() == Some(&through))
            })
    }

    fn effectful_names_shared_across_paths(
        module: &ModuleClassification,
        source: &str,
    ) -> Vec<String> {
        let owners = reachable_fn_owners(source);
        let mut complaints = Vec::new();
        for row in &module.effectful {
            let name = row.rsplit("::").next().expect("a name");
            let Some(bearers) = owners.get(name).filter(|bearers| bearers.len() > 1) else {
                continue;
            };
            if !bearers_of_one_path(bearers) {
                complaints.push(format!(
                    "{}: `{name}` is classified effectful and its {} bearers are declared at \
                     different paths ({bearers:?}, the file's top level being the empty one). A \
                     denial names one path and a row is a bare name, so no record can classify \
                     and deny the second one: a denial for it needs a row, and a row for it is \
                     a name in two classes (`PR309-SHARED-EFFECTFUL-PIN-CANNOT-RECORD-ITS-DENIAL`). \
                     An effectful name is shared only by bearers of one path -- `cfg` twins, or a \
                     trait's declaration and its impls -- so give the other callable a name of \
                     its own",
                    module.path,
                    bearers.len()
                ));
            }
        }
        complaints
    }

    fn unpinned_shared_names(module: &ModuleClassification, source: &str) -> Vec<String> {
        let path = &module.path;
        let borne: BTreeMap<String, usize> = reachable_fn_multiplicity(source)
            .into_iter()
            .filter(|(_, bearers)| *bearers > 1)
            .collect();
        let mut complaints = Vec::new();
        for (name, bearers) in &borne {
            if module.shared.get(name) != Some(bearers) {
                complaints.push(format!(
                    "{path}: {bearers} callables bear the name `{name}` and the record pins {}. The \
                     record classifies by bare name and a denial names one path, so a second \
                     callable under a classified name is answered for by a row that was never \
                     written for it (`PR309-INLINE-WRAPPER-NAME-COLLISION`). Give it a name of \
                     its own, or pin the count in `shared` once every bearer is what the row says; \
                     an effectful name cannot be shared across paths at all",
                    module
                        .shared
                        .get(name)
                        .map_or("nothing, which means one".to_owned(), usize::to_string)
                ));
            }
        }
        for (name, pinned) in &module.shared {
            if !borne.contains_key(name) {
                complaints.push(format!(
                    "{path}: the record pins {pinned} callables named `{name}` and the file holds \
                     fewer than two; a pin that outlives its callables admits the next one unseen"
                ));
            }
        }
        complaints
    }

    pub(in crate::effects::tests) fn shared_names_are_pinned() {
        let record = wrappers();
        let mut complaints = Vec::new();
        let mut pinned = 0;
        for module in &record.module {
            let source = fs::read_to_string(repo_root().join(&module.path))
                .unwrap_or_else(|_| panic!("{} is recorded and not in the tree", module.path));
            complaints.extend(unpinned_shared_names(module, &source));
            complaints.extend(effectful_names_shared_across_paths(module, &source));
            pinned += module.shared.len();
        }
        assert!(
            complaints.is_empty(),
            "a bare name does not say which callable it classifies:\n{}",
            complaints.join("\n")
        );
        assert!(
            pinned > 40,
            "only {pinned} shared names are pinned; the multiplicity reading is finding nothing"
        );
    }

    pub(in crate::effects::tests) fn the_collision_witness_and_its_renamed_control() {
        const COORDINATOR: &str = "src/engine/coordinator.rs";
        const WITNESS: &str = "\npub(super) mod rf_inline_collision {\n\
             \x20   pub(crate) fn run(path: &std::path::Path) -> std::io::Result<()> {\n\
             \x20       std::fs::write(path, b\"rf duplicate-name effect\")\n\
             \x20   }\n\
             }\n";
        let record = wrappers();
        let module = record
            .module
            .iter()
            .find(|module| module.path == COORDINATOR)
            .expect("the coordinator is a classified module");
        let source = fs::read_to_string(repo_root().join(COORDINATOR)).expect(COORDINATOR);

        let (unclassified, invented, _) = classification_disagreement(module, &source);
        assert!(
            unclassified.is_empty()
                && invented.is_empty()
                && unpinned_shared_names(module, &source).is_empty(),
            "{COORDINATOR} is not clean at this head, so nothing below is evidence of anything"
        );
        assert!(
            module.effectful.iter().any(|name| name == "run") && !module.shared.contains_key("run"),
            "`run` is the classified name the witness hides behind, borne by one callable"
        );

        let collided = format!("{source}{WITNESS}");
        let (unclassified, _, _) = classification_disagreement(module, &collided);
        assert!(
            unclassified.is_empty(),
            "the witness is meant to be invisible to the name census, and it found {unclassified:?}"
        );
        let complaints = unpinned_shared_names(module, &collided);
        assert!(
            complaints.len() == 1
                && complaints
                    .iter()
                    .all(|complaint| complaint.contains("2 callables bear the name `run`")),
            "a second `run`, in an inline module of {COORDINATOR}, calling `std::fs::write` and \
             visible to every module under `engine::topology`, was not refused: {complaints:#?}"
        );

        let renamed = format!(
            "{source}{}",
            WITNESS.replace("fn run(", "fn rf_unique_effect(")
        );
        let (unclassified, _, _) = classification_disagreement(module, &renamed);
        assert_eq!(
            unclassified,
            vec!["rf_unique_effect".to_owned()],
            "under a name nothing else bears, the same function is what the name census exists \
             to find"
        );
        assert!(unpinned_shared_names(module, &renamed).is_empty());
    }

    fn denials_no_row_classifies(record: &Wrappers, denied: &ClippyToml) -> Vec<String> {
        let classified: BTreeSet<String> = record
            .module
            .iter()
            .flat_map(|module| {
                module
                    .effectful
                    .iter()
                    .map(move |name| format!("{}::{name}", module.crate_path))
            })
            .collect();
        denied
            .all()
            .filter(|entry| entry.path.starts_with("upstroke::"))
            .filter(|entry| !classified.contains(&entry.path))
            .map(|entry| entry.path.clone())
            .collect()
    }

    pub(in crate::effects::tests) fn the_shared_effectful_pin_witness_and_its_controls() {
        const COORDINATOR: &str = "src/engine/coordinator.rs";
        const WITNESS: &str = "\npub(super) mod rf2_shared {\n\
             \x20   pub(crate) fn run_with(path: &std::path::Path) -> std::io::Result<()> {\n\
             \x20       std::fs::write(path, b\"rf2 shared effectful name\")\n\
             \x20   }\n\
             }\n";
        const SECOND_PATH: &str = "upstroke::engine::coordinator::rf2_shared::run_with";
        let of = |record: &Wrappers| -> usize {
            record
                .module
                .iter()
                .position(|module| module.path == COORDINATOR)
                .expect("the coordinator is a classified module")
        };
        let read = |path: &str| -> String {
            fs::read_to_string(repo_root().join(path))
                .unwrap_or_else(|_| panic!("{path} is read by this test and not in the tree"))
                .replace("\r\n", "\n")
        };
        let record_text = read(WRAPPERS_TOML);
        let denied_text = read(CLIPPY_TOML);
        let source = read(COORDINATOR);
        let collided = format!("{source}{WITNESS}");

        let record = wrappers_from(&record_text);
        let module = &record.module[of(&record)];
        assert!(
            effectful_names_shared_across_paths(module, &source).is_empty()
                && unpinned_shared_names(module, &source).is_empty(),
            "{COORDINATOR} is not clean at this head, so nothing below is evidence of anything"
        );

        let unpinned = unpinned_shared_names(module, &collided);
        assert!(
            unpinned.len() == 1 && unpinned[0].contains("2 callables bear the name `run_with`"),
            "{unpinned:#?}"
        );

        let anchor = "crate_path = \"upstroke::engine::coordinator\"\n";
        assert_eq!(record_text.matches(anchor).count(), 1);
        let pinned_text =
            record_text.replace(anchor, &format!("{anchor}shared = {{ run_with = 2 }}\n"));
        let pinned = wrappers_from(&pinned_text);
        let module = &pinned.module[of(&pinned)];
        assert!(
            unpinned_shared_names(module, &collided).is_empty(),
            "the pin is meant to satisfy the count; that is the witness"
        );
        let across = effectful_names_shared_across_paths(module, &collided);
        assert!(
            across.len() == 1 && across[0].contains("`run_with` is classified effectful"),
            "an effectful name pinned across two paths was admitted: {across:#?}"
        );

        let denial = format!(
            "disallowed-methods = [\n    {{ path = \"{SECOND_PATH}\", reason = \"UPSTROKE-WRAPPER: control\" }},"
        );
        assert_eq!(denied_text.matches("disallowed-methods = [").count(), 1);
        let with_denial = denylist_from(&denied_text.replace("disallowed-methods = [", &denial));
        assert_eq!(
            denials_no_row_classifies(&pinned, &with_denial),
            vec![SECOND_PATH.to_owned()]
        );
        let rows = "    \"run_harness_on\",\n    \"run_with\",\n]";
        assert_eq!(pinned_text.matches(rows).count(), 1);
        let with_row = wrappers_from(&pinned_text.replace(
            rows,
            "    \"run_harness_on\",\n    \"rf2_shared::run_with\",\n    \"run_with\",\n]",
        ));
        assert!(denials_no_row_classifies(&with_row, &with_denial).is_empty());
        assert_eq!(
            names_in_two_classes(&with_row.module[of(&with_row)]),
            vec!["run_with".to_owned()]
        );

        let mut shared_effectful = Vec::new();
        for module in &record.module {
            let source = fs::read_to_string(repo_root().join(&module.path)).expect("a module");
            assert_eq!(
                effectful_names_shared_across_paths(module, &source),
                Vec::<String>::new()
            );
            shared_effectful.extend(
                module
                    .effectful
                    .iter()
                    .map(|row| row.rsplit("::").next().expect("a name"))
                    .filter(|name| module.shared.contains_key(*name))
                    .map(|name| format!("{}::{name}", module.path)),
            );
        }
        assert!(
            shared_effectful.len() > 2,
            "only {shared_effectful:?} are effectful and shared, so the one-path rule judged \
             nothing the tree holds"
        );
        assert!(bearers_of_one_path(&[String::new(), String::new()]));
        assert!(bearers_of_one_path(&[
            "trait GitView".to_owned(),
            "impl GitView for HostView".to_owned(),
        ]));
        assert!(!bearers_of_one_path(&[
            String::new(),
            "mod rf2_shared".to_owned()
        ]));
        assert!(!bearers_of_one_path(&[
            String::new(),
            "impl Hatch".to_owned()
        ]));
        assert!(!bearers_of_one_path(&[
            "trait GitView".to_owned(),
            "impl Other for HostView".to_owned(),
        ]));
    }

    pub(in crate::effects::tests) fn effectful_wrappers_are_denied() {
        let record = wrappers();
        let denied = denylist()
            .paths()
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<BTreeSet<String>>();
        let mut named = 0;
        for module in &record.module {
            if module.effectful.is_empty() {
                continue;
            }
            assert!(
                !module.crate_path.is_empty(),
                "{} records effectful wrappers and no crate path to name them by; an \
                 unreachable module's wrappers belong in `effectful_unnameable`",
                module.path
            );
            for name in &module.effectful {
                let path = format!("{}::{name}", module.crate_path);
                assert!(
                    denied.contains(&path),
                    "{} classifies `{name}` effectful and `{path}` is not in {CLIPPY_TOML}",
                    module.path
                );
                named += 1;
            }
        }
        assert!(named >= 10, "only {named} wrappers were checked");

        let unclassified = denials_no_row_classifies(&record, &denylist());
        assert!(
            unclassified.is_empty(),
            "{CLIPPY_TOML} denies {unclassified:?} and no module classifies them effectful"
        );
    }

    pub(in crate::effects::tests) fn crate_paths_name_the_modules() {
        // `crate_path = ""` is the record's escape hatch: the check above
        // refuses an effectful row it cannot path, so an empty path pushes
        // every effectful body of that module into `effectful_unnameable`,
        // the class the denial check skips. #306 found three private `mod`s
        // of `engine` using it on the claim that a private module has no
        // clippy path. It has: a `pub(super)` fn there is visible to every
        // module under `engine::topology`, and a reference to
        // `crate::engine::attempt::run_attempt` from
        // `engine::topology::integrate` passed clippy undenied and was refused
        // once the path was listed. The hatch is legitimate for exactly one
        // module, the binary crate root, which has no `upstroke::` path at all.
        let record = wrappers();
        let empty: Vec<&str> = record
            .module
            .iter()
            .filter(|module| module.crate_path.is_empty())
            .map(|module| module.path.as_str())
            .collect();
        assert_eq!(
            empty,
            vec!["src/main.rs"],
            "a library module records no crate path, so an effectful name in it cannot \
             be denied and would be filed `effectful_unnameable` on a claim clippy does \
             not honour (`PR7-WRAPPERS-EMPTY-DOMAIN`)"
        );
        let mut checked = 0;
        for module in &record.module {
            if module.crate_path.is_empty() {
                continue;
            }
            let expected = format!(
                "upstroke::{}",
                module
                    .path
                    .trim_start_matches("src/")
                    .trim_end_matches(".rs")
                    .trim_end_matches("/mod")
                    .replace('/', "::")
            );
            assert_eq!(
                module.crate_path, expected,
                "{}: the crate path does not name the module the file declares, so a \
                 denial built from it would resolve to nothing",
                module.path
            );
            checked += 1;
        }
        assert!(checked > 50, "only {checked} crate paths were checked");
    }

    pub(in crate::effects::tests) fn funnel_rows_name_a_site() {
        let record = wrappers();
        let mut checked = 0;
        for module in &record.module {
            if module.funnel.is_empty() {
                continue;
            }
            let source = fs::read_to_string(repo_root().join(&module.path)).expect("read module");
            let production = production_code(&source);
            assert!(
                production.contains("EffectSiteId") || production.contains("Site"),
                "{} classifies funnels and never names a site",
                module.path
            );
            for name in &module.funnel {
                let bare = name.rsplit("::").next().expect("a name");
                assert!(
                    production.contains(&format!("fn {bare}")),
                    "{} classifies `{name}` a funnel and declares no such fn",
                    module.path
                );
                let path = format!("{}::{name}", module.crate_path);
                assert!(
                    !denylist().paths().contains(path.as_str()),
                    "`{path}` is classified a funnel and is also denied"
                );
                checked += 1;
            }
        }
        assert!(checked >= 15, "only {checked} funnels were checked");
    }

    pub(in crate::effects::tests) fn libc_items_are_classified_and_denied() {
        let record = wrappers();
        let mut used: BTreeSet<String> = BTreeSet::new();
        for (_, source) in scanned_sources() {
            let text = blank_comments_and_strings(&source);
            let mut at = 0;
            while let Some(hit) = text[at..].find("libc::") {
                let start = at + hit + "libc::".len();
                let item: String = text[start..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                at = start.max(at + 1);
                if !item.is_empty() {
                    used.insert(item);
                }
            }
        }
        assert!(used.len() > 60, "only {} libc items found", used.len());

        let classified: BTreeSet<&str> = record
            .libc
            .effect
            .iter()
            .chain(&record.libc.not_an_effect)
            .map(String::as_str)
            .collect();
        let unclassified: Vec<&String> = used
            .iter()
            .filter(|item| !classified.contains(item.as_str()))
            .collect();
        assert!(
            unclassified.is_empty(),
            "these `libc::` items are used and unclassified: {unclassified:?}"
        );
        let overlap: Vec<&String> = record
            .libc
            .effect
            .iter()
            .filter(|item| record.libc.not_an_effect.contains(item))
            .collect();
        assert!(overlap.is_empty(), "classified both ways: {overlap:?}");

        let denied_toml = denylist();
        let denied = denied_toml.paths();
        for item in &record.libc.effect {
            let path = format!("libc::{item}");
            assert!(
                denied.contains(path.as_str()),
                "`{path}` is classified an effect and is not denied"
            );
        }
        let effects: BTreeSet<&str> = record.libc.effect.iter().map(String::as_str).collect();
        for path in &denied {
            let Some(item) = path.strip_prefix("libc::") else {
                continue;
            };
            assert!(
                effects.contains(item),
                "{CLIPPY_TOML} denies `{path}` and {WRAPPERS_TOML} does not classify \
                 `{item}` an effect"
            );
        }
    }
}
