//! Extended notes: `docs/internals/runner/host/environment.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

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
    PosixFoldingCase,
    Windows,
}

impl GitdirRule {
    pub const ALL: &'static [Self] = &[Self::Posix, Self::PosixFoldingCase, Self::Windows];

    #[must_use]
    pub const fn native() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::PosixFoldingCase
        } else {
            Self::Posix
        }
    }

    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Posix => "gitdir:",
            Self::PosixFoldingCase | Self::Windows => "gitdir/i:",
        }
    }

    #[must_use]
    pub fn spelling(self, path: &[u8]) -> Vec<u8> {
        match self {
            Self::Posix | Self::PosixFoldingCase => path.to_vec(),
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
        let (Some(common), Some(included)) = (path_bytes(common_dir), path_bytes(include)) else {
            return Err(refuse(
                "a path is not valid Unicode, which Git for Windows cannot read from its \
                 environment",
            ));
        };
        let common = glob_escaped(&rule.spelling(&common));
        let included = rule.spelling(&included);
        if common.contains(&b'\n') {
            return Err(refuse(
                "its path contains a newline, which Git cannot read in a configuration key",
            ));
        }
        let mut parameters = Vec::new();
        for suffix in [b"".as_slice(), b"/worktrees/*".as_slice()] {
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

fn glob_escaped(path: &[u8]) -> Vec<u8> {
    let mut escaped = Vec::with_capacity(path.len());
    for byte in path {
        if matches!(byte, b'\\' | b'*' | b'?' | b'[' | b']') {
            escaped.push(b'\\');
        }
        escaped.push(*byte);
    }
    escaped
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
