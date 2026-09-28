//! Extended notes: `docs/internals/effects/tests/source_oracles.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

#[cfg(test)]
pub(super) mod oracles {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::effects::census_domain::{candidates_for, scan_module_declarations, sole_present};
    use crate::effects::tests::cfg::WHOLE_FILE_TEST_MODULES;
    use crate::effects::tests::{
        crate_roots, is_the_literal_mod_tests_form, repo_root, scanned_sources,
    };
    use crate::effects::{
        CLASSIFIED_MODULES, Predicate, RUSTC_WHITESPACE, TOPOLOGY_MODULES, attribute_open,
        blank_comments, blank_comments_and_strings, externally_reachable_fns, gate_predicate,
        next_test_only_attribute, parse_predicate, production_code, production_region,
        reachable_fn_multiplicity,
    };

    fn declared_production_children(
        declared_in: &Path,
        source: &str,
    ) -> Vec<(String, [PathBuf; 2])> {
        declared_children(declared_in, source)
            .into_iter()
            .filter(|(_, _, test_only)| !test_only)
            .map(|(name, candidates, _)| (name, candidates))
            .collect()
    }

    fn declared_children(declared_in: &Path, source: &str) -> Vec<(String, [PathBuf; 2], bool)> {
        refuse_unclassifiable_cfg_attr(declared_in, source);
        scan_module_declarations(source)
            .unwrap_or_else(|refusal| panic!("{}: {refusal}", declared_in.display()))
            .into_iter()
            .map(|declaration| {
                let candidates = candidates_for(
                    crate_roots(),
                    declared_in,
                    &declaration.inline_path,
                    &declaration.name,
                )
                .unwrap_or_else(|refusal| panic!("{refusal}"));
                (declaration.name, candidates, declaration.test_only)
            })
            .collect()
    }

    fn refuse_unclassifiable_cfg_attr(declared_in: &Path, source: &str) {
        let blanked = blank_comments_and_strings(source);
        let mut rest = blanked.as_str();
        while let Some(at) = rest.find("cfg_attr") {
            rest = &rest[at + "cfg_attr".len()..];
            let applied = &rest[..rest.find(']').unwrap_or(rest.len())];
            assert!(
                !applied.contains("cfg"),
                "`{}` writes `cfg_attr{applied}]`, which rustc can apply as a `cfg` that \
                 `scan_module_declarations` does not decide. A walk that cannot classify a \
                 declaration production or test-only must not classify it",
                declared_in.display()
            );
        }
    }

    mod domain {
        use std::collections::BTreeSet;
        use std::fs;
        use std::path::{Path, PathBuf};

        use crate::effects::census_domain::sole_present;
        use crate::effects::{blank_comments_and_strings, production_region};

        use super::declared_children;

        pub(super) struct ProductionModule {
            sources: Vec<(PathBuf, String)>,
        }

        impl ProductionModule {
            pub(super) fn walk(root: &Path) -> Self {
                let mut queue = vec![root.to_path_buf()];
                let mut seen = BTreeSet::new();
                let mut sources: Vec<(PathBuf, String)> = Vec::new();
                let mut accounted: BTreeSet<PathBuf> = BTreeSet::new();
                let mut test_owned: BTreeSet<PathBuf> = BTreeSet::new();
                while let Some(path) = queue.pop() {
                    if !seen.insert(path.clone()) {
                        continue;
                    }
                    let source = fs::read_to_string(&path).expect("a declared module file");
                    for (name, candidates, test_only) in declared_children(&path, &source) {
                        let resolved = sole_present(&candidates, &|candidate| candidate.is_file())
                            .unwrap_or_else(|present| {
                                panic!(
                                    "`{}` declares `mod {name};` and {present} of {candidates:?} \
                                     exist. A census domain that cannot name the file a \
                                     declaration resolves to is not a domain",
                                    path.display()
                                )
                            })
                            .clone();
                        accounted.insert(resolved.clone());
                        if test_only {
                            test_owned.insert(module_dir(&resolved));
                        } else {
                            queue.push(resolved);
                        }
                    }
                    sources.push((path, source));
                }
                sources.sort_by(|(left, _), (right, _)| left.cmp(right));
                assert!(
                    sources.len() > 1 && sources.iter().any(|(path, _)| path == root),
                    "the walk of `{}` returned {:?}: a module domain is the root plus what it \
                     declares",
                    root.display(),
                    sources.iter().map(|(path, _)| path).collect::<Vec<_>>()
                );
                refuse_unaccounted_files(root, &accounted, &test_owned);
                refuse_macro_declared_modules(&sources);
                Self { sources }
            }

            pub(super) fn sources_for_witness(&self) -> Vec<(PathBuf, String)> {
                self.sources.clone()
            }

            pub(super) fn files(&self) -> Vec<PathBuf> {
                self.sources.iter().map(|(path, _)| path.clone()).collect()
            }

            pub(super) fn row_mapping_wildcards(&self) -> RowMappingScan {
                let mut scan = RowMappingScan {
                    scanned: Vec::new(),
                    offenders: Vec::new(),
                };
                for (path, source) in &self.sources {
                    let mut mappings = 0_usize;
                    let production = blank_comments_and_strings(&production_region(source));
                    let mut rest = production.as_str();
                    while let Some(at) = rest.find("fn row(") {
                        rest = &rest[at + "fn row(".len()..];
                        let body_end = rest.find("\n    }").unwrap_or(rest.len());
                        let body = &rest[..body_end];
                        mappings += 1;
                        for wildcard in ["_ =>", "_=>"] {
                            if body.contains(wildcard) {
                                scan.offenders.push(format!(
                                    "`{}`: a `row()` mapping falls back through `{wildcard}`, \
                                     so a site added later compiles with no declared row: …{}",
                                    path.display(),
                                    &body[..body.len().min(160)]
                                ));
                            }
                        }
                    }
                    scan.scanned.push((path.clone(), mappings));
                }
                scan
            }
        }

        pub(super) fn module_dir(file: &Path) -> PathBuf {
            if file.file_stem().is_some_and(|stem| stem == "mod") {
                file.parent().unwrap_or(file).to_path_buf()
            } else {
                file.with_extension("")
            }
        }

        pub(super) fn refuse_unaccounted_files(
            root: &Path,
            accounted: &BTreeSet<PathBuf>,
            test_owned: &BTreeSet<PathBuf>,
        ) {
            let owned = module_dir(root);
            if !owned.is_dir() {
                return;
            }
            let mut stack = vec![owned];
            let mut unaccounted: Vec<PathBuf> = Vec::new();
            while let Some(current) = stack.pop() {
                let entries = fs::read_dir(&current).unwrap_or_else(|error| {
                    panic!("`{}` is not readable: {error}", current.display())
                });
                for entry in entries {
                    let path = entry.expect("a directory entry").path();
                    if path.is_dir() {
                        if !test_owned.contains(&path) {
                            stack.push(path);
                        }
                    } else if path.extension().is_some_and(|ext| ext == "rs")
                        && !accounted.contains(&path)
                    {
                        unaccounted.push(path);
                    }
                }
            }
            unaccounted.sort();
            assert!(
                unaccounted.is_empty(),
                "no declaration in the module rooted at `{}` accounts for {unaccounted:?}. A \
                 census domain derived from declarations is only the module when the \
                 declarations account for every file of it; a macro at item position expands to \
                 a declaration this scan cannot read, and the file it declares would otherwise \
                 be scanned by nothing",
                root.display()
            );
        }

        fn item_position_macros(blanked: &str) -> Vec<(usize, String)> {
            let bytes = blanked.as_bytes();
            let mut depth = 0_usize;
            let mut found = Vec::new();
            for (at, byte) in bytes.iter().enumerate() {
                match byte {
                    b'{' | b'(' | b'[' => depth += 1,
                    b'}' | b')' | b']' => depth = depth.saturating_sub(1),
                    b'!' if depth == 0 => {
                        let mut after = at + 1;
                        while bytes.get(after).is_some_and(|b| b.is_ascii_whitespace()) {
                            after += 1;
                        }
                        if !matches!(bytes.get(after), Some(b'(' | b'[' | b'{')) {
                            continue;
                        }
                        let mut start = at;
                        while start > 0
                            && (bytes[start - 1].is_ascii_alphanumeric()
                                || bytes[start - 1] == b'_')
                        {
                            start -= 1;
                        }
                        if start == at {
                            continue;
                        }
                        let mut before = start;
                        while before > 0 && bytes[before - 1].is_ascii_whitespace() {
                            before -= 1;
                        }
                        let item = before == 0 || matches!(bytes[before - 1], b';' | b'}' | b']');
                        if item {
                            let name = blanked[start..at].to_owned();
                            let line = blanked[..start].matches('\n').count() + 1;
                            found.push((line, name));
                        }
                    }
                    _ => {}
                }
            }
            found
        }

        pub(super) fn refuse_macro_declared_modules(sources: &[(PathBuf, String)]) {
            let blanked: Vec<(PathBuf, String)> = sources
                .iter()
                .map(|(path, source)| (path.clone(), blank_comments_and_strings(source)))
                .collect();
            let mut defined: BTreeSet<String> = BTreeSet::new();
            for (_, source) in &blanked {
                let mut rest = source.as_str();
                while let Some(at) = rest.find("macro_rules!") {
                    rest = &rest[at + "macro_rules!".len()..];
                    let name: String = rest
                        .trim_start()
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        defined.insert(name);
                    }
                }
            }
            let mut offenders = Vec::new();
            for (path, source) in &blanked {
                for (line, name) in item_position_macros(source) {
                    if !defined.contains(&name) {
                        offenders.push(format!("`{}:{line}` invokes `{name}!`", path.display()));
                    }
                }
            }
            assert!(
                offenders.is_empty(),
                "an item-position macro whose `macro_rules!` is not in this module expands to \
                 items the declaration scan cannot read, so it can declare a module -- with a \
                 `#[path]` even one outside the module's own directory, where the directory \
                 reconciliation cannot find it either: {offenders:?}"
            );
        }

        pub(super) struct RowMappingScan {
            scanned: Vec<(PathBuf, usize)>,
            offenders: Vec<String>,
        }

        impl RowMappingScan {
            pub(super) fn paths(&self) -> Vec<PathBuf> {
                self.scanned.iter().map(|(path, _)| path.clone()).collect()
            }

            pub(super) fn mappings(&self) -> usize {
                self.scanned.iter().map(|(_, found)| found).sum()
            }

            pub(super) fn read_without_a_mapping(&self) -> Vec<&PathBuf> {
                self.scanned
                    .iter()
                    .filter(|(_, found)| *found == 0)
                    .map(|(path, _)| path)
                    .collect()
            }

            pub(super) fn offenders(&self) -> &[String] {
                &self.offenders
            }
        }
    }

    use domain::{ProductionModule, refuse_macro_declared_modules, refuse_unaccounted_files};

    fn item_body(source: &str, signature: &str) -> String {
        let blanked = blank_comments_and_strings(source);
        let at = blanked
            .find(signature)
            .unwrap_or_else(|| panic!("`{signature}` is not in this file"));
        let open = at + blanked[at..].find('{').expect("the item has a body");
        let mut depth = 0_usize;
        for (offset, byte) in blanked[open..].bytes().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return blanked[open..=open + offset].to_owned();
                    }
                }
                _ => {}
            }
        }
        panic!("`{signature}` has no closing brace")
    }

    fn panic_message(body: impl FnOnce()) -> Option<String> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(body))
            .err()
            .map(|payload| {
                payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| {
                        payload
                            .downcast_ref::<&str>()
                            .map(|text| (*text).to_owned())
                    })
                    .unwrap_or_else(|| "<non-string panic payload>".to_owned())
            })
    }

    pub(in crate::effects::tests) fn site_row_mappings_have_no_wildcard_arm() {
        let module = ProductionModule::walk(&repo_root().join("src/topology/effects.rs"));
        let walked = module.files();
        let scan = module.row_mapping_wildcards();

        let scanned: Vec<PathBuf> = scan.paths();
        let unwalked: Vec<&PathBuf> = scanned
            .iter()
            .filter(|path| !walked.contains(path))
            .collect();
        let unscanned: Vec<&PathBuf> = walked
            .iter()
            .filter(|path| !scanned.contains(path))
            .collect();
        assert_eq!(
            scanned, walked,
            "this census read a different set of files from the one the walk produced, so its \
             domain is not the declared production module. Read and not walked: {unwalked:?}; \
             walked and not read: {unscanned:?}"
        );

        assert!(
            walked.len() >= 8,
            "only {} file(s) in the `topology::effects` production module, so this census is \
             looking at the wrong module: {walked:?}",
            walked.len()
        );
        let mappings: usize = scan.mappings();
        assert!(
            mappings >= 8,
            "only {mappings} `row()` mappings scanned, so this census is looking at the wrong \
             files"
        );
        assert!(scan.offenders().is_empty(), "{:#?}", scan.offenders());
    }

    pub(in crate::effects::tests) fn the_row_mapping_census_domain_is_the_declared_module() {
        let effects = repo_root().join("src/topology/effects.rs");

        let synthetic =
            "mod vocab;\n#[cfg(test)]\nmod row_cases;\nmod twelfth;\n#[cfg(test)]\nmod tests;\n";
        let declared = declared_production_children(&effects, synthetic);
        let names: Vec<&str> = declared.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            ["vocab", "twelfth"],
            "the domain is read from the declarations. `row_cases` is `#[cfg(test)]` on the \
             declaration, which is exactly what `production_region` cannot cut out of the file \
             it names, and its stem is not `tests`, which is what a file-name rule reads instead"
        );
        assert_eq!(
            declared[1].1,
            [
                repo_root().join("src/topology/effects/twelfth.rs"),
                repo_root().join("src/topology/effects/twelfth/mod.rs"),
            ],
            "a production child in the `<name>/mod.rs` layout has to be a candidate; it is a \
             directory entry, so a `read_dir` filtered to `*.rs` files never sees it"
        );

        let lib = repo_root().join("src/lib.rs");
        let lib_source = fs::read_to_string(&lib).expect("the crate root");
        let topology = declared_production_children(&lib, &lib_source)
            .into_iter()
            .find(|(name, _)| name == "topology")
            .expect("`src/lib.rs` declares `mod topology;`");
        assert_eq!(
            sole_present(&topology.1, &|candidate| candidate.is_file())
                .expect("exactly one candidate for `topology` is on disk"),
            &repo_root().join("src/topology/mod.rs"),
            "the resolution has to name the `<name>/mod.rs` file, not merely list it"
        );

        let module = ProductionModule::walk(&effects);
        let walked = module.files();
        let mut expected: Vec<PathBuf> = [
            "src/topology/effects.rs",
            "src/topology/effects/bijection.rs",
            "src/topology/effects/export.rs",
            "src/topology/effects/harness.rs",
            "src/topology/effects/registry.rs",
            "src/topology/effects/residue_authority.rs",
            "src/topology/effects/sites.rs",
            "src/topology/effects/vocab.rs",
        ]
        .iter()
        .map(|relative| repo_root().join(relative))
        .collect();
        expected.sort();
        assert_eq!(
            walked, expected,
            "the `row()` census reads the root and the seven production children of \
             `topology::effects`, and nothing else"
        );
        assert!(
            !walked.contains(&repo_root().join("src/topology/effects/tests.rs")),
            "`tests.rs` is declared `#[cfg(test)]` and is not production code: {walked:?}"
        );

        let refusal = panic_message(|| {
            declared_production_children(&effects, "#[cfg_attr(all(), cfg(test))]\nmod hidden;\n");
        })
        .expect("a `cfg_attr` that can apply a `cfg` has to stop the walk");
        assert!(
            refusal.contains("cannot classify a declaration"),
            "the walk stopped, but for some other reason: {refusal}"
        );
        assert_eq!(
            declared_production_children(&effects, "mod hidden;\n")
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["hidden"],
            "control: `mod hidden;` on its own is classified, so (4) measured the `cfg_attr`"
        );

        let scan = module.row_mapping_wildcards();
        assert_eq!(
            scan.paths(),
            walked,
            "the scan's record is not the walk's output, so the equality the census asserts is \
             between something else and something else"
        );
        let barren = scan.read_without_a_mapping();
        assert!(
            barren.len() >= 5,
            "only {} file(s) of the domain hold no `row()` mapping, so this part no longer \
             measures that a file with no hit is still recorded: {barren:?}",
            barren.len()
        );
        assert!(
            scan.mappings() > 0,
            "no file of the domain holds a `row()` mapping at all, so the scan found nothing \
             and the count above is vacuous"
        );

        let owned: BTreeSet<PathBuf> = [repo_root().join("src/topology/effects/tests")]
            .into_iter()
            .collect();
        let mut accounted: BTreeSet<PathBuf> = walked
            .iter()
            .filter(|path| *path != &effects)
            .cloned()
            .collect();
        accounted.insert(repo_root().join("src/topology/effects/tests.rs"));
        refuse_unaccounted_files(&effects, &accounted, &owned);
        let mut short = accounted.clone();
        let dropped = repo_root().join("src/topology/effects/vocab.rs");
        assert!(
            short.remove(&dropped),
            "the accounting did not hold `vocab.rs`"
        );
        let refusal = panic_message(|| {
            refuse_unaccounted_files(&effects, &short, &owned);
        })
        .expect("a file no declaration accounts for has to stop the walk");
        assert!(
            refusal.contains("no declaration in the module rooted at")
                && refusal.contains("vocab.rs"),
            "the walk stopped, but for some other reason: {refusal}"
        );
        let mut without_tests = accounted.clone();
        assert!(without_tests.remove(&repo_root().join("src/topology/effects/tests.rs")));
        let tests_refusal = panic_message(|| {
            refuse_unaccounted_files(&effects, &without_tests, &owned);
        })
        .expect("an unaccounted `tests.rs` has to stop the walk too");
        assert!(
            tests_refusal.contains("tests.rs"),
            "the refusal did not name the test file: {tests_refusal}"
        );

        let hidden = repo_root().join("src/topology/effects.rs");
        let invocation = "mod bijection;\ndeclare_hidden!();\n".to_owned();
        let refusal = panic_message(|| {
            refuse_macro_declared_modules(&[(hidden.clone(), invocation.clone())]);
        })
        .expect("an item-position macro the walk cannot read has to stop it");
        assert!(
            refusal.contains("declare_hidden") && refusal.contains("cannot read"),
            "the walk stopped, but for some other reason: {refusal}"
        );
        refuse_macro_declared_modules(&[
            (hidden.clone(), invocation),
            (
                repo_root().join("src/topology/effects/vocab.rs"),
                "macro_rules! declare_hidden { () => {}; }\n".to_owned(),
            ),
        ]);
        refuse_macro_declared_modules(&[(hidden, "const _: () = assert!(true);\n".to_owned())]);
        refuse_macro_declared_modules(&module.sources_for_witness());

        let this_file = fs::read_to_string(repo_root().join("src/effects/tests/source_oracles.rs"))
            .expect("this file");
        let body = item_body(&this_file, "fn site_row_mappings_have_no_wildcard_arm");
        assert!(
            body.contains("offenders") && !body.contains("sole_present"),
            "`item_body` did not isolate the row-mapping census: {body}"
        );
        const READERS: [&str; 6] = [
            "fs",
            "File",
            "read_to_string",
            "read_dir",
            "include_str",
            "include_bytes",
        ];
        for reader in READERS {
            assert!(
                !body.contains(reader),
                "the row-mapping census names `{reader}`, so it obtains source text from \
                 somewhere other than the walk it hands the scan — a second reader beside the \
                 domain equality part (5) describes, which is a defect whether or not that \
                 equality still holds: {body}"
            );
        }
        let own = item_body(
            &this_file,
            "fn the_row_mapping_census_domain_is_the_declared_module",
        );
        assert!(
            READERS.iter().any(|reader| own.contains(reader)),
            "control: the needle set cannot detect a file read even in a body that does one"
        );
    }

    pub(in crate::effects::tests) fn topology_production_names_no_funnel() {
        const FUNNELS: &[&str] = &[
            "workspace_manager::",
            "rundir::",
            "EventLog::",
            "establish_stable_prefix",
            "util::write_json",
            "util::write_text",
        ];
        let mut topology = 0;
        let mut callers = Vec::new();
        for (path, source) in scanned_sources() {
            let is_topology = TOPOLOGY_MODULES
                .iter()
                .any(|banned| path.starts_with(banned) || path == *banned);
            if !is_topology || !path.starts_with("src/topology/") {
                continue;
            }
            topology += 1;
            let production = blank_comments_and_strings(&production_region(&source));
            for funnel in FUNNELS {
                if production.contains(funnel) {
                    callers.push(format!("{path} names `{funnel}` in production"));
                }
            }
        }
        assert!(topology >= 8, "only {topology} topology modules scanned");
        assert!(callers.is_empty(), "{callers:#?}");

        let registry = fs::read_to_string(repo_root().join("src/topology/registry.rs"))
            .expect("src/topology/registry.rs");
        let production = production_region(&registry);
        assert!(
            !production.contains("rundir::"),
            "the production region names a funnel"
        );
        assert!(
            registry.contains("rundir::create_public_dir"),
            "the control: the registry's TEST region builds its fixture through the \
             run-directory funnel, so a production/test split that had collapsed \
             would fail here instead of reporting silence"
        );
        assert!(
            production.len() < registry.len(),
            "the production region is the whole file, so the split did nothing"
        );
    }

    pub(in crate::effects::tests) fn the_reachable_fn_parser_finds_every_shape() {
        let source = concat!(
            "pub fn free() {}\n",
            "pub(crate) fn crate_visible() {}\n",
            "pub(super) fn super_visible() {}\n",
            "pub(in crate::engine) fn path_visible() {}\n",
            "pub (in crate::engine) async fn spaced_path_visible() {}\n",
            "pub(in crate::engine) extern \"Rust\" fn abi_visible() {}\n",
            "pub unsafe extern \"C\" fn unsafe_abi_visible() {}\n",
            "fn private() {}\n",
            "pub const fn constant() -> u8 { 1 }\n",
            "pub unsafe fn unsafely() {}\n",
            "impl Thing { pub fn inherent(&self) {} fn hidden(&self) {} }\n",
            "impl Trait for Thing { fn through_the_trait(&self) {} }\n",
            "pub trait Public { fn declared(&self) -> u8; fn defaulted(&self) -> u8 { 1 } }\n",
            "trait Private { fn private_default(&self) -> u8 { 1 } }\n",
            "impl Trait for [u8; 4] { fn through_an_array_impl(&self) {} }\n",
            "pub trait Wide { fn default_returning_an_array(&self) -> [u8; 4] { [0; 4] } }\n",
            "pub fn /* between */ after_a_comment() {}\n",
            "pub fn\n    after_a_line_break() {}\n",
            "pub fn r#raw_identifier() {}\n",
            "pub fn \u{fc}n\u{ef}_non_ascii() {}\n",
            "impl<T> Glued<T>for Thing<T> { fn after_a_glued_for(&self) {} }\n",
            "impl::path::Trait for Thing { fn after_a_glued_impl(&self) {} }\n",
            "impl Trait for&'static str { fn for_a_reference(&self) {} }\n",
            "impl<F: for<'a> Fn(&'a u8)> Holder<F> { fn behind_a_bound(&self) {} }\n",
            "impl Trait for Thing { fn returning_a_type_ending_fn(&self) -> \u{c9}fn { \u{c9}fn } }\n",
            "impl \u{c9}for { fn behind_a_type_ending_for(&self) {} }\n",
            "macro_rules! named { ($name:ident) => { pub fn $name() {} }; }\n",
            "#[cfg(test)]\nmod tests { pub fn in_the_test_region() {} }\n",
        );
        let found = externally_reachable_fns(source);
        assert_eq!(
            found,
            vec![
                "abi_visible".to_owned(),
                "after_a_comment".to_owned(),
                "after_a_glued_for".to_owned(),
                "after_a_glued_impl".to_owned(),
                "after_a_line_break".to_owned(),
                "behind_a_bound".to_owned(),
                "constant".to_owned(),
                "crate_visible".to_owned(),
                "default_returning_an_array".to_owned(),
                "defaulted".to_owned(),
                "for_a_reference".to_owned(),
                "free".to_owned(),
                "inherent".to_owned(),
                "path_visible".to_owned(),
                "raw_identifier".to_owned(),
                "returning_a_type_ending_fn".to_owned(),
                "spaced_path_visible".to_owned(),
                "super_visible".to_owned(),
                "through_an_array_impl".to_owned(),
                "through_the_trait".to_owned(),
                "unsafe_abi_visible".to_owned(),
                "unsafely".to_owned(),
                "\u{fc}n\u{ef}_non_ascii".to_owned(),
            ],
            "the parser's answer moved"
        );
        assert!(
            !found.iter().any(|name| name.contains('$')),
            "a macro's metavariable is not a name; what expansion names is outside this reading \
             (`PR7-WRAPPERS-EMPTY-DOMAIN`), and it must not look as if it were inside"
        );
        assert!(!found.contains(&"private".to_owned()));
        assert!(!found.contains(&"hidden".to_owned()));
        assert!(!found.contains(&"in_the_test_region".to_owned()));
        assert!(!found.contains(&"declared".to_owned()));
        assert!(!found.contains(&"private_default".to_owned()));
        assert!(
            !found.contains(&"behind_a_type_ending_for".to_owned()),
            "the `for` that ends `\u{c9}for` is not a keyword, so its impl is inherent"
        );

        for separator in RUSTC_WHITESPACE {
            let written = format!(
                "#[rustfmt::skip]\npub(crate) fn{separator}sep_free() {{}}\n\
                 pub trait{separator}SepTrait {{ fn sep_defaulted(&self) -> u8 {{ 1 }} }}\n\
                 impl{separator}SepTrait{separator}for{separator}Thing {{ fn sep_through(&self) {{}} }}\n"
            );
            assert_eq!(
                externally_reachable_fns(&written),
                vec![
                    "sep_defaulted".to_owned(),
                    "sep_free".to_owned(),
                    "sep_through".to_owned(),
                ],
                "U+{:04X} separates tokens for rustc, and written after `fn`, `trait`, `impl` and \
                 around `for` it hid a name from the classification domain (the review of \
                 84123789 executed `fn`, U+200E, the name, in `src/engine/coordinator.rs`): \
                 {written:?}",
                u32::from(separator)
            );
            assert_eq!(
                reachable_fn_multiplicity(&written)
                    .into_iter()
                    .collect::<Vec<_>>(),
                vec![
                    ("sep_defaulted".to_owned(), 1),
                    ("sep_free".to_owned(), 1),
                    ("sep_through".to_owned(), 1),
                ],
                "U+{:04X}: a name the domain holds has to be one the count holds, once",
                u32::from(separator)
            );
            let unread = format!("fn{separator}on_text_no_tokenizer_touched() {{}}");
            assert_eq!(
                crate::effects::declared_fns(&unread),
                vec![(0, "on_text_no_tokenizer_touched")],
                "U+{:04X}: the name reader names the one definition itself, so it reads the \
                 separator on text no tokenizer has rewritten",
                u32::from(separator)
            );
        }

        let exploit = concat!(
            "pub trait ContainerHooks {\n",
            "    fn phase(&mut self) -> u8;\n",
            "    fn remove_without_a_site(&self, path: &Path) { let _ = fs::remove_file(path); }\n",
            "}\n",
        );
        assert!(
            externally_reachable_fns(exploit).contains(&"remove_without_a_site".to_owned()),
            "the effect a default trait body performs is invisible to the domain again"
        );
    }

    pub(in crate::effects::tests) fn the_domain_reaches_past_a_configured_item() {
        // The shape `PR7-WRAPPERS-EMPTY-DOMAIN` measured in six classified
        // modules: a `#[cfg(test)] use` among the imports at the top of the
        // file, and every production `pub fn` below it. A region that cuts the
        // file at its first `#[cfg(test)]` derives nothing from such a file,
        // and an empty derived set compares equal to an empty record -- so the
        // classification census passes over a module it never read.
        let source = concat!(
            "use std::path::Path;\n",
            "#[cfg(test)]\n",
            "use std::collections::BTreeSet;\n",
            "pub(super) fn below_the_cut(path: &Path) -> bool { path.exists() }\n",
            "impl Display for Thing { fn fmt(&self, f: &mut Formatter<'_>) -> Result { Ok(()) } }\n",
            "#[cfg(test)]\n",
            "pub fn test_only_item() {}\n",
            "#[cfg(test)]\n",
            "impl Thing { pub(crate) fn at(path: &Path) -> Self { Self } }\n",
            "#[cfg(test)]\n",
            "mod tests { pub fn in_the_test_region() {} }\n",
        );
        let found = externally_reachable_fns(source);
        assert_eq!(
            found,
            vec!["below_the_cut".to_owned(), "fmt".to_owned()],
            "a `#[cfg(test)]` item above a production `pub fn` takes it out of the \
             classification domain (`PR7-WRAPPERS-EMPTY-DOMAIN`); derived: {found:?}"
        );
        for excluded in ["test_only_item", "at", "in_the_test_region"] {
            assert!(
                !found.contains(&excluded.to_owned()),
                "`{excluded}` is a test-only item and is in the domain"
            );
        }
    }

    pub(in crate::effects::tests) fn every_classified_module_that_declares_a_visible_fn_has_a_domain()
     {
        // The six modules `PR7-WRAPPERS-EMPTY-DOMAIN` measured, pinned by name:
        // each cuts at a `#[cfg(test)] use` in its imports and each carried an
        // all-empty record in `effects/wrappers.toml` that the census accepted.
        const CUT_AT_A_USE: [&str; 6] = [
            "src/agent/claude.rs",
            "src/agent/codex.rs",
            "src/agent/copilot.rs",
            "src/engine/attempt.rs",
            "src/engine/coordinator.rs",
            "src/engine/resume.rs",
        ];
        for path in CUT_AT_A_USE {
            let source = fs::read_to_string(repo_root().join(path)).expect("a classified module");
            assert!(
                !externally_reachable_fns(&source).is_empty(),
                "{path}: the classification domain is empty, so its record in \
                 effects/wrappers.toml is checked against nothing (`PR7-WRAPPERS-EMPTY-DOMAIN`)"
            );
        }

        // The general form: a classified module whose production code declares
        // a `pub`-visible `fn` derives at least that one. Lexical and
        // sufficient, not a second parser: any of these three spellings in the
        // blanked production code is a visible fn whatever else the file holds.
        let mut with_a_visible_fn = 0_usize;
        for path in CLASSIFIED_MODULES {
            let source = fs::read_to_string(repo_root().join(path)).expect("a classified module");
            let code = production_code(&source);
            let declares_visible_fn = ["pub fn ", "pub(crate) fn ", "pub(super) fn "]
                .iter()
                .any(|spelling| code.contains(spelling));
            if declares_visible_fn {
                with_a_visible_fn += 1;
                assert!(
                    !externally_reachable_fns(&source).is_empty(),
                    "{path}: the production code declares a visible fn and the \
                     classification domain is empty"
                );
            }
        }
        assert!(
            with_a_visible_fn > 40,
            "only {with_a_visible_fn} classified modules declare a visible fn; the scan is \
             not reading the tree"
        );

        // The one classified module whose empty record is legitimate: it
        // declares constants and no `fn` at all, in any region.
        let names = fs::read_to_string(repo_root().join("src/rundir/names.rs"))
            .expect("src/rundir/names.rs");
        assert!(
            !production_code(&names).contains("fn "),
            "src/rundir/names.rs declares a fn now; its all-empty record is no longer \
             legitimately empty"
        );
        assert!(
            externally_reachable_fns(&names).is_empty(),
            "src/rundir/names.rs derives a name and records none"
        );
    }

    pub(in crate::effects::tests) fn the_comment_blanker_models_raw_strings() {
        let exploit = r####"const A: &str = r#"x" //"#; const B: &str = "docker";"####;
        let blanked = blank_comments(exploit);
        assert!(
            blanked.contains("\"docker\""),
            "a raw string erased the literal after it: {blanked}"
        );

        for (label, source) in [
            ("raw, no hashes", r###"let a = r"//"; let b = "docker";"###),
            ("byte raw", r###"let a = br#""//"#; let b = "docker";"###),
            ("byte string", r#"let a = b"\"//"; let b = "docker";"#),
            ("char literal", "let a = '\"'; let b = \"docker\";"),
            ("escaped quote", "let a = \"\\\" //\"; let b = \"docker\";"),
            ("block comment", "/* // */ let b = \"docker\";"),
            ("nested block", "/* /* // */ */ let b = \"docker\";"),
        ] {
            assert!(
                blank_comments(source).contains("\"docker\""),
                "{label}: the needle after it was erased: {}",
                blank_comments(source)
            );
        }

        for source in [
            "// let b = \"docker\";\nlet c = 1;",
            "/* let b = \"docker\"; */ let c = 1;",
            "//! names \"docker\" in prose\nlet c = 1;",
            "/// names \"docker\" in prose\nlet c = 1;",
        ] {
            assert!(
                !blank_comments(source).contains("\"docker\""),
                "a comment naming the needle survived: {}",
                blank_comments(source)
            );
        }

        let counted = "// one\n/* two\nthree */\nlet b = 1;\n";
        assert_eq!(
            blank_comments(counted).lines().count(),
            counted.lines().count(),
            "the blanker lost a line"
        );
    }

    fn notes_section(notes: &str, heading: &str) -> String {
        let after = notes
            .split_once(&format!("\n{heading}\n"))
            .map(|(_, rest)| rest)
            .unwrap_or_else(|| panic!("{heading} is not a heading in the effects notes"));
        after
            .split_once("\n## ")
            .map_or(after, |(section, _)| section)
            .to_owned()
    }

    fn worked_example(section: &str, heading: &str) -> (String, String) {
        let fenced = section
            .split_once("```text\n")
            .map(|(_, rest)| rest)
            .unwrap_or_else(|| panic!("{heading} carries no ```text worked example"));
        let fenced = fenced
            .split_once("\n```")
            .map_or(fenced, |(block, _)| block);
        let mut input = None;
        let mut output = None;
        for line in fenced.lines() {
            if let Some(rest) = line.strip_prefix("in: ") {
                input = Some(rest.to_owned());
            } else if let Some(rest) = line.strip_prefix("out: ") {
                output = Some(rest.to_owned());
            }
        }
        match (input, output) {
            (Some(input), Some(output)) => (input, output),
            _ => panic!("{heading}'s worked example needs an `in: ` line and an `out: ` line"),
        }
    }

    pub(in crate::effects::tests) fn the_notes_give_each_blanker_its_own_contract() {
        const NOTES: &str = "docs/internals/effects.md";
        const DELETING: &str = "## `pub fn blank_comments(source: &str) -> String {`";
        const BLANKING: &str = "## `pub fn blank_comments_and_strings(source: &str) -> String {`";
        const LENGTH_CLAIMS: [&str; 2] = [
            "replaced by spaces of the same length",
            "keeps every byte offset",
        ];

        let notes = fs::read_to_string(repo_root().join(NOTES))
            .expect("the effects notes")
            .replace("\r\n", "\n");
        let deleting = notes_section(&notes, DELETING);
        let blanking = notes_section(&notes, BLANKING);
        let (deleting_in, deleting_out) = worked_example(&deleting, DELETING);
        let (blanking_in, blanking_out) = worked_example(&blanking, BLANKING);

        assert_eq!(
            blank_comments(&deleting_in),
            deleting_out,
            "{DELETING}'s worked example is not what `blank_comments` returns for {deleting_in:?}"
        );
        assert_eq!(
            blank_comments_and_strings(&blanking_in),
            blanking_out,
            "{BLANKING}'s worked example is not what `blank_comments_and_strings` returns for \
             {blanking_in:?}"
        );
        assert!(
            deleting_out.contains("\"docker\"") && !deleting_out.contains("/*"),
            "{DELETING}'s example has to show a comment gone and a literal kept: {deleting_out:?}"
        );
        assert!(
            !blanking_out.contains("docker") && !blanking_out.contains("/*"),
            "{BLANKING}'s example has to show both blanked: {blanking_out:?}"
        );

        for (heading, section, preserves_length) in [
            (
                DELETING,
                &deleting,
                blank_comments(&deleting_in).len() == deleting_in.len(),
            ),
            (
                BLANKING,
                &blanking,
                blank_comments_and_strings(&blanking_in).len() == blanking_in.len(),
            ),
        ] {
            let flattened = section.split_whitespace().collect::<Vec<&str>>().join(" ");
            for claim in LENGTH_CLAIMS {
                assert_eq!(
                    flattened.contains(claim),
                    preserves_length,
                    "{heading}: the notes stating \"{claim}\" is {}, while the helper preserving \
                     its input's length is {preserves_length}",
                    flattened.contains(claim)
                );
            }
        }
    }

    pub(in crate::effects::tests) fn a_multi_byte_char_literal_keeps_the_blankers_phase() {
        for (label, source, leaked) in [
            (
                "the reviewer's pair",
                "const P: (char, char) = ('é','{');\n",
                "{",
            ),
            (
                "a closing brace",
                "const P: (char, char) = ('é','}');\n",
                "}",
            ),
            ("a cascade", "const P: [char; 3] = ['é','{','{'];\n", "{"),
            (
                "four-byte scalar",
                "const P: (char, char) = ('😀','{');\n",
                "{",
            ),
            (
                "three-byte scalar",
                "const P: (char, char) = ('—','{');\n",
                "{",
            ),
            (
                "ascii, the shape that already worked",
                "const P: char = '{';\n",
                "{",
            ),
            (
                "an escape beside it",
                "const P: (char, char) = ('\\u{7f}','{');\n",
                "{",
            ),
            (
                "a Unicode escape as long as its underscores make it",
                "const P: (char, char) = ('\\u{7b____________________}','{');\n",
                "{",
            ),
            (
                "a Unicode escape of six digits",
                "const P: (char, char) = ('\\u{00_00_7b}','{');\n",
                "{",
            ),
            (
                "a byte escape",
                "const P: (u8, char) = (b'\\x7b','{');\n",
                "{",
            ),
            (
                "an escaped quote",
                "const P: (char, char) = ('\\'','{');\n",
                "{",
            ),
            (
                "an escaped backslash",
                "const P: (char, char) = ('\\\\','{');\n",
                "{",
            ),
        ] {
            let blanked = blank_comments_and_strings(source);
            assert!(
                !blanked.contains(leaked),
                "{label}: a `{leaked}` inside a char literal survived as code: {blanked:?}"
            );
            assert_eq!(
                blanked.len(),
                source.len(),
                "{label}: the blanking moved byte offsets, which callers map to lines"
            );
            assert!(
                blanked.contains("const P"),
                "{label}: the blanking ate the code around the literal: {blanked:?}"
            );
        }

        for lifetime in [
            "fn f<'a>(x: &'a str) -> &'a str { x }\n",
            "fn g<'a,'b>(x: &'a str, y: &'b str) -> usize { x.len() + y.len() }\n",
            "fn h(x: &'_ str) -> &'static str { \"k\" }\n",
        ] {
            let blanked = blank_comments_and_strings(lifetime);
            assert!(
                blanked.contains("str") && blanked.contains('{'),
                "a lifetime was read as a char literal and swallowed the code after \
                 it: {lifetime:?} -> {blanked:?}"
            );
        }
        let kept = "const P: (char, char) = ('é','{');\nlet q = '😀';\nlet r = '—';\n";
        assert_eq!(
            blank_comments(kept),
            kept,
            "the sibling blanker altered a source that holds no comment at all"
        );
        let commented = blank_comments("const P: char = 'é'; // names \"docker\"\n");
        assert!(
            commented.starts_with("const P: char = 'é';"),
            "the sibling blanker lost the literal: {commented:?}"
        );
        assert!(
            !commented.contains("docker"),
            "the comment after a multi-byte char literal survived: {commented:?}"
        );

        let attacked = "fn above() {}\n\
                        #[cfg(test)]\n\
                        mod tests {\n\
                            const P: (char, char) = ('é','{');\n\
                        }\n\
                        fn forged_below() {}\n";
        let region = production_code(attacked);
        assert!(region.contains("fn above()"), "{region:?}");
        assert!(
            region.contains("fn forged_below()"),
            "the desync blanked from the test module to end of file, so every \
             production item below it is invisible to every census: {region:?}"
        );
        assert!(
            !region.contains("const P"),
            "the test module itself must still be removed: {region:?}"
        );
    }

    pub(in crate::effects::tests) fn an_unfindable_item_end_blanks_the_attribute() {
        let region = production_code("fn above() {}\n#[cfg(test)]\nmod tests {\nfn below() {}\n");
        assert!(region.contains("fn above()"), "{region:?}");
        assert!(
            region.contains("fn below()"),
            "an unbalanced brace blanked the rest of the file: {region:?}"
        );
        assert!(
            region.contains("mod tests {"),
            "the test module must read as production when the region cannot find its \
             end, so the censuses go loud: {region:?}"
        );
        assert!(
            !region.contains("#[cfg(test)]"),
            "the attribute itself is still removed: {region:?}"
        );

        let region = production_code("fn above() {}\n#[cfg(test)]\nuse a::b\n");
        assert!(region.contains("fn above()"), "{region:?}");
        assert!(
            region.contains("use a::b"),
            "an unterminated item blanked the rest of the file: {region:?}"
        );
        assert!(!region.contains("#[cfg(test)]"), "{region:?}");

        let region =
            production_code("fn above() {}\n#[cfg(test)]\nmod tests {\n}\nfn below() {}\n");
        assert!(region.contains("fn above()") && region.contains("fn below()"));
        assert!(
            !region.contains("mod tests"),
            "a well-formed item is still removed: {region:?}"
        );
    }

    pub(in crate::effects::tests) fn the_whole_file_modules_are_read_from_the_declarations() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("src is readable") {
                let path = entry.expect("a directory entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    files.push(path);
                }
            }
        }

        let modules = crate::effects::census_domain::whole_file_test_modules(&root, &files, 13);
        fn relative<'a>(root: &std::path::Path, path: &'a std::path::Path) -> &'a std::path::Path {
            path.strip_prefix(root).unwrap_or(path)
        }
        fn sorted(mut paths: Vec<&std::path::Path>) -> Vec<&std::path::Path> {
            paths.sort_unstable();
            paths
        }
        let expected = sorted(
            WHOLE_FILE_TEST_MODULES
                .iter()
                .map(std::path::PathBuf::as_path)
                .collect(),
        );
        let stem_is_tests =
            |path: &std::path::Path| path.file_stem().is_some_and(|stem| stem == "tests");
        let expected_named_tests: Vec<&std::path::Path> = expected
            .iter()
            .copied()
            .filter(|path| stem_is_tests(path))
            .collect();
        let expected_not_named_tests: Vec<&std::path::Path> = expected
            .iter()
            .copied()
            .filter(|path| !stem_is_tests(path))
            .collect();

        let named = sorted(
            modules
                .iter()
                .filter(|path| path.file_stem().is_none_or(|stem| stem != "tests"))
                .map(|path| relative(&root, path))
                .collect(),
        );
        assert_eq!(
            named, expected_not_named_tests,
            "these are the whole-file test modules a `file_stem == \"tests\"` rule does not see, and \
             a census that uses that rule reads them as production"
        );
        let resolved = sorted(modules.iter().map(|path| relative(&root, path)).collect());
        assert_eq!(
            resolved, expected,
            "the crate's whole-file test modules are not what `WHOLE_FILE_TEST_MODULES` lists; a \
             census skipping only the ones named `tests.rs` by file name leaves the rest inside \
             its domain"
        );

        let declarations =
            crate::effects::census_domain::declared_whole_file_test_modules(&root, &files);
        fn declared_file<'a>(
            root: &std::path::Path,
            declaration: &'a crate::effects::census_domain::TestModuleDeclaration,
        ) -> &'a std::path::Path {
            let resolved =
                crate::effects::census_domain::sole_present(&declaration.candidates, &|path| {
                    path.is_file()
                })
                .expect("a derived declaration resolves to exactly one file");
            relative(root, resolved)
        }
        let declared = sorted(
            declarations
                .iter()
                .map(|declaration| declared_file(&root, declaration))
                .collect(),
        );
        assert_eq!(
            declared, expected,
            "reading the declarations resolves a different population than \
             `WHOLE_FILE_TEST_MODULES` lists"
        );
        let is_literal = |declaration: &crate::effects::census_domain::TestModuleDeclaration| {
            is_the_literal_mod_tests_form(
                &declaration.name,
                &declaration.inline_path,
                &declaration.guard,
            )
        };
        let literal = sorted(
            declarations
                .iter()
                .filter(|declaration| is_literal(declaration))
                .map(|declaration| declared_file(&root, declaration))
                .collect(),
        );
        let declaring: Vec<String> = declarations
            .iter()
            .filter(|declaration| is_literal(declaration))
            .map(|declaration| {
                relative(&root, &declaration.declared_in)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            literal, expected_named_tests,
            "these are the whole-file test modules declared by a literal `#[cfg(test)] mod \
             tests;` -- that name, that guard, at their parent's own top level -- and the \
             file-name rule finds exactly them. A declaration narrowed to `all(test, <platform>)` \
             resolves to a `tests.rs` and is missing from the left side only: it is still a \
             whole-file test module, it is not this form, and what a file-name census should do \
             about a module that exists on only some platforms is the question this failure \
             asks. The declarations were read in {declaring:?}"
        );
        let inherited: Vec<(&std::path::Path, String, Vec<String>, String)> = declarations
            .iter()
            .filter(|declaration| !declaration.inline_path.is_empty())
            .map(|declaration| {
                (
                    relative(&root, &declaration.declared_in),
                    declaration.name.clone(),
                    declaration.inline_path.clone(),
                    declaration.guard.clone(),
                )
            })
            .collect();
        assert_eq!(
            inherited,
            vec![(
                std::path::Path::new("agent/proc.rs"),
                "readiness".to_owned(),
                vec!["test_support".to_owned()],
                "test".to_owned(),
            )],
            "the declarations reached only through an inline `cfg(test)` ancestor are not what \
             this tree contains"
        );
    }

    pub(in crate::effects::tests) fn the_configured_item_is_removed_and_the_rest_kept() {
        let region = production_code("fn above() {}\n#[cfg(test)]\nmod tests;\nfn below() {}\n");
        assert!(region.contains("fn above()"), "{region:?}");
        assert!(region.contains("fn below()"), "{region:?}");
        assert!(!region.contains("mod tests;"), "{region:?}");
        assert_eq!(
            region.lines().count(),
            4,
            "the item is blanked in place, so line numbers survive: {region:?}"
        );

        let region = production_code(
            "fn above() {}\n#[cfg(test)]\nmod tests {\n    fn inner() { let _ = 1; }\n}\nfn below() {}\n",
        );
        assert!(region.contains("fn above()") && region.contains("fn below()"));
        assert!(!region.contains("fn inner()"), "{region:?}");

        let region = production_code("use a::b;\n#[cfg(test)]\nuse c::d;\nfn below() {}\n");
        assert!(region.contains("use a::b;") && region.contains("fn below()"));
        assert!(!region.contains("use c::d;"), "{region:?}");

        let region = production_code("#[cfg(test)]\nuse a::{b, c};\nfn below() {}\n");
        assert!(region.contains("fn below()"));
        assert!(!region.contains("a::"), "{region:?}");
        assert!(
            !region.contains(';'),
            "the trailing `;` goes with the item: {region:?}"
        );

        let region = production_code(
            "#[cfg(test)]\npub const ALL: &str = \"x\";\n#[cfg(test)]\npub(super) fn f() { g(); }\nfn below() {}\n",
        );
        assert!(region.contains("fn below()"));
        assert!(
            !region.contains("ALL") && !region.contains("g();"),
            "{region:?}"
        );

        let region = production_code(
            "struct S {\n    kept: u8,\n    #[cfg(test)]\n    gone: Option<u8>,\n    also_kept: u8,\n}\n",
        );
        assert!(region.contains("kept: u8,") && region.contains("also_kept: u8,"));
        assert!(!region.contains("gone"), "{region:?}");

        let region =
            production_code("#[cfg(test)]\n#[allow(dead_code)]\nmod tests;\nfn below() {}\n");
        assert!(region.contains("fn below()"));
        assert!(!region.contains("mod tests;"), "{region:?}");
    }

    pub(in crate::effects::tests) fn typed_test_functions_are_removed_and_later_code_is_kept() {
        for prefix in [
            "",
            "pub ",
            "pub(super) ",
            "pub(in crate::effects) ",
            "pub(crate) async unsafe ",
            "extern \"C\" ",
        ] {
            let source = format!(
                "#[cfg(test)]\n{prefix}fn excluded() -> Result<(RunReport, RunState), UpstrokeError> {{\n\
                 let hidden = HostRunner::new();\nOk((report, state))\n}}\n\
                 fn production() {{ let visible = HostRunner::new(); }}\n"
            );
            let region = production_code(&source);
            assert!(
                !region.contains("hidden"),
                "typed test function survived its cfg removal: {prefix:?}: {region:?}"
            );
            assert!(region.contains("fn production()"), "{prefix:?}: {region:?}");
            assert!(region.contains("let visible"), "{prefix:?}: {region:?}");
            assert_eq!(region.matches("HostRunner::new(").count(), 1, "{region:?}");
            assert_eq!(region.len(), source.len());
            assert_eq!(region.lines().count(), source.lines().count());
        }

        for field in [
            "callback: fn() -> Result<A, B>,",
            "generic: BTreeMap<K, V>,",
            "callback: unsafe extern \"C\" fn() -> Result<A, B>,",
        ] {
            let source =
                format!("struct S {{ #[cfg(test)] {field} kept: u8, }}\nfn production() {{}}\n");
            let region = production_code(&source);
            assert!(region.contains("kept: u8"), "{field}: {region:?}");
            assert!(region.contains("fn production()"), "{field}: {region:?}");
        }

        for source in [
            "#[cfg(test)]\nfn broken() -> Result<A, B>\nfn production() { let visible = HostRunner::new(); }\n",
            "#[cfg(test)] fn broken() -> Result<A, B> { fn production() {}\n",
            "#[cfg(test)] fn broken() -> Result<A, B>\n",
            "#[cfg(test)] pub(super fn broken() -> Result<A, B> { fn production() {}\n",
        ] {
            let region = production_code(source);
            assert!(
                region.contains("fn broken()"),
                "an incomplete test item swallowed later source: {region:?}"
            );
            assert_eq!(
                region.contains("fn production()"),
                source.contains("fn production()")
            );
        }
    }

    pub(in crate::effects::tests) fn a_test_only_element_is_removed_to_where_rustc_ends_it() {
        fn kept(source: &str) -> String {
            let region = production_code(source);
            assert_eq!(region.len(), source.len(), "{source:?}");
            assert_eq!(
                region.matches('\n').count(),
                source.matches('\n').count(),
                "{source:?}"
            );
            region.split_whitespace().collect::<Vec<_>>().join(" ")
        }

        const BEFORE: &str = "fn kept_before() { let r#x = o!(); }\n";
        const AFTER: &str = "fn kept_after() { n!(); }\n";
        for (what, item) in [
            (
                "two type parameters",
                "#[cfg(test)]\nfn t<A: Copy, B>(a: A, b: B) {\n    assert!(true);\n}\n",
            ),
            (
                "a lifetime and a type",
                "#[cfg(test)]\nfn t<'a, T>(x: &'a T) -> &'a T {\n    panic!()\n}\n",
            ),
            (
                "a two-argument return type on a generic function",
                "#[cfg(test)]\npub(crate) fn t<T>() -> Result<T, String> {\n    Err(format!(\"x\"))\n}\n",
            ),
            (
                "a where clause of two predicates",
                "#[cfg(test)]\nfn t<T>(x: T) where T: Copy, T: Clone {\n    assert!(true);\n}\n",
            ),
            (
                "the tree's own `same_path`, made generic",
                "#[cfg(test)]\npub(crate) fn same_path<L: AsRef<Path>, R: AsRef<Path>>(left: L, right: R) -> bool {\n    panic!()\n}\n",
            ),
            (
                "a const block in the return type",
                "#[cfg(test)]\nfn t() -> Foo<{ N > 1 }> {\n    m!();\n}\n",
            ),
            (
                "an array in the return type",
                "#[cfg(test)]\nfn t() -> [u8; 2] {\n    m!()\n}\n",
            ),
            (
                "an arrow at the header's own level",
                "#[cfg(test)]\nfn t() -> impl Fn() -> Result<u8, u8> {\n    m!()\n}\n",
            ),
            (
                "a generic impl",
                "#[cfg(test)]\nimpl<A, B> X<A, B> {\n    m!();\n    fn f() { m!(); }\n}\n",
            ),
            (
                "a generic trait impl with a where clause",
                "#[cfg(test)]\nimpl<T> Tr<T, u8> for W<T> where T: A, T: B {\n    fn f() { m!(); }\n}\n",
            ),
            (
                "an unsafe generic impl",
                "#[cfg(test)]\nunsafe impl<A, B> Send for X<A, B> {}\n",
            ),
            (
                "a generic struct",
                "#[cfg(test)]\nstruct S<A, B> {\n    a: A,\n    b: [u8; m!()],\n}\n",
            ),
            (
                "a generic tuple struct with a where clause",
                "#[cfg(test)]\nstruct S<A, B>(A, B) where A: Copy, B: Copy;\n",
            ),
            (
                "a generic enum",
                "#[cfg(test)]\nenum E<A, B> {\n    X(A),\n    Y(B) = m!(),\n}\n",
            ),
            (
                "a generic union",
                "#[cfg(test)]\nunion U<A: Copy, B: Copy> {\n    a: A,\n    b: B,\n}\n",
            ),
            (
                "a generic trait with supertraits",
                "#[cfg(test)]\ntrait T<A, B>: Into<(A, B)> + From<A, B> {\n    fn f(&self) { m!(); }\n}\n",
            ),
            (
                "an unsafe generic trait",
                "#[cfg(test)]\nunsafe trait T<A, B> {}\n",
            ),
            ("a type alias", "#[cfg(test)]\ntype M = HashMap<u8, u8>;\n"),
            (
                "a static of a generic type",
                "#[cfg(test)]\nstatic S: Mutex<HashMap<u8, u8>> = m!();\n",
            ),
            (
                "a static of an array type",
                "#[cfg(test)]\nstatic S: [u8; 2] = [0; 2];\n",
            ),
            (
                "a static whose initializer holds a block in a call",
                "#[cfg(test)]\nstatic S: u8 = f({ g(); 1 });\n",
            ),
            (
                "a const of a generic type",
                "#[cfg(test)]\nconst C: HashMap<u8, u8> = m!();\n",
            ),
            (
                "a const whose initializer compares",
                "#[cfg(test)]\nconst C: bool = 1 < 2 && m!(a, b) > 0;\n",
            ),
            (
                "a module-level `const _`",
                "#[cfg(test)]\nconst _: () = {\n    m!();\n};\n",
            ),
            ("a const function", "#[cfg(test)]\nconst fn t<A, B>() {}\n"),
            (
                "a const foreign-ABI function",
                "#[cfg(test)]\nconst extern \"C\" fn t<A, B>() {}\n",
            ),
            ("an async function", "#[cfg(test)]\nasync fn t<A, B>() {}\n"),
            (
                "an unsafe function",
                "#[cfg(test)]\nunsafe fn t<A, B>() {}\n",
            ),
            (
                "a foreign-ABI function",
                "#[cfg(test)]\nextern \"C\" fn t<A, B>() {}\n",
            ),
            (
                "every qualifier before `fn`",
                "#[cfg(test)]\npub(in crate::a) const unsafe extern \"C\" fn t<A, B>() {\n    m!();\n}\n",
            ),
            (
                "a raw function name",
                "#[cfg(test)]\nfn r#match<A, B>() {\n    m!();\n}\n",
            ),
            (
                "a non-ASCII function name",
                "#[cfg(test)]\nfn \u{e9}<A, B>() {\n    m!();\n}\n",
            ),
            (
                "an impl of a trait whose name ends `fn` after a non-ASCII letter",
                "#[cfg(test)]\nimpl \u{c9}fn for u8 {\n    const LINE: u32 = line!();\n}\n",
            ),
            (
                "a supertrait whose name ends `fn` after a non-ASCII letter, before a `where`",
                "#[cfg(test)]\ntrait Sub: \u{c9}fn where Self: Sized {\n    fn f(&self) { m!(); }\n}\n",
            ),
            (
                "attributes written before the gate",
                "#[doc = concat!(\"a\", \"b\")]\n#[inline]\n#[cfg(test)]\n#[allow(dead_code)]\nfn t<A, B>() {}\n",
            ),
            (
                "a macro definition",
                "#[cfg(test)]\nmacro_rules! m {\n    () => {};\n}\n",
            ),
            (
                "a macro definition in parentheses",
                "#[cfg(test)]\nmacro_rules! m (\n    () => {}\n);\n",
            ),
            (
                "a thread-local",
                "#[cfg(test)]\nthread_local! {\n    static X: HashMap<u8, u8> = HashMap::new();\n}\n",
            ),
            (
                "an extern block",
                "#[cfg(test)]\nunsafe extern \"C\" {\n    safe fn f(a: u8, b: u8);\n}\n",
            ),
            (
                "an inline module",
                "#[cfg(test)]\npub(crate) mod tests {\n    m!();\n}\n",
            ),
            (
                "a gate the predicate entails but does not spell alone",
                "#[cfg(all(unix, test))]\nmacro_rules! m {\n    () => {};\n}\n",
            ),
            (
                "a gate with a literal in its predicate",
                "#[cfg(all(test, feature = \"a, b)\"))]\nthread_local! {\n    static X: u8 = 0;\n}\n",
            ),
            (
                "a gate that negates its negation",
                "#[cfg(not(not(test)))]\nconst C: u8 = m!();\n",
            ),
            (
                "a trailing comma in the predicate's list",
                "#[cfg(all(test,))]\nconst C: u8 = m!();\n",
            ),
            (
                "a gate no build satisfies",
                "#[cfg(any())]\nconst C: u8 = m!();\n",
            ),
            (
                "a gate's tokens spaced and commented",
                "# /* a */ [ cfg /* b */ ( all ( unix , // c\n test ) ) ]\nfn t<A, B>() { m!(); }\n",
            ),
            (
                "a separator rustc reads as whitespace in the predicate",
                "#[cfg(all(unix,\u{200e}test))]\nconst C: u8 = m!();\n",
            ),
        ] {
            assert_eq!(
                kept(&format!("{BEFORE}{item}{AFTER}")),
                "fn kept_before() { let r#x = o!(); } fn kept_after() { n!(); }",
                "{what}: {item:?}"
            );
        }

        for (what, source, region) in [
            (
                "the only generic parameter of an impl",
                "impl<#[cfg(test)] 'a> NoHooks {\n    pub fn kept(&self) { m!(); }\n}\n",
                "impl< > NoHooks { pub fn kept(&self) { m!(); } }",
            ),
            (
                "the last generic parameter, after a generic bound",
                "impl<T: Into<u8>, #[cfg(test)] U: Into<(u8, u8)>> X<T> {\n    pub fn kept() { m!(); }\n}\n",
                "impl<T: Into<u8>, > X<T> { pub fn kept() { m!(); } }",
            ),
            (
                "a middle generic parameter",
                "impl<'a, #[cfg(test)] 'b, T> X<'a, T> {\n    pub fn kept() {}\n}\n",
                "impl<'a, T> X<'a, T> { pub fn kept() {} }",
            ),
            (
                "a generic parameter after a closure-typed bound",
                "impl<F: Fn(u8) -> u8, #[cfg(test)] U> X<F> {\n    pub fn kept() { m!(); }\n}\n",
                "impl<F: Fn(u8) -> u8, > X<F> { pub fn kept() { m!(); } }",
            ),
            (
                "a generic parameter bounded by a closure type",
                "pub fn f<#[cfg(test)] F: Fn(u8) -> u8>() {\n    kept();\n}\n",
                "pub fn f< >() { kept(); }",
            ),
            (
                "the last generic parameter of a trait",
                "pub trait T<#[cfg(test)] A> {\n    fn kept(&self) { m!(); }\n}\n",
                "pub trait T< > { fn kept(&self) { m!(); } }",
            ),
            (
                "a defaulted generic parameter of a struct",
                "pub struct S<#[cfg(test)] A = Vec<u8>> {\n    pub kept: u8,\n}\n",
                "pub struct S< > { pub kept: u8, }",
            ),
            (
                "a const generic parameter with a block default",
                "pub enum E<T, #[cfg(test)] const N: bool = { 1 > 0 }> {\n    Kept(T),\n}\n",
                "pub enum E<T, > { Kept(T), }",
            ),
            (
                "a later generic parameter of a function",
                "fn f<T, #[cfg(test)] U>() {\n    kept();\n}\n",
                "fn f<T, >() { kept(); }",
            ),
            (
                "a later generic parameter of a struct",
                "struct S<T, #[cfg(test)] U> {\n    kept: T,\n}\n",
                "struct S<T, > { kept: T, }",
            ),
            (
                "a later generic parameter of a union",
                "union U<T: Copy, #[cfg(test)] V> {\n    kept: T,\n}\n",
                "union U<T: Copy, > { kept: T, }",
            ),
            (
                "a later generic parameter of a trait",
                "trait Tr<T, #[cfg(test)] U> {\n    fn kept(&self) {}\n}\n",
                "trait Tr<T, > { fn kept(&self) {} }",
            ),
            (
                "a later generic parameter of a type alias",
                "type A<T, #[cfg(test)] U> = Vec<T>;\n",
                "type A<T, > = Vec<T>;",
            ),
            (
                "a later generic parameter of a raw-named function",
                "fn r#match<T, #[cfg(test)] U>() {\n    kept();\n}\n",
                "fn r#match<T, >() { kept(); }",
            ),
            (
                "a later generic parameter of a non-ASCII-named function",
                "fn \u{e9}<T, #[cfg(test)] U>() {\n    kept();\n}\n",
                "fn \u{e9}<T, >() { kept(); }",
            ),
            (
                "a binder's parameter",
                "pub fn f() where for<#[cfg(test)] 'a> &'a u8: Copy {\n    kept();\n}\n",
                "pub fn f() where for< > &'a u8: Copy { kept(); }",
            ),
            (
                "a binder's later parameter",
                "pub fn f() where for<'a, #[cfg(test)] 'b> &'a u8: Copy {\n    kept();\n}\n",
                "pub fn f() where for<'a, > &'a u8: Copy { kept(); }",
            ),
            (
                "a generic parameter after an array in a bound",
                "struct S<T: Into<[u8; 2]>, #[cfg(test)] U> {\n    kept: T,\n}\n",
                "struct S<T: Into<[u8; 2]>, > { kept: T, }",
            ),
            (
                "a generic parameter after a const default block",
                "struct S<const N: usize = { 1 }, #[cfg(test)] U> {\n    kept: [u8; N],\n}\n",
                "struct S<const N: usize = { 1 }, > { kept: [u8; N], }",
            ),
            (
                "a generic parameter bounded by an array",
                "struct S<#[cfg(test)] T: Into<[u8; 2]>> {\n    kept: u8,\n}\n",
                "struct S< > { kept: u8, }",
            ),
            (
                "an argument in a call inside a closure's body",
                "const C: u8 = g(|a| f(a, #[cfg(test)] |x| x, kept!()));\n",
                "const C: u8 = g(|a| f(a, kept!()));",
            ),
            (
                "an element of an array inside a closure's body",
                "const C: u8 = g(|a| [a, #[cfg(test)] |x| x, kept!()]);\n",
                "const C: u8 = g(|a| [a, kept!()]);",
            ),
            (
                "a field of a struct expression inside a closure's body",
                "const C: u8 = g(|a| S { a, #[cfg(test)] b: |x| x, c: kept!() });\n",
                "const C: u8 = g(|a| S { a, c: kept!() });",
            ),
            (
                "a closure argument after a `|` a comparison follows",
                "const C: bool = f(a | b > c, #[cfg(test)] |x| x, kept!());\n",
                "const C: bool = f(a | b > c, kept!());",
            ),
            (
                "the first parameter of a closure",
                "fn f() {\n    let g = |#[cfg(test)] a: u8| {\n        kept()\n    };\n}\n",
                "fn f() { let g = | | { kept() }; }",
            ),
            (
                "a later parameter of a closure",
                "fn f() {\n    let g = |a: Vec<u8>, #[cfg(test)] b: u8| kept(a);\n}\n",
                "fn f() { let g = |a: Vec<u8>, | kept(a); }",
            ),
            (
                "a closure parameter beside a comma",
                "const C: u8 = f(|a, #[cfg(test)] b| a, kept!());\n",
                "const C: u8 = f(|a, | a, kept!());",
            ),
            (
                "a later parameter of a generic function",
                "fn f<T>(a: u8, #[cfg(test)] b: u8) {\n    kept();\n}\n",
                "fn f<T>(a: u8, ) { kept(); }",
            ),
            (
                "a let statement with a generic type",
                "fn f() {\n    #[cfg(test)]\n    let m: HashMap<u8, u8> = HashMap::new();\n    kept();\n}\n",
                "fn f() { kept(); }",
            ),
            (
                "an if statement and its else chain",
                "fn f() {\n    #[cfg(test)]\n    if a {\n        one();\n    } else if b {\n        two();\n    } else {\n        three();\n    }\n    kept();\n}\n",
                "fn f() { kept(); }",
            ),
            (
                "an if statement that indexes in its condition",
                "fn f() {\n    #[cfg(test)]\n    if a[0] {\n        one();\n    } else {\n        two();\n    }\n    kept();\n}\n",
                "fn f() { kept(); }",
            ),
            (
                "an if statement with a turbofish in its condition",
                "fn f() {\n    #[cfg(test)]\n    if g::<u8, u8>() {\n        one();\n    }\n    kept();\n}\n",
                "fn f() { kept(); }",
            ),
            (
                "an inline const block",
                "fn f() {\n    #[cfg(test)]\n    const {\n        assert!(true);\n    }\n    kept();\n}\n",
                "fn f() { kept(); }",
            ),
            (
                "an unsafe block",
                "fn f() {\n    #[cfg(test)]\n    unsafe {\n        g();\n    }\n    kept();\n}\n",
                "fn f() { kept(); }",
            ),
            (
                "a closure argument that compares",
                "const _: () = {\n    f(#[cfg(test)] |a| a < 1, kept!());\n};\n",
                "const _: () = { f( kept!()); };",
            ),
            (
                "a field of a struct expression that compares",
                "const C: S = S {\n    #[cfg(test)]\n    a: 1 < 2,\n    b: kept!(),\n};\n",
                "const C: S = S { b: kept!(), };",
            ),
            (
                "an argument after a comparison",
                "const C: bool = f(a < b, #[cfg(test)] c, kept!());\n",
                "const C: bool = f(a < b, kept!());",
            ),
            (
                "a closure argument after a comparison",
                "const C: u8 = f(a < b, #[cfg(test)] |x| x > 1, kept!());\n",
                "const C: u8 = f(a < b, kept!());",
            ),
            (
                "a closure argument after comparing a raw `for`",
                "const C: u8 = f(r#for < a, #[cfg(test)] |x| x > 1, kept!());\n",
                "const C: u8 = f(r#for < a, kept!());",
            ),
            (
                "an or-pattern arm after a guarded or-pattern arm",
                "const C: u8 = match x {\n    A | B if y < z => 1,\n    #[cfg(test)]\n    C | E => 3,\n    D => kept!(),\n};\n",
                "const C: u8 = match x { A | B if y < z => 1, D => kept!(), };",
            ),
            (
                "a block arm after an or-pattern",
                "const C: u8 = match x {\n    A | B => 1,\n    #[cfg(test)]\n    C => { 2 }\n    D => kept!(),\n};\n",
                "const C: u8 = match x { A | B => 1, D => kept!(), };",
            ),
            (
                "a variant whose discriminant shifts",
                "enum E {\n    #[cfg(test)]\n    A = 1 << 2,\n    B = kept!(),\n}\n",
                "enum E { B = kept!(), }",
            ),
            (
                "a tuple field of a function-pointer type",
                "struct S(#[cfg(test)] unsafe fn(u8) -> u8, pub u16);\n",
                "struct S( pub u16);",
            ),
            (
                "a field whose type holds a `;`",
                "struct S {\n    #[cfg(test)]\n    gone: [u8; 2],\n    kept: u8,\n}\n",
                "struct S { kept: u8, }",
            ),
            (
                "the last field, with no comma after it",
                "struct S {\n    kept: u8,\n    #[cfg(test)]\n    gone: u8\n}\n",
                "struct S { kept: u8, }",
            ),
            (
                "an inner attribute before a test-only item",
                "mod m {\n    #![allow(dead_code)]\n    #[cfg(test)]\n    fn gone() {}\n    fn kept() {}\n}\n",
                "mod m { #![allow(dead_code)] fn kept() {} }",
            ),
            (
                "an earlier item's attribute",
                "#[inline]\nfn kept() {}\n#[cfg(test)]\nfn gone() {}\n",
                "#[inline] fn kept() {}",
            ),
            (
                "an attribute that does not close, then a gate",
                "#[\nfn kept() {}\n#[cfg(test)]\nfn gone() {}\n",
                "#[ fn kept() {}",
            ),
            (
                "a `>` no `<` opened, in a header",
                "#[cfg(test)] fn t() > u8 { m!(); }\n",
                "fn t() > u8 { m!(); }",
            ),
            (
                "a header its block closes before a body",
                "mod m {\n    #[cfg(test)]\n    fn t()\n}\nconst K: u8 = { 1 };\n",
                "mod m { fn t() } const K: u8 = { 1 };",
            ),
            (
                "a header with an unclosed parameter list",
                "fn kept() {}\n#[cfg(test)]\nfn t(\n",
                "fn kept() {} fn t(",
            ),
            (
                "an item with no `;` before the next function",
                "#[cfg(test)]\nstatic S: u8 = 1\nfn kept() {}\nconst Y: u8 = 2;\n",
                "static S: u8 = 1 fn kept() {} const Y: u8 = 2;",
            ),
            (
                "an item with no `;` before a non-ASCII-named function",
                "#[cfg(test)]\nstatic S: u8 = 1\nfn \u{e9}() {}\nconst Y: u8 = 2;\n",
                "static S: u8 = 1 fn \u{e9}() {} const Y: u8 = 2;",
            ),
            (
                "a header with no body before the next function",
                "#[cfg(test)]\nimpl X\nfn kept() {}\nconst Y: u8 = 2;\n",
                "impl X fn kept() {} const Y: u8 = 2;",
            ),
            (
                "a header with no body before a non-ASCII-named function",
                "#[cfg(test)]\nimpl X\nfn \u{e9}() {}\nconst Y: u8 = 2;\n",
                "impl X fn \u{e9}() {} const Y: u8 = 2;",
            ),
            (
                "an item its block closes before its `;`",
                "mod m {\n    #[cfg(test)]\n    static S: u8 = 1\n}\nconst K: u8 = 2;\n",
                "mod m { static S: u8 = 1 } const K: u8 = 2;",
            ),
            (
                "an item with an unclosed initializer",
                "fn kept() {}\n#[cfg(test)]\nstatic S: [u8; 1] = [0;\n",
                "fn kept() {} static S: [u8; 1] = [0;",
            ),
            (
                "a generic parameter a `)` ends",
                "fn f<#[cfg(test)] T) > kept() {}\n",
                "fn f< T) > kept() {}",
            ),
            (
                "a generic parameter a `]` ends",
                "fn f<#[cfg(test)] T] > kept() {}\n",
                "fn f< T] > kept() {}",
            ),
            (
                "a generic parameter a `}` ends",
                "fn f<#[cfg(test)] T} > kept() {}\n",
                "fn f< T} > kept() {}",
            ),
            (
                "a generic parameter a `;` ends",
                "fn f<#[cfg(test)] T; > kept() {}\n",
                "fn f< T; > kept() {}",
            ),
            (
                "a generic parameter the file ends",
                "fn kept() {}\nfn f<#[cfg(test)] T\n",
                "fn kept() {} fn f< T",
            ),
            (
                "a header its parenthesis closes before a body",
                "f(#[cfg(test)] fn t()) {}\n",
                "f( fn t()) {}",
            ),
            (
                "a header its bracket closes before a body",
                "[#[cfg(test)] fn t()] {}\n",
                "[ fn t()] {}",
            ),
            (
                "a header with a `;` inside its angle brackets",
                "#[cfg(test)] fn t<A; B>() {}\n",
                "fn t<A; B>() {}",
            ),
            (
                "an item its parenthesis closes before its `;`",
                "f(#[cfg(test)] static S: u8 = 1); fn kept() {}\n",
                "f( static S: u8 = 1); fn kept() {}",
            ),
            (
                "an item its bracket closes before its `;`",
                "[#[cfg(test)] static S: u8 = 1]; fn kept() {}\n",
                "[ static S: u8 = 1]; fn kept() {}",
            ),
            (
                "a generic parameter with an unclosed group",
                "fn kept() {}\nfn f<#[cfg(test)] T: Fn(\n",
                "fn kept() {} fn f< T: Fn(",
            ),
            (
                "an if statement a `;` ends before any block",
                "#[cfg(test)] if a; fn kept() { m!(); }\n",
                "fn kept() { m!(); }",
            ),
            (
                "an if statement its block closes",
                "mod m {\n    #[cfg(test)]\n    if a\n}\nfn kept() { m!(); }\n",
                "mod m { } fn kept() { m!(); }",
            ),
            (
                "an if argument its parenthesis closes before a block",
                "const C: u8 = f(#[cfg(test)] if a) { m!() };\nfn kept() {}\n",
                "const C: u8 = f( ) { m!() }; fn kept() {}",
            ),
            (
                "an if element its bracket closes before a block",
                "const C: [u8; 1] = [#[cfg(test)] if a] { m!() };\nfn kept() {}\n",
                "const C: [u8; 1] = [ ] { m!() }; fn kept() {}",
            ),
            (
                "an element with an unclosed group",
                "fn kept() {}\nconst C: u8 = f(#[cfg(test)] g(\n",
                "fn kept() {} const C: u8 = f( g(",
            ),
        ] {
            assert_eq!(kept(source), region, "{what}: {source:?}");
        }

        for (what, source) in [
            (
                "a production gate",
                "#[cfg(not(test))]\nfn kept() { m!(); }\n",
            ),
            (
                "a gate some production build satisfies",
                "#[cfg(any(unix, test))]\nfn kept() { m!(); }\n",
            ),
            (
                "a gate that compares `test`",
                "#[cfg(test = \"x\")]\nfn kept() { m!(); }\n",
            ),
            (
                "a conditional attribute, which configures nothing out",
                "#[cfg_attr(test, allow(dead_code))]\nfn kept() { m!(); }\n",
            ),
            (
                "a gate applied through `cfg_attr`, which this region does not read",
                "#[cfg_attr(unix, cfg(test))]\nfn kept() { m!(); }\n",
            ),
            (
                "an inner gate, which this region does not read",
                "mod kept {\n    #![cfg(test)]\n    fn kept() { m!(); }\n}\n",
            ),
            (
                "a raw attribute name",
                "#[r#cfg(test)]\nfn kept() { m!(); }\n",
            ),
            (
                "an unreadable predicate",
                "#[cfg(test, unix)]\nfn kept() { m!(); }\n",
            ),
            (
                "another attribute whose arguments read as a gate",
                "#[my_attribute(test)]\nfn kept() { m!(); }\n",
            ),
            ("a gate with no predicate", "const C: u8 = f(#[cfg] a);\n"),
            (
                "a predicate with a doc comment in it, which reads as no gate",
                "#[cfg(all(test /** d */))]\nfn kept() { m!(); }\n",
            ),
        ] {
            assert_eq!(
                kept(source),
                blank_comments_and_strings(source)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                "{what}: {source:?}"
            );
        }
    }

    pub(in crate::effects::tests) fn a_configured_attribute_in_prose_is_inert() {
        for prose in [
            "/* a fixture in prose: #[cfg(test)] opens a test module */\nfn kept() {}\n",
            "const CFG_TEST_ATTR: &str = \"#[cfg(test)]\";\nfn kept() {}\n",
            "//! prose naming #[cfg(test)]\nfn kept() {}\n",
            "/// a doc comment naming #[cfg(test)]\nfn kept() {}\n",
        ] {
            let region = production_code(prose);
            assert!(
                region.contains("fn kept()"),
                "a `#[cfg(test)]` in prose removed the item after it: {prose:?} -> {region:?}"
            );
            assert!(
                !region.contains("#[cfg(test)]"),
                "the attribute survived the blanking: {region:?}"
            );
        }
        let region = production_code(
            "// prose: #[cfg(test)] mod tests;\n#[cfg(test)]\nmod tests;\nfn kept() {}\n",
        );
        assert!(region.contains("fn kept()"), "{region:?}");
        assert!(!region.contains("mod tests;"), "{region:?}");
    }

    struct AboveTheCut {
        truncated: String,
        whole: String,
        gates: usize,
    }

    fn read_above_the_cut(path: &str, source: &str) -> AboveTheCut {
        const CUT: &str = "#[cfg(test)]";
        let region = production_region(source);
        let truncated = blank_comments_and_strings(&region);
        let whole = production_code(source);
        let prefix = whole.get(..truncated.len()).unwrap_or(&whole);
        let with_its_gate =
            production_code(source.get(..region.len() + CUT.len()).unwrap_or(source));
        let above_the_cut = with_its_gate
            .get(..truncated.len())
            .unwrap_or(&with_its_gate);
        assert_eq!(
            prefix.replace(' ', ""),
            above_the_cut.replace(' ', ""),
            "{path}: above the truncating cut, this region reads the whole file differently \
             from the text above the cut read with the gate that makes the cut"
        );
        assert!(
            above_the_cut.len() == truncated.len()
                && above_the_cut
                    .bytes()
                    .zip(truncated.bytes())
                    .all(|(kept, cut)| kept == cut || kept == b' '),
            "{path}: the truncating region keeps code this one does not, other than code this \
             one removes as test-only"
        );
        let mut gates = 0_usize;
        let mut from = 0;
        while let Some((hash, close)) = next_test_only_attribute(&region, &truncated, from) {
            let predicate = attribute_open(&region, truncated.as_bytes(), hash)
                .and_then(|(_, open)| gate_predicate(&region, &truncated, open));
            assert!(
                predicate.as_ref().is_some_and(is_false_wherever_test_is),
                "{path}: above the truncating cut, this region removes the element under `{}` as \
                 test-only, and its predicate holds under some assignment of its names with \
                 `test` false",
                region.get(hash..=close).unwrap_or_default()
            );
            gates += 1;
            from = close + 1;
        }
        AboveTheCut {
            truncated,
            whole,
            gates,
        }
    }

    fn is_false_wherever_test_is(predicate: &Predicate) -> bool {
        fn value<'a>(
            predicate: &'a Predicate,
            assigned: &BTreeMap<&'a str, bool>,
        ) -> Result<bool, &'a str> {
            match predicate {
                Predicate::Test => Ok(false),
                Predicate::Other(name) => assigned.get(name.as_str()).copied().ok_or(name.as_str()),
                Predicate::Not(inner) => value(inner, assigned).map(|held| !held),
                Predicate::All(parts) => {
                    let mut open = None;
                    for part in parts {
                        match value(part, assigned) {
                            Ok(false) => return Ok(false),
                            Ok(true) => {}
                            Err(name) => open = open.or(Some(name)),
                        }
                    }
                    open.map_or(Ok(true), Err)
                }
                Predicate::Any(parts) => {
                    let mut open = None;
                    for part in parts {
                        match value(part, assigned) {
                            Ok(true) => return Ok(true),
                            Ok(false) => {}
                            Err(name) => open = open.or(Some(name)),
                        }
                    }
                    open.map_or(Ok(false), Err)
                }
            }
        }
        fn holds_somewhere<'a>(
            predicate: &'a Predicate,
            assigned: &mut BTreeMap<&'a str, bool>,
        ) -> bool {
            match value(predicate, assigned) {
                Ok(held) => held,
                Err(name) => [true, false].into_iter().any(|each| {
                    assigned.insert(name, each);
                    let held = holds_somewhere(predicate, assigned);
                    assigned.remove(name);
                    held
                }),
            }
        }
        !holds_somewhere(predicate, &mut BTreeMap::new())
    }

    pub(in crate::effects::tests) fn the_whole_region_contains_the_truncated_one() {
        let mut compared = 0_usize;
        let mut strictly_larger = 0_usize;
        let mut gained: BTreeSet<String> = BTreeSet::new();
        for (path, source) in scanned_sources() {
            let AboveTheCut {
                truncated, whole, ..
            } = read_above_the_cut(&path, &source);
            compared += 1;
            if whole.trim().len() > truncated.trim().len() {
                strictly_larger += 1;
                gained.insert(path);
            }
        }
        assert!(compared > 40, "only {compared} files were compared");
        assert!(
            strictly_larger >= 8,
            "only {strictly_larger} files gained anything, so the two regions are the same \
             function and this comparison proves nothing"
        );
        assert!(
            gained.contains("src/engine/coordinator.rs"),
            "the legacy coordinator — 35 of 1599 lines under the truncating region — must be one \
             of the files that gains, or the census that adopted this helper still cannot see it"
        );

        const SAME_PATH: &str =
            "pub(crate) fn same_path(left: &Path, right: &Path) -> bool {\n    left == right\n}\n";
        const TARGETS: &str = "target_os = \"linux\", target_os = \"macos\", \
             target_os = \"windows\", target_os = \"ios\", target_os = \"android\", \
             target_os = \"freebsd\", target_os = \"dragonfly\", target_os = \"openbsd\", \
             target_os = \"netbsd\", target_os = \"solaris\", target_os = \"illumos\", \
             target_os = \"fuchsia\", target_os = \"redox\", target_os = \"haiku\", \
             target_os = \"hermit\", target_os = \"wasi\", target_os = \"emscripten\", \
             target_os = \"vxworks\", target_os = \"espidf\", target_os = \"horizon\", \
             target_os = \"aix\"";
        let many_targets =
            format!("#[cfg(all(test, any({TARGETS})))]\nconst _: () = ();\n#[cfg(test)]\n");
        for (what, above, gates) in [
            ("the gate alone", "#[cfg(test)]\n", 0),
            (
                "an attribute above the gate that makes the cut",
                "#[allow(dead_code)]\n#[cfg(test)]\n",
                0,
            ),
            (
                "attributes and a comment above the gate that makes the cut",
                "#[inline]\n// a note\n#[allow(dead_code)]\n#[cfg(test)]\n",
                0,
            ),
            (
                "a spelling of the gate other than the one that cuts",
                "#[cfg(all(test))]\n",
                1,
            ),
            (
                "an attribute above a compound gate",
                "#[allow(dead_code)]\n#[cfg(all(unix, test))]\n",
                1,
            ),
            ("a gate that no build satisfies", "#[cfg(any())]\n", 1),
            (
                "a gate naming twenty-one configuration names",
                many_targets.as_str(),
                1,
            ),
        ] {
            let source = format!(
                "fn before() {{}}\n{above}{SAME_PATH}fn after() {{}}\n#[cfg(test)]\nmod tests {{}}\n"
            );
            let read = read_above_the_cut(what, &source);
            assert_eq!(read.gates, gates, "{what}: {source:?}");
            assert!(
                !read.whole.contains("same_path") && read.whole.contains("fn after()"),
                "{what}: the element is not what the region removed: {:?}",
                read.whole
            );
        }
        let all_targets = format!("all(test, any({TARGETS}))");
        let any_target = format!("any({TARGETS})");
        for (predicate, entails) in [
            ("test", true),
            ("all(unix, test)", true),
            ("not(not(test))", true),
            ("any()", true),
            ("all(unix, not(unix))", true),
            ("any(unix, test)", false),
            ("not(test)", false),
            ("all()", false),
            ("unix", false),
            ("test = \"x\"", false),
            (all_targets.as_str(), true),
            (any_target.as_str(), false),
        ] {
            let parsed = parse_predicate(predicate).unwrap_or_else(|refusal| panic!("{refusal}"));
            assert_eq!(is_false_wherever_test_is(&parsed), entails, "{predicate}");
        }
        let names: Vec<String> = (0..1000).map(|at| format!("name_{at}")).collect();
        let listed = names.join(", ");
        let negated: Vec<String> = names.iter().map(|name| format!("not({name})")).collect();
        let negated = negated.join(", ");
        for (predicate, entails) in [
            (format!("all(test, any({listed}))"), true),
            (format!("any({listed}, test)"), false),
            (format!("all(any({listed}), not(any({listed})))"), true),
            (format!("all(any({listed}), any({negated}))"), false),
        ] {
            let parsed = parse_predicate(&predicate).unwrap_or_else(|refusal| panic!("{refusal}"));
            assert_eq!(
                is_false_wherever_test_is(&parsed),
                entails,
                "a predicate naming {} configuration names",
                names.len()
            );
        }

        const SENTINEL: &str = "\npub fn sentinel_below_every_configured_item() {}\n";
        let mut carried = 0_usize;
        for (path, source) in scanned_sources() {
            let region = production_code(&format!("{source}{SENTINEL}"));
            assert!(
                region.contains("fn sentinel_below_every_configured_item()"),
                "{path}: an item appended below the whole file is not in its region, so the \
                 region ends somewhere earlier than the file does and everything past that \
                 point is invisible to every census that counts over it"
            );
            carried += 1;
        }
        assert_eq!(
            carried, compared,
            "the sentinel pass and the prefix pass walked different trees"
        );
    }

    pub(in crate::effects::tests) fn every_early_stop_is_at_a_module() {
        fn cut_shape(source: &str) -> Option<String> {
            let blanked = blank_comments_and_strings(source);
            let cut = blanked.find("#[cfg(test)]")?;
            let after = blanked[cut + "#[cfg(test)]".len()..].trim_start();
            Some(
                after
                    .split_whitespace()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        }

        fn is_module(shape: &str) -> bool {
            shape
                .split_whitespace()
                .next()
                .is_some_and(|first| first == "mod" || first.starts_with("pub"))
                && shape.contains("mod ")
        }

        let mut offenders = BTreeMap::new();
        let mut at_a_module = 0usize;
        for (path, source) in scanned_sources() {
            let Some(shape) = cut_shape(&source) else {
                continue;
            };
            if is_module(&shape) {
                at_a_module += 1;
                continue;
            }
            offenders.insert(path, shape);
        }

        let named: BTreeSet<&str> = offenders.keys().map(String::as_str).collect();
        assert_eq!(
            named,
            BTreeSet::from([
                "src/agent/bin.rs",
                "src/agent/claude.rs",
                "src/agent/codex.rs",
                "src/agent/copilot.rs",
                "src/agent/proc.rs",
                "src/engine/attempt.rs",
                "src/engine/coordinator.rs",
                "src/engine/options.rs",
                "src/engine/resume.rs",
                "src/util.rs",
            ]),
            "the set of files whose `effects::production_region` stops at something \
             other than a module moved. Everything below such a cut is invisible to \
             every census that consults that region, silently. Shapes found: \
             {offenders:#?}"
        );

        assert!(is_module("mod tests {"));
        assert!(is_module("mod fake; #[cfg(test)]"));
        assert!(is_module("pub(crate) mod fixtures"));
        assert!(!is_module("use super::X;"));
        assert!(!is_module("pub const ALL:"));
        assert!(!is_module("pub(super) fn resume_harness_inner("));
        assert_eq!(cut_shape("fn a() {}\n").as_deref(), None);
        assert_eq!(
            cut_shape("//! prose naming #[cfg(test)]\nfn a() {}\n").as_deref(),
            None,
            "a `#[cfg(test)]` in a comment classifies, so this census reads prose"
        );

        assert!(
            at_a_module > 20,
            "only {at_a_module} file(s) cut at a module; the scan is not reading the tree"
        );

        let definitions: usize = scanned_sources()
            .iter()
            .map(|(_, source)| {
                blank_comments_and_strings(source)
                    .matches("fn production_region(")
                    .count()
            })
            .sum();
        assert_eq!(
            definitions, 2,
            "this crate no longer has exactly two `production_region` \
             implementations; the divergence table in this test's doc comment \
             describes a tree that no longer exists"
        );
        let shared: usize = scanned_sources()
            .iter()
            .map(|(_, source)| {
                blank_comments_and_strings(source)
                    .matches("fn production_code(")
                    .count()
            })
            .sum();
        assert_eq!(
            shared, 1,
            "`production_code` is the one region every whole-tree prohibition census \
             shares; a second definition is the divergence this table exists to count"
        );
    }
}
