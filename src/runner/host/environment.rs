//! Extended notes: `docs/internals/runner/host/environment.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

use crate::error::UpstrokeError;
use crate::runner::{AgentId, ExecutionRole};
use crate::workspace_manager::NO_REPLACEMENT_OBJECTS;

use super::{RESERVED_ALWAYS, credential_location, reserved_keys, supplies_credentials};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyCase {
    Sensitive,
    Insensitive,
}

impl KeyCase {
    pub const ALL: &'static [Self] = &[Self::Sensitive, Self::Insensitive];

    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Insensitive
        } else {
            Self::Sensitive
        }
    }

    #[must_use]
    pub fn same_key(self, left: &OsStr, right: &OsStr) -> bool {
        match self {
            Self::Sensitive => left == right,
            Self::Insensitive => left
                .to_string_lossy()
                .eq_ignore_ascii_case(&right.to_string_lossy()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ObjectGraph {
    #[default]
    Recorded,
    RecordedIn(ManagedRepository),
    AsReplaced,
}

pub const CONFIG_PARAMETERS: &str = "GIT_CONFIG_PARAMETERS";

pub const RECORDED_OBJECTS_INCLUDE: &[u8] = b"[core]\n\tuseReplaceRefs = false\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitdirRule {
    Posix,
    Windows,
}

impl GitdirRule {
    pub const ALL: &'static [Self] = &[Self::Posix, Self::Windows];

    #[must_use]
    pub const fn native() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Posix
        }
    }

    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Posix | Self::Windows => "gitdir:",
        }
    }

    #[must_use]
    pub fn spelling(self, path: &[u8]) -> Vec<u8> {
        match self {
            Self::Posix => path.to_vec(),
            Self::Windows => {
                let local = match path.strip_prefix(br"\\?\UNC\") {
                    Some(share) => [b"//".as_slice(), share].concat(),
                    None => path.strip_prefix(br"\\?\").unwrap_or(path).to_vec(),
                };
                local
                    .into_iter()
                    .map(|byte| if byte == b'\\' { b'/' } else { byte })
                    .collect()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRepository {
    common_dir: PathBuf,
    include: PathBuf,
    parameters: OsString,
}

impl ManagedRepository {
    pub fn new(common_dir: &Path, include: &Path, rule: GitdirRule) -> Result<Self, UpstrokeError> {
        Self::matching(common_dir, include, rule, folds_case)
    }

    fn matching(
        common_dir: &Path,
        include: &Path,
        rule: GitdirRule,
        mut folds: impl FnMut(&Path, &OsStr) -> Result<bool, String>,
    ) -> Result<Self, UpstrokeError> {
        let refuse = |why: &str| UpstrokeError::Refused {
            message: format!(
                "upstroke cannot keep role processes on the objects the repository at {} \
                 records without changing its configuration: {why}",
                common_dir.display()
            ),
        };
        if !common_dir.is_absolute() || !include.is_absolute() {
            return Err(refuse(&format!(
                "the repository's common directory and the include file {} must both be \
                 absolute paths",
                include.display()
            )));
        }
        let unicode = "a path is not valid Unicode, which Git for Windows cannot read from its \
                       environment";
        let (Some(common), Some(included)) = (path_bytes(common_dir), path_bytes(include)) else {
            return Err(refuse(unicode));
        };
        if rule.spelling(&common).contains(&b'\n') {
            return Err(refuse(
                "its path contains a newline, which Git cannot read in a configuration key",
            ));
        }
        let included = rule.spelling(&included);
        let unmeasured = |why: String| {
            refuse(&format!(
                "it could not look the directories on that path up in another case ({why}), \
                 so it cannot tell where Git must match the path with case and where without"
            ))
        };
        let mut common = Vec::new();
        let mut parent = PathBuf::new();
        for component in common_dir.components() {
            let Some(bytes) = path_bytes(Path::new(component.as_os_str())) else {
                return Err(refuse(unicode));
            };
            match component {
                Component::Prefix(_) => matched(&mut common, &rule.spelling(&bytes), false),
                Component::RootDir => common.push(b'/'),
                Component::Normal(name) => {
                    let folded = folds(&parent, name).map_err(unmeasured)?;
                    if common.last().is_some_and(|last| *last != b'/') {
                        common.push(b'/');
                    }
                    matched(&mut common, &rule.spelling(&bytes), folded);
                }
                Component::CurDir | Component::ParentDir => {
                    return Err(refuse(
                        "its path holds a `.` or `..` component, which the path Git matches \
                         never does",
                    ));
                }
            }
            parent.push(component);
        }
        let mut linked = b"/".to_vec();
        let folded = folds(common_dir, OsStr::new("refs")).map_err(unmeasured)?;
        matched(&mut linked, b"worktrees", folded);
        linked.extend_from_slice(b"/*");
        let mut parameters = Vec::new();
        for suffix in [b"".as_slice(), linked.as_slice()] {
            if !parameters.is_empty() {
                parameters.push(b' ');
            }
            let key = [
                b"includeIf.".as_slice(),
                rule.keyword().as_bytes(),
                &common,
                suffix,
                b".path",
            ]
            .concat();
            parameters.extend(single_quoted(&key));
            parameters.push(b'=');
            parameters.extend(single_quoted(&included));
        }
        let Some(parameters) = os_string(parameters) else {
            return Err(refuse("the include entries are not valid Unicode"));
        };
        Ok(Self {
            common_dir: common_dir.to_path_buf(),
            include: include.to_path_buf(),
            parameters,
        })
    }

    #[must_use]
    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    #[must_use]
    pub fn include(&self) -> &Path {
        &self.include
    }

    #[must_use]
    pub fn parameters(&self) -> &OsStr {
        &self.parameters
    }

    pub fn verify_include(&self) -> Result<(), UpstrokeError> {
        match std::fs::read(&self.include) {
            Ok(bytes) if bytes == RECORDED_OBJECTS_INCLUDE => Ok(()),
            Ok(_) => Err(self.include_refusal("it no longer holds what upstroke wrote there")),
            Err(error) => Err(self.include_refusal(&error.to_string())),
        }
    }

    fn include_refusal(&self, why: &str) -> UpstrokeError {
        UpstrokeError::Refused {
            message: format!(
                "refusing to start a role process: the include file {} that keeps Git \
                 commands in the repository at {} on the objects it records cannot be \
                 used ({why}), and without it they would read what `git replace` \
                 substitutes for them. Resuming the run writes it again",
                self.include.display(),
                self.common_dir.display()
            ),
        }
    }
}

fn folds_case(parent: &Path, name: &OsStr) -> Result<bool, String> {
    match in_the_other_case(name) {
        Some(other) => same_entry(&parent.join(name), &parent.join(other)),
        None => Ok(false),
    }
}

fn in_the_other_case(name: &OsStr) -> Option<OsString> {
    let bytes = path_bytes(Path::new(name))?;
    if !bytes.iter().any(u8::is_ascii_alphabetic) {
        return None;
    }
    os_string(bytes.iter().map(|byte| other_case(*byte)).collect())
}

const fn other_case(byte: u8) -> u8 {
    if byte.is_ascii_lowercase() {
        byte.to_ascii_uppercase()
    } else {
        byte.to_ascii_lowercase()
    }
}

#[cfg(unix)]
fn same_entry(entry: &Path, other: &Path) -> Result<bool, String> {
    use std::os::unix::fs::MetadataExt;
    let failed = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    let found = std::fs::symlink_metadata(entry).map_err(|error| failed(entry, error))?;
    match std::fs::symlink_metadata(other) {
        Ok(other) => Ok(other.dev() == found.dev() && other.ino() == found.ino()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(failed(other, error)),
    }
}

#[cfg(not(unix))]
fn same_entry(entry: &Path, other: &Path) -> Result<bool, String> {
    let failed = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    let found = std::fs::canonicalize(entry).map_err(|error| failed(entry, error))?;
    match std::fs::canonicalize(other) {
        Ok(other) => Ok(other == found),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(failed(other, error)),
    }
}

fn matched(into: &mut Vec<u8>, spelled: &[u8], folded: bool) {
    for byte in spelled {
        if folded && byte.is_ascii_alphabetic() {
            into.extend_from_slice(&[b'[', *byte, other_case(*byte), b']']);
            continue;
        }
        if matches!(byte, b'\\' | b'*' | b'?' | b'[' | b']') {
            into.push(b'\\');
        }
        into.push(*byte);
    }
}

fn single_quoted(text: &[u8]) -> Vec<u8> {
    let mut quoted = vec![b'\''];
    for byte in text {
        if *byte == b'\'' {
            quoted.extend_from_slice(br"'\''");
        } else {
            quoted.push(*byte);
        }
    }
    quoted.push(b'\'');
    quoted
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    Some(path.as_os_str().as_bytes().to_vec())
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Option<Vec<u8>> {
    path.to_str().map(|path| path.as_bytes().to_vec())
}

#[cfg(unix)]
fn os_string(bytes: Vec<u8>) -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt;
    Some(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn os_string(bytes: Vec<u8>) -> Option<OsString> {
    String::from_utf8(bytes).ok().map(OsString::from)
}

#[derive(Debug)]
pub struct HostEnvironment {
    base: Vec<(OsString, OsString)>,
    case: KeyCase,
    objects: ObjectGraph,
}

impl HostEnvironment {
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            base: std::env::vars_os().collect(),
            case: KeyCase::current(),
            objects: ObjectGraph::Recorded,
        }
    }

    #[must_use]
    pub fn with_base(base: Vec<(OsString, OsString)>, case: KeyCase) -> Self {
        Self {
            base,
            case,
            objects: ObjectGraph::Recorded,
        }
    }

    #[must_use]
    pub fn reading(mut self, objects: ObjectGraph) -> Self {
        self.objects = objects;
        self
    }

    #[must_use]
    pub const fn objects(&self) -> &ObjectGraph {
        &self.objects
    }

    #[must_use]
    pub fn base(&self) -> &[(OsString, OsString)] {
        &self.base
    }

    #[must_use]
    pub const fn case(&self) -> KeyCase {
        self.case
    }

    #[must_use]
    pub fn reserved_values(
        &self,
        role: &ExecutionRole,
        agent: Option<&AgentId>,
    ) -> Vec<(&'static str, OsString)> {
        let mut supplied = Vec::new();
        for key in RESERVED_ALWAYS {
            if let Some(value) = self.lookup(key) {
                supplied.push((*key, value));
            }
        }
        if supplies_credentials(role) {
            if let Some(key) = agent.and_then(credential_location) {
                if let Some(value) = self.lookup(key) {
                    supplied.push((key, value));
                }
            }
        }
        supplied
    }

    pub fn compose(
        &self,
        role: &ExecutionRole,
        agent: Option<&AgentId>,
        overlay: &[(String, String)],
    ) -> Result<Vec<(OsString, OsString)>, UpstrokeError> {
        self.preflight(overlay)?;
        let mut composed = self.base.clone();
        for reserved in reserved_keys() {
            composed.retain(|(name, _)| !self.case.same_key(name, OsStr::new(reserved)));
        }
        for (key, value) in self.reserved_values(role, agent) {
            upsert(&mut composed, self.case, OsString::from(key), value);
        }
        for (key, value) in overlay {
            upsert(
                &mut composed,
                self.case,
                OsString::from(key),
                OsString::from(value),
            );
        }
        match &self.objects {
            ObjectGraph::Recorded => {
                let (key, value) = NO_REPLACEMENT_OBJECTS;
                upsert(
                    &mut composed,
                    self.case,
                    OsString::from(key),
                    OsString::from(value),
                );
            }
            ObjectGraph::RecordedIn(repository) => {
                let mut parameters = composed
                    .iter()
                    .find(|(name, _)| self.case.same_key(name, OsStr::new(CONFIG_PARAMETERS)))
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                if !parameters.is_empty() {
                    parameters.push(" ");
                }
                parameters.push(repository.parameters());
                upsert(
                    &mut composed,
                    self.case,
                    OsString::from(CONFIG_PARAMETERS),
                    parameters,
                );
            }
            ObjectGraph::AsReplaced => {}
        }
        Ok(composed)
    }

    pub fn preflight(&self, overlay: &[(String, String)]) -> Result<(), UpstrokeError> {
        for (key, _) in overlay {
            if let Some(reserved) = reserved_keys()
                .into_iter()
                .find(|reserved| self.case.same_key(OsStr::new(key), OsStr::new(reserved)))
            {
                return Err(UpstrokeError::Refused {
                    message: format!(
                        "the command overlay sets `{key}`, which is reserved by the host runner \
                         (`{reserved}`). An adapter may select a profile or change CLI behaviour, \
                         but the runner owns the environment the process executes in \
                         (DESIGN.md:258-264)"
                    ),
                });
            }
        }
        Ok(())
    }

    pub(super) fn lookup(&self, key: &str) -> Option<OsString> {
        self.base
            .iter()
            .find(|(name, _)| self.case.same_key(name, OsStr::new(key)))
            .map(|(_, value)| value.clone())
    }
}

fn upsert(into: &mut Vec<(OsString, OsString)>, case: KeyCase, key: OsString, value: OsString) {
    if let Some(slot) = into.iter_mut().find(|(name, _)| case.same_key(name, &key)) {
        slot.1 = value;
        return;
    }
    into.push((key, value));
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKTREES_FOLDED: &str = "[wW][oO][rR][kK][tT][rR][eE][eE][sS]";

    fn entries(pattern: &str, worktrees: &str, include: &str) -> OsString {
        OsString::from(format!(
            "'includeIf.{pattern}.path'='{include}' \
             'includeIf.{pattern}/{worktrees}/*.path'='{include}'"
        ))
    }

    #[cfg(unix)]
    #[test]
    fn each_component_is_matched_with_case_unless_its_directory_folds_it() {
        let common = Path::new(r"/srv/it's [x]*?\y/.git");
        let include = Path::new("/home/u/.upstroke/git/recorded-objects.gitconfig");
        let exact = r"gitdir:/srv/it'\''s \[x\]\*\?\\y/.git";
        for (folding, pattern, worktrees) in [
            (&[][..], exact, "worktrees"),
            (
                &[r"it's [x]*?\y"][..],
                r"gitdir:/srv/[iI][tT]'\''[sS] \[[xX]\]\*\?\\[yY]/.git",
                "worktrees",
            ),
            (
                &["srv", r"it's [x]*?\y", ".git", "refs"][..],
                r"gitdir:/[sS][rR][vV]/[iI][tT]'\''[sS] \[[xX]\]\*\?\\[yY]/.[gG][iI][tT]",
                WORKTREES_FOLDED,
            ),
            (&["refs"][..], exact, WORKTREES_FOLDED),
            (
                &[".git"][..],
                r"gitdir:/srv/it'\''s \[x\]\*\?\\y/.[gG][iI][tT]",
                "worktrees",
            ),
        ] {
            let mut asked = Vec::new();
            let repository =
                ManagedRepository::matching(common, include, GitdirRule::Posix, |parent, name| {
                    asked.push(parent.join(name));
                    Ok(folding.iter().any(|folds| OsStr::new(folds) == name))
                })
                .expect("a path Git can read in a key");
            assert_eq!(
                repository.parameters(),
                entries(pattern, worktrees, &include.display().to_string()),
                "folding {folding:?}: a component its directory folds carries both cases of \
                 each ASCII letter, every other byte is escaped as it was, the keyword is \
                 `gitdir:` whatever folds, and `worktrees` folds where the common directory \
                 finds `refs` in another case, whatever the component `.git` does"
            );
            assert_eq!(
                asked,
                [
                    "/srv",
                    r"/srv/it's [x]*?\y",
                    r"/srv/it's [x]*?\y/.git",
                    r"/srv/it's [x]*?\y/.git/refs",
                ]
                .map(PathBuf::from),
                "each component is looked up in the directory that holds it, from the root, \
                 and then `refs` in the common directory, which holds `worktrees`"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_windows_rule_matches_its_prefix_as_spelled_and_each_component_by_its_directory() {
        let include = Path::new(r"C:\Users\Me\.upstroke\git\recorded-objects.gitconfig");
        for (common, folding, pattern, worktrees) in [
            (
                r"\\?\C:\Users\Me\it's [x]\.git",
                &[][..],
                r"gitdir:C:/Users/Me/it'\''s \[x\]/.git",
                "worktrees",
            ),
            (
                r"\\?\C:\Users\Me\it's [x]\.git",
                &["Users", "Me"][..],
                r"gitdir:C:/[Uu][sS][eE][rR][sS]/[Mm][eE]/it'\''s \[x\]/.git",
                "worktrees",
            ),
            (
                r"\\?\UNC\server\share\repo\.git",
                &["repo", ".git", "refs"][..],
                r"gitdir://server/share/[rR][eE][pP][oO]/.[gG][iI][tT]",
                WORKTREES_FOLDED,
            ),
        ] {
            let repository = ManagedRepository::matching(
                Path::new(common),
                include,
                GitdirRule::Windows,
                |_, name| Ok(folding.iter().any(|folds| OsStr::new(folds) == name)),
            )
            .expect("a path Git can read in a key");
            assert_eq!(
                repository.parameters(),
                entries(
                    pattern,
                    worktrees,
                    "C:/Users/Me/.upstroke/git/recorded-objects.gitconfig"
                ),
                "{common}, folding {folding:?}"
            );
        }
    }

    #[test]
    fn a_path_whose_directories_cannot_be_looked_up_in_another_case_is_refused() {
        let root = std::env::temp_dir();
        let include = root.join("recorded-objects.gitconfig");
        let refused = ManagedRepository::matching(
            &root.join("repo").join(".git"),
            &include,
            GitdirRule::native(),
            |parent, name| Err(format!("{} is gone", parent.join(name).display())),
        )
        .expect_err("a condition whose case nobody measured is never written")
        .to_string();
        assert!(
            refused.contains("could not look the directories on that path up in another case")
                && refused.contains(" is gone"),
            "{refused}"
        );
        let refused = ManagedRepository::matching(
            &root.join("repo").join(".git"),
            &include,
            GitdirRule::native(),
            |parent, name| {
                if name == "refs" {
                    Err(format!("{} is gone", parent.join(name).display()))
                } else {
                    Ok(false)
                }
            },
        )
        .expect_err("a `worktrees` whose directory nobody measured is never written")
        .to_string();
        assert!(
            refused.contains("could not look the directories on that path up in another case")
                && refused.contains("refs is gone"),
            "{refused}"
        );
        let refused = ManagedRepository::matching(
            &root.join("..").join("repo").join(".git"),
            &include,
            GitdirRule::native(),
            |_, _| Ok(false),
        )
        .expect_err("Git matches a path with no `..` in it")
        .to_string();
        assert!(refused.contains("`..`"), "{refused}");
    }
}
