//! The slot names, the snapshot names, and the intent record they are written
//! into.
//!
//! `decisions.workspace_candidates.manager` names two of the three namespaces
//! literally -- "detached linked worktrees with durable synced intents
//! (`tasks/k<key>-g<gen>`, `merge/s<seq>`)" -- and
//! `decisions.workspace_candidates.snapshots` requires the third's members to be
//! "never reused across roles or attempts".
//!
//! **Containment is structural; uniqueness is not, and the difference is worth
//! stating exactly.** [`safe_component`] is what makes a [`Slot`] path
//! containment-by-construction — a name that could escape the execution root is
//! refused, whatever produced it. Uniqueness across roles and attempts is a
//! weaker guarantee: [`SnapshotName`]'s constructors encode the role and, for an
//! attempt's snapshots, the task, the generation and the attempt (for an
//! integration's, the sequence) into the name, so a caller that goes through them
//! cannot collide — not even two tasks judged at once at the same generation and
//! attempt — but they are not the only way to obtain one.
//! [`SlotInstance::from_intent_name`] reconstructs a [`Slot::Snapshot`] straight
//! from an on-disk intent filename, so that reclaim never has to trust a path stored
//! inside a record, and a name reconstructed that way carries whatever the
//! filename carried. **So "never reused across roles or attempts" rests on
//! caller discipline rather than on the type**, and this module documents that
//! rather than claiming a guarantee it does not provide.
//!
//! Pure string and path arithmetic. Nothing in this module reads or writes the
//! filesystem; the funnels that act on the paths it names are the parent's.
//!
//! **A slot has one instance per coordinator incarnation** (PR11 follow-up C,
//! `reviews/2026-10-02-pr11-follow-up-c-record.md`, §3.3 as §4.2 repairs it).
//! [`Slot`] stays the logical identity every caller passes, frozen recovery and
//! the merge module included; what the manager creates on disk is an
//! *instance* of it, the slot's component followed by `_` and the creating
//! incarnation's [`InstanceTag`]: `tasks/k<key>-g<gen>_<tag>`,
//! `merge/s<seq>_<tag>`, `snapshots/<name>_<tag>`, and the intent
//! `intents/<namespace>.<component>_<tag>.intent`. No incarnation renders
//! another's names, so a dead coordinator's Git writers, which name their slot
//! by path or by registration name, cannot reach the instance its successor
//! uses. A name with no tag was written before instances existed, and is
//! another incarnation's like any other ([`SlotInstance`]).
//!
//! **[`Slot`]'s five effect-site accessors are deliberately not here.** `row`,
//! `add_site`, `write_intent_site`, `remove_site` and `remove_intent_site` map a
//! slot to the [`EffectSiteId`](crate::topology::effects::EffectSiteId) its
//! funnel runs under, which is effect-site vocabulary rather than naming, and
//! `effects::tests::every_site_the_inventory_declares_has_a_funnel_that_names_
//! it_or_is_recorded_absent` reads `src/workspace_manager.rs` **by path** for
//! exactly those eleven variant literals. They stay in the parent, in a second
//! `impl Slot` block beside the module declaration, so that census keeps
//! measuring what it measured before the split.

// **This child states its own lint level and inherits nothing.** A Rust lint
// level is scoped by the module tree rather than by the file, so an out-of-line
// child of `src/workspace_manager.rs` inherits that file's inner
// `#![allow(clippy::disallowed_methods, disallowed_types, disallowed_macros)]`
// unless it says otherwise -- `PR6-LANEF-004`, and the mistake two W1 pull
// requests then made independently (#100 and #102). Nothing here reaches a
// governed primitive, so all three are DENIED and this module takes no
// `effects/allowlist.toml` row: a row records an allowance, and this module
// takes none.
#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use super::Refusal;

/// The three worktree namespaces of an execution root.
///
/// `decisions.workspace_candidates.manager` names two of them literally —
/// "detached linked worktrees with durable synced intents (`tasks/k<key>-g<gen>`,
/// `merge/s<seq>`)" — and `snapshots` names the third, whose members
/// `decisions.workspace_candidates.snapshots` requires to be "never reused
/// across roles or attempts".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// `tasks/k<key>-g<gen>` — a task worktree, R9.
    Task {
        /// The task key.
        key: String,
        /// The generation number.
        generation: u32,
    },
    /// `merge/s<seq>` — a staging worktree, R10. Never created for an
    /// exact-base fast sequence.
    Staging {
        /// The merge sequence number.
        sequence: u64,
    },
    /// `snapshots/<name>` — an exact gate or review snapshot, R24.
    Snapshot {
        /// The snapshot's name. Built through one of [`SnapshotName`]'s
        /// constructors it encodes the role and the task, generation and attempt
        /// (or the sequence) it judges, so callers going through them cannot
        /// collide; see that type for why that is not a property of every
        /// `SnapshotName`.
        name: SnapshotName,
    },
}

/// A snapshot's name.
///
/// The constructors below encode the role and, for an attempt's snapshots, the
/// task, the generation and the attempt (for an integration's, the sequence) into
/// the string, which is how a caller that uses them satisfies
/// `decisions.workspace_candidates.snapshots`' "never reused across roles or
/// attempts" — and, since attempts of different tasks run at once from PR11, across
/// tasks: two tasks at the same generation and attempt name different slots. It is **not** a property of the type: `SlotInstance::from_intent_name`
/// rebuilds one straight from an on-disk intent filename, so a reconstructed
/// name carries whatever the filename carried. The module doc says where that
/// leaves the guarantee.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotName(String);

impl SnapshotName {
    /// The one snapshot the whole gate set of task `key`'s attempt runs on.
    #[must_use]
    pub fn gates(key: u32, generation: u32, attempt: u32) -> Self {
        Self(format!("k{key}-g{generation}-a{attempt}-gates"))
    }

    /// One fresh snapshot per reviewer of task `key`'s attempt.
    #[must_use]
    pub fn review(key: u32, generation: u32, attempt: u32, reviewer: u32) -> Self {
        Self(format!("k{key}-g{generation}-a{attempt}-review{reviewer}"))
    }

    /// The snapshot an integration transaction's gate set runs on.
    #[must_use]
    pub fn integration(sequence: u64) -> Self {
        Self(format!("s{sequence}-integration"))
    }

    /// One fresh snapshot per integration reviewer.
    #[must_use]
    pub fn integration_review(sequence: u64, reviewer: u32) -> Self {
        Self(format!("s{sequence}-review{reviewer}"))
    }

    /// The name as a directory component.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SnapshotName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Check that `name` is safe as a single path component, or say why it is not.
///
/// The grammar is one non-empty run of ASCII alphanumerics, `-` and `_` that
/// does not start with `-`. Everything containment needs follows from it: a
/// separator, `..`, a drive or UNC prefix and a non-UTF-8 byte cannot occur
/// at all, and `.` is excluded so that [`Slot::intent_name`]'s `.` joins stay
/// unambiguous. The objection is the `why` of [`Refusal::SlotName`], and the
/// verdict is a `Result` so that a caller cannot drop it unread.
///
/// # Errors
///
/// The first objection, as a sentence fragment that completes "refusing the
/// slot name `x`: ...".
pub(super) fn safe_component(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("it is empty");
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("only ASCII alphanumerics, `-`, and `_` are legal in a slot component");
    }
    if name.starts_with('-') {
        return Err("a leading `-` would be read as an option by the Git commands the funnels run");
    }
    Ok(())
}

impl Slot {
    /// The slot's namespace directory and its component within it: the one
    /// rendering that [`Self::relative`], [`Self::id`] and
    /// [`Self::intent_name`] are three spellings of.
    fn parts(&self) -> (&'static str, Cow<'_, str>) {
        match self {
            Self::Task { key, generation } => {
                ("tasks", Cow::Owned(format!("k{key}-g{generation}")))
            }
            Self::Staging { sequence } => ("merge", Cow::Owned(format!("s{sequence}"))),
            Self::Snapshot { name } => ("snapshots", Cow::Borrowed(name.as_str())),
        }
    }

    /// [`Self::parts`] for the instance `tag` names: the component followed
    /// by `_` and the tag, or the bare component for an untagged name.
    ///
    /// The tag's alphabet has no `_`, so [`SlotInstance::from_parts`] splits
    /// an instance at its last `_` whatever the component holds.
    fn instance_parts(&self, tag: Option<&InstanceTag>) -> (&'static str, Cow<'_, str>) {
        let (namespace, component) = self.parts();
        match tag {
            Some(tag) => (
                namespace,
                Cow::Owned(format!("{component}_{}", tag.as_str())),
            ),
            None => (namespace, component),
        }
    }

    /// The slot's path relative to the execution root, with no tag: the
    /// logical slot's own spelling, which no instance the manager creates
    /// has ([`Self::instance_relative`] is the path of one).
    #[must_use]
    pub fn relative(&self) -> PathBuf {
        self.instance_relative(None)
    }

    /// The path, relative to the execution root, of the instance of this slot
    /// that `tag` names, or of the untagged name with `None`.
    #[must_use]
    pub(super) fn instance_relative(&self, tag: Option<&InstanceTag>) -> PathBuf {
        let (namespace, component) = self.instance_parts(tag);
        PathBuf::from(namespace).join(&*component)
    }

    /// The slot's identifier with no tag; [`Self::instance_id`] says what an
    /// identifier is.
    ///
    /// # Errors
    ///
    /// [`Refusal::SlotName`], from [`Self::validate`].
    pub fn id(&self) -> Result<SlotId, Refusal> {
        self.instance_id(None)
    }

    /// The identifier of the instance `tag` names, as the intent record
    /// spells it: a [`SlotId`], the text `<namespace>/<component>[_<tag>]`,
    /// chosen to mirror [`Self::instance_relative`] so that an operator
    /// reading the record can find the directory. It is a name, not a path:
    /// nothing joins it to a root or opens it, and the filesystem path of an
    /// instance comes from `instance_relative()`, which is `PathBuf`
    /// arithmetic over the same parts, never from this text.
    ///
    /// The slot is validated first, which is what lets the result be a
    /// `SlotId` by construction: a `SlotId` is the canonical spelling of an
    /// instance of a slot that passes [`Self::validate`], and [`SlotId`]'s
    /// own parser admits nothing else.
    ///
    /// # Errors
    ///
    /// [`Refusal::SlotName`], from [`Self::validate`].
    pub(super) fn instance_id(&self, tag: Option<&InstanceTag>) -> Result<SlotId, Refusal> {
        self.validate()?;
        Ok(SlotId {
            kind: self.intent_kind(),
            text: self.instance_id_text(tag),
        })
    }

    /// The text of [`Self::instance_id`], before validation: an identifier,
    /// not a path.
    fn instance_id_text(&self, tag: Option<&InstanceTag>) -> String {
        let (namespace, component) = self.instance_parts(tag);
        format!("{namespace}/{component}")
    }

    /// The intent file's name with no tag; [`Self::instance_intent_name`] is
    /// the name of an instance's.
    #[must_use]
    pub fn intent_name(&self) -> String {
        self.instance_intent_name(None)
    }

    /// The name of the intent file of the instance `tag` names, injective over
    /// instances: the two parts are joined by `.`, which [`safe_component`]
    /// forbids inside either, and the tag follows the component's last `_`.
    #[must_use]
    pub(super) fn instance_intent_name(&self, tag: Option<&InstanceTag>) -> String {
        let (namespace, component) = self.instance_parts(tag);
        format!("{namespace}.{component}.intent")
    }

    /// What the intent record calls this kind.
    #[must_use]
    pub fn intent_kind(&self) -> IntentKind {
        match self {
            Self::Task { .. } => IntentKind::Task,
            Self::Staging { .. } => IntentKind::Staging,
            Self::Snapshot { .. } => IntentKind::Snapshot,
        }
    }

    /// [`Self::intent_kind`] as the word the record and the refusals use.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        self.intent_kind().as_str()
    }

    /// Refuse a slot whose components could escape the execution root.
    ///
    /// # Errors
    ///
    /// [`Refusal::SlotName`], naming the kind, the name and
    /// [`safe_component`]'s objection.
    pub(super) fn validate(&self) -> Result<(), Refusal> {
        let name = match self {
            Self::Task { key, .. } => key.as_str(),
            // A sequence number renders as decimal digits, which is a safe
            // component by construction.
            Self::Staging { .. } => return Ok(()),
            Self::Snapshot { name } => name.as_str(),
        };
        safe_component(name).map_err(|why| Refusal::SlotName {
            kind: self.kind(),
            name: name.to_owned(),
            why,
        })
    }

    /// The inverse of [`Self::parts`] on well-formed input: the slot whose
    /// namespace and component these are, or `None` when no slot renders to
    /// them. Reached only through [`SlotInstance::from_parts`], which the
    /// intent-name parser, the namespace walk and [`SlotId`]'s parser share,
    /// so the three spellings cannot drift apart. The `?` sites are
    /// dispositioned on [`SlotInstance::from_intent_name`]. Containment is not
    /// checked here; the callers validate what this returns.
    fn from_parts(namespace: &str, component: &str) -> Option<Self> {
        match namespace {
            "tasks" => {
                let rest = component.strip_prefix('k')?;
                let (key, generation) = rest.rsplit_once("-g")?;
                Some(Self::Task {
                    key: key.to_owned(),
                    generation: generation.parse().ok()?,
                })
            }
            "merge" => {
                let rest = component.strip_prefix('s')?;
                Some(Self::Staging {
                    sequence: rest.parse().ok()?,
                })
            }
            "snapshots" => Some(Self::Snapshot {
                name: SnapshotName(component.to_owned()),
            }),
            _ => None,
        }
    }
}

/// The domain string [`InstanceTag::of_incarnation`] hashes ahead of the
/// incarnation id, so that no other SHA-256 this crate takes of an id can
/// coincide with a tag.
const INSTANCE_TAG_DOMAIN: &[u8] = b"upstroke.slot-instance-tag.v1\0";

/// Crockford's base32 alphabet, as `crate::ulid` spells it: no `I`, `L`, `O`
/// or `U`, and no `_`, which is what lets an instance split at its last `_`.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// The tag of the slot instances one coordinator incarnation creates: twelve
/// Crockford base32 characters rendering the first 60 bits of SHA-256 over
/// [`INSTANCE_TAG_DOMAIN`] and the incarnation id.
///
/// **A function of the id alone**, so every manager of one incarnation renders
/// one instance of a slot, in any process. Three frozen recovery tests depend
/// on that: a child process drives a staging worktree that its parent then
/// asserts at its own manager's path, both derived with the run's recorded
/// incarnation (the record, §4.2, "Why the randomness is in the id").
///
/// **Its uniqueness is the id's, and probabilistic.** The production id
/// carries 128 bits the process draws from the host (`crate::ulid`'s
/// incarnation constructor), so two production incarnations share a tag with
/// probability 2^-60 per pair under SHA-256 as a random function, and their
/// ids are equal with probability at most 2^-80 within one millisecond. A
/// test's fixed ids render fixed, distinct tags.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceTag(String);

impl InstanceTag {
    /// The length of every tag, in characters: 60 bits, five to a character.
    pub const LEN: usize = 12;

    /// The tag of `incarnation`'s instances.
    #[must_use]
    pub fn of_incarnation(incarnation: &str) -> Self {
        let mut digest = Sha256::new();
        digest.update(INSTANCE_TAG_DOMAIN);
        digest.update(incarnation.as_bytes());
        let bits = digest
            .finalize()
            .into_iter()
            .take(8)
            .fold(0_u64, |value, byte| (value << 8) | u64::from(byte))
            >> 4;
        Self(
            (0..Self::LEN)
                .rev()
                .map(|i| {
                    #[expect(
                        clippy::indexing_slicing,
                        reason = "the five-bit mask bounds this index into the 32-entry alphabet"
                    )]
                    let byte = CROCKFORD[((bits >> (i * 5)) & 0x1F) as usize];
                    char::from(byte)
                })
                .collect(),
        )
    }

    /// The tag as its twelve characters.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `text` as a tag: exactly [`Self::LEN`] characters of the alphabet,
    /// upper case, or nothing.
    fn parse(text: &str) -> Option<Self> {
        (text.len() == Self::LEN && text.bytes().all(|byte| CROCKFORD.contains(&byte)))
            .then(|| Self(text.to_owned()))
    }
}

/// One instance of a [`Slot`] on disk: the logical slot, and the tag of the
/// incarnation that created it, or `None` for a name written before instances
/// existed.
///
/// What the parent finds in the intents directory, under a slot namespace or
/// in the registry, read back from a name. Every instance but the current
/// incarnation's own is another incarnation's, and is residue of its slot's
/// row: the manager never uses it, and every reclaim of its slot removes it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotInstance {
    /// The logical slot, as every caller of the manager names it.
    slot: Slot,
    /// The creating incarnation's tag; `None` for an untagged name.
    tag: Option<InstanceTag>,
}

impl SlotInstance {
    /// The logical slot this is an instance of.
    #[must_use]
    pub fn slot(&self) -> &Slot {
        &self.slot
    }

    /// The tag of the incarnation that created it, or `None` for a name
    /// written before instances existed.
    #[must_use]
    pub fn tag(&self) -> Option<&InstanceTag> {
        self.tag.as_ref()
    }

    /// Read an intent file's name back into the instance it records, so that
    /// reclaim never has to trust a path stored inside a record.
    ///
    /// `Some` exactly on [`Slot::instance_intent_name`]'s image, tagged or
    /// not. The name is parsed and then re-rendered, and only a name equal to
    /// its own rendering is an intent name. A name that merely *reads* as one
    /// — `tasks.kalpha-g03.intent` or `merge.s+7.intent`, both of which the
    /// integer parser accepts — is refused, because the slot it would produce
    /// renders to a different file: reclaim would remove that file's intent
    /// and leave this one for every later start to enumerate, report as
    /// reclaimed, and leave again.
    ///
    /// `None` is this parser's whole verdict. Three `?` reach it here, five
    /// more inside [`Slot::from_parts`] through [`Self::from_parts`], which
    /// this, the namespace walk's [`Self::from_entry`] and [`SlotId`]'s parser
    /// share, and the round-trip comparison is the last exit. Each is
    /// dispositioned here in terms of what the verdict means to the one
    /// caller, the intents directory walk:
    ///
    /// - `strip_suffix(".intent")?`: no `.intent` suffix. **Not an intent
    ///   name** at all: a staging `.tmp`, an editor's backup, a stray file.
    /// - `split_once('.')?`: the suffix and no `.` before it, so no namespace.
    ///   **Malformed.**
    /// - `from_parts(..)?`, and inside it: an unknown namespace (**malformed**,
    ///   or another version's); a task or staging component without its `k`
    ///   or `s` prefix, or a task component without the `-g` separator
    ///   (**malformed**); a generation that is not a `u32` or a sequence that
    ///   is not a `u64` (**malformed**; the `ParseIntError` is discarded
    ///   because which way the digits failed adds nothing the file's name,
    ///   which the caller reports, does not already say).
    /// - `then_some`: the name parses but does not re-render to itself.
    ///   **Malformed by canon** rather than by shape; the `g03` case above.
    ///
    /// "Not an intent name" and "malformed intent name" are told apart here
    /// for the reader and folded into one `None` for the caller, deliberately:
    /// the walk has one action for both. It refuses the reclaim and names the
    /// file, because it may delete only what it can prove it owns and may
    /// skip nothing that might be an intent another version wrote. The name
    /// in the refusal carries the distinction to the operator; a second
    /// variant would carry it to a caller with no second action to take. The
    /// parser itself does not know it is reading the intents directory, which
    /// is why the refusal, and its context, are the caller's.
    ///
    /// Grammar here, containment in [`Slot::validate`]: a well-formed name
    /// whose component is not a [`safe_component`] — an empty key, a snapshot
    /// name carrying `.` — is returned, and `validate` refuses it.
    #[must_use]
    pub fn from_intent_name(name: &str) -> Option<Self> {
        let stem = name.strip_suffix(".intent")?;
        let (namespace, instance) = stem.split_once('.')?;
        let parsed = Self::from_parts(namespace, instance)?;
        (parsed.slot.instance_intent_name(parsed.tag()) == name).then_some(parsed)
    }

    /// Read the name of an entry of a slot namespace's directory —
    /// `<namespace>/<name>` under the execution root — back into the instance
    /// it is, or `None` when no instance of that namespace renders to it.
    ///
    /// The namespace walk's reader, the counterpart of
    /// [`Self::from_intent_name`] with the same canon: only a name equal to its
    /// own rendering is an instance. Containment is the caller's, through
    /// [`Slot::validate`].
    #[must_use]
    pub fn from_entry(namespace: &str, name: &str) -> Option<Self> {
        let parsed = Self::from_parts(namespace, name)?;
        (parsed.slot.instance_parts(parsed.tag()).1 == name).then_some(parsed)
    }

    /// The instance whose namespace and component — the slot's component,
    /// then `_` and a tag, or the component alone — these are, or `None` when
    /// no instance of that namespace parses from them. Grammar only: each
    /// caller compares the instance's own rendering with what it read, so
    /// that a name that merely parses is not taken for one the engine wrote.
    ///
    /// **The split.** The tag's alphabet has no `_`, so a tag can only follow
    /// the last `_`, and only exactly [`InstanceTag::LEN`] characters of the
    /// alphabet after it make one. Anything else is a component with no tag:
    /// a name written before instances existed, which is another
    /// incarnation's like any other. A component the engine renders never ends
    /// in `_` and twelve such characters — a task's ends in `-g<generation>`,
    /// a staging's is `s<sequence>`, and every snapshot constructor ends in a
    /// lower-case word or a digit after a `-` — so no untagged name of the
    /// engine's reads as a tagged one.
    fn from_parts(namespace: &str, instance: &str) -> Option<Self> {
        let (component, tag) = match instance.rsplit_once('_') {
            Some((component, tail)) => match InstanceTag::parse(tail) {
                Some(tag) => (component, Some(tag)),
                None => (instance, None),
            },
            None => (instance, None),
        };
        Some(Self {
            slot: Slot::from_parts(namespace, component)?,
            tag,
        })
    }
}

/// The durable per-owner recovery record `resource_accounting` requires of
/// every worktree, staging, and snapshot slot.
///
/// `enforcement_domains.external_physical`: "every worktree, staging, snapshot,
/// and container intent is a durable per-owner recovery record in its row,
/// reclaimed at process start (never 'empty')".
///
/// The worktree path is **not** a field. Reclaim derives it from the intent's
/// own name and the execution root, so a record cannot name a path outside the
/// root it lives in — the containment `cleanup` requires ("expected-path,
/// contained, idempotent, and never establishes authority") is then structural
/// rather than checked.
///
/// **This is a persisted schema**, and `design/15_design_event_log_resume_run_layout.md`
/// states its contract under the run layout ("Synced intents"). The wire
/// format is four JSON strings in the order of [`Self::FIELDS`] — `kind`,
/// `slot`, `run_id`, `incarnation` — with no defaults and no aliases. This
/// type derives `Serialize` and `Deserialize`; what is written by hand is
/// underneath: the reader of the wire shape, [`IntentRecordWire`], accepts a
/// key only by finding it in `FIELDS`, [`IntentKind`] is read only by
/// matching one of its own [`IntentKind::WORDS`], and [`SlotId`] is parsed
/// through the slot grammar. So there is no attribute an alias could ride in
/// on, and a key or word outside the lists is refused. The test on this type
/// pins the exact bytes and compares `FIELDS` to a serialized record's keys.
///
/// Reading a record checks it three ways before one exists: `kind` is an
/// [`IntentKind`]; `slot` is a [`SlotId`], the canonical spelling of a slot
/// passing [`Slot::validate`]; and the two must agree, so a `task` record
/// naming a `merge/` slot is refused. The fields are private and the only
/// other way to hold a record is [`Self::new`], which takes the kind from
/// the slot, so a record cannot be built or changed into a state its reader
/// refuses: what it writes, it reads back. Reclaim still trusts the file's
/// name and nothing inside the file; the typing keeps a record honest for
/// whoever reads it, it grants nothing.
///
/// Written by [`WorkspaceManager::write_intent`] through [`Self::new`].
///
/// [`WorkspaceManager::write_intent`]: super::WorkspaceManager::write_intent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "IntentRecordWire")]
pub struct IntentRecord {
    /// `task`, `staging`, or `snapshot`: always the slot's own kind.
    kind: IntentKind,
    /// The instance's identifier, from [`Slot::instance_id`]:
    /// `<namespace>/<component>_<tag>`, mirroring the instance's relative path
    /// so an operator can find the directory.
    slot: SlotId,
    /// The run that owns it.
    run_id: String,
    /// The coordinator incarnation that wrote it, so a later incarnation of the
    /// same run can tell its own residue from a live sibling's.
    incarnation: String,
}

impl IntentRecord {
    /// The wire field names, in wire order. [`WireField::ALL`] is the same
    /// list as variants, and the reader accepts a key by looking it up here,
    /// so these names are the only keys a record can be read from.
    pub const FIELDS: [&'static str; 4] = ["kind", "slot", "run_id", "incarnation"];

    /// The record for the instance of `slot` that `tag` names, the slot
    /// passing [`Slot::validate`]. Its `slot` field is the instance's
    /// identifier, which mirrors the instance's relative path. The kind is
    /// the slot's own, so a record cannot be built with a kind that
    /// disagrees with its slot; the fields are private, so it cannot be
    /// changed into one afterwards either. This and the reader are the only
    /// two ways to hold a record, and both hold the same invariant.
    ///
    /// # Errors
    ///
    /// [`Refusal::SlotName`], from [`Slot::validate`] through
    /// [`Slot::instance_id`].
    pub fn new(
        slot: &Slot,
        tag: Option<&InstanceTag>,
        run_id: String,
        incarnation: String,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            kind: slot.intent_kind(),
            slot: slot.instance_id(tag)?,
            run_id,
            incarnation,
        })
    }

    /// `task`, `staging`, or `snapshot`: the kind of [`Self::slot`].
    #[must_use]
    pub fn kind(&self) -> IntentKind {
        self.kind
    }

    /// The slot's identifier.
    #[must_use]
    pub fn slot(&self) -> &SlotId {
        &self.slot
    }

    /// The run that owns the slot.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// The coordinator incarnation that wrote the record.
    #[must_use]
    pub fn incarnation(&self) -> &str {
        &self.incarnation
    }
}

/// [`IntentRecord`] as it is read: the same four fields, each typed, before
/// the check that `kind` and `slot` agree.
struct IntentRecordWire {
    kind: IntentKind,
    slot: SlotId,
    run_id: String,
    incarnation: String,
}

/// One of [`IntentRecord::FIELDS`], by position; any other key is unknown.
#[derive(Clone, Copy)]
enum WireField {
    Kind,
    Slot,
    RunId,
    Incarnation,
}

impl WireField {
    /// The variants in the order of [`IntentRecord::FIELDS`]: the two lists
    /// are one table, and a key is accepted only by being found in `FIELDS`.
    const ALL: [WireField; 4] = [
        WireField::Kind,
        WireField::Slot,
        WireField::RunId,
        WireField::Incarnation,
    ];
}

impl<'de> Deserialize<'de> for WireField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FieldVisitor;

        impl Visitor<'_> for FieldVisitor {
            type Value = WireField;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "one of {:?}", IntentRecord::FIELDS)
            }

            fn visit_str<E: de::Error>(self, key: &str) -> Result<WireField, E> {
                IntentRecord::FIELDS
                    .iter()
                    .zip(WireField::ALL)
                    .find(|(name, _)| **name == key)
                    .map(|(_, field)| field)
                    .ok_or_else(|| de::Error::unknown_field(key, &IntentRecord::FIELDS))
            }
        }

        deserializer.deserialize_identifier(FieldVisitor)
    }
}

impl<'de> Deserialize<'de> for IntentRecordWire {
    /// The `?`s here propagate the deserializer's own error, which already
    /// names the key, the value and the position; nothing here could add to
    /// it. Each field is read once, a second occurrence is a duplicate and
    /// a missing one is missing, in serde's words.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct WireVisitor;

        impl<'de> Visitor<'de> for WireVisitor {
            type Value = IntentRecordWire;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an intent record")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut kind = None;
                let mut slot = None;
                let mut run_id = None;
                let mut incarnation = None;
                while let Some(field) = map.next_key::<WireField>()? {
                    match field {
                        WireField::Kind => {
                            if kind.is_some() {
                                return Err(de::Error::duplicate_field("kind"));
                            }
                            kind = Some(map.next_value()?);
                        }
                        WireField::Slot => {
                            if slot.is_some() {
                                return Err(de::Error::duplicate_field("slot"));
                            }
                            slot = Some(map.next_value()?);
                        }
                        WireField::RunId => {
                            if run_id.is_some() {
                                return Err(de::Error::duplicate_field("run_id"));
                            }
                            run_id = Some(map.next_value()?);
                        }
                        WireField::Incarnation => {
                            if incarnation.is_some() {
                                return Err(de::Error::duplicate_field("incarnation"));
                            }
                            incarnation = Some(map.next_value()?);
                        }
                    }
                }
                Ok(IntentRecordWire {
                    kind: kind.ok_or_else(|| de::Error::missing_field("kind"))?,
                    slot: slot.ok_or_else(|| de::Error::missing_field("slot"))?,
                    run_id: run_id.ok_or_else(|| de::Error::missing_field("run_id"))?,
                    incarnation: incarnation
                        .ok_or_else(|| de::Error::missing_field("incarnation"))?,
                })
            }
        }

        deserializer.deserialize_struct("IntentRecord", &IntentRecord::FIELDS, WireVisitor)
    }
}

impl TryFrom<IntentRecordWire> for IntentRecord {
    type Error = IntentRecordError;

    fn try_from(wire: IntentRecordWire) -> Result<Self, Self::Error> {
        if wire.kind != wire.slot.kind() {
            return Err(IntentRecordError::KindDisagreesWithSlot {
                kind: wire.kind,
                slot: wire.slot,
            });
        }
        Ok(Self {
            kind: wire.kind,
            slot: wire.slot,
            run_id: wire.run_id,
            incarnation: wire.incarnation,
        })
    }
}

/// A record whose fields each read but do not agree.
#[derive(Debug, thiserror::Error)]
pub enum IntentRecordError {
    /// `kind` names one kind and `slot` lies in another kind's namespace.
    #[error("the intent record says `{kind}` but its slot `{slot}` is a {}", .slot.kind())]
    KindDisagreesWithSlot {
        /// The kind the record claims.
        kind: IntentKind,
        /// The slot it names.
        slot: SlotId,
    },
}

/// What the intent record calls a slot's kind. The wire spelling is one of
/// [`Self::WORDS`]; the reader is written by hand against that list, so
/// nothing else deserializes and no attribute could widen it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntentKind {
    /// `tasks/k<key>-g<gen>`.
    Task,
    /// `merge/s<seq>`.
    Staging,
    /// `snapshots/<name>`.
    Snapshot,
}

impl IntentKind {
    /// The variants, in wire order.
    pub const ALL: [IntentKind; 3] = [IntentKind::Task, IntentKind::Staging, IntentKind::Snapshot];

    /// The three wire spellings, in variant order: [`Self::as_str`] of each
    /// of [`Self::ALL`], so the words written and the words read are one
    /// list, and a word is read only by matching one of these.
    pub const WORDS: [&'static str; 3] = [
        Self::ALL[0].as_str(),
        Self::ALL[1].as_str(),
        Self::ALL[2].as_str(),
    ];

    /// The word the record and the refusals use.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Staging => "staging",
            Self::Snapshot => "snapshot",
        }
    }
}

impl Serialize for IntentKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for IntentKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KindVisitor;

        impl Visitor<'_> for KindVisitor {
            type Value = IntentKind;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "one of {:?}", IntentKind::WORDS)
            }

            fn visit_str<E: de::Error>(self, word: &str) -> Result<IntentKind, E> {
                IntentKind::ALL
                    .into_iter()
                    .find(|kind| kind.as_str() == word)
                    .ok_or_else(|| de::Error::unknown_variant(word, &IntentKind::WORDS))
            }
        }

        deserializer.deserialize_str(KindVisitor)
    }
}

impl fmt::Display for IntentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An instance's identifier as the intent record spells it: the canonical
/// `<namespace>/<component>[_<tag>]` of an instance of a slot that passes
/// [`Slot::validate`].
///
/// This is a name, not a path. Its grammar mirrors the instance's relative
/// path so that an operator reading a record can find the directory, and the
/// `/` in it is part of the record's wire format on every platform. Nothing
/// joins a `SlotId` to a root or opens it, and no code derives a filesystem
/// path from its text: a path comes from the typed [`Slot`] through
/// [`Slot::instance_relative`], which is `PathBuf` arithmetic. A reader that
/// needs the path parses the text back into an instance and renders that.
///
/// It is produced by [`Slot::instance_id`] from a validated slot's parts and
/// a tag, and it is parsed by [`TryFrom<String>`] on the way out of JSON
/// through the same [`SlotInstance::from_parts`] the intent-name parser uses,
/// then validated, then compared with its own re-rendering. So the text a reader holds is always
/// one a validated slot renders to: not `..`, not a leading `/`, not a
/// backslash, not an empty key, not `merge/s01`. The private fields are the
/// invariant; the only ways to hold one are those two.
///
/// `Serialize` is written by hand so that the text is written straight from
/// the borrow: the derive's `into = "String"` form would clone it first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct SlotId {
    /// The namespace's kind, so the record can check its `kind` against it.
    kind: IntentKind,
    /// The canonical spelling.
    text: String,
}

impl SlotId {
    /// The identifier as the record spells it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The kind of the slot this names, from its namespace.
    #[must_use]
    pub fn kind(&self) -> IntentKind {
        self.kind
    }

    /// The grammar, as a verdict over the text: the instance it names, or
    /// the first objection. Each `?` is a refusal with its reason.
    fn parse(value: &str) -> Result<SlotInstance, &'static str> {
        let (namespace, component) = value
            .split_once('/')
            .ok_or("it has no `/` between the namespace and the component")?;
        if !matches!(namespace, "tasks" | "merge" | "snapshots") {
            return Err("the namespace is not `tasks`, `merge` or `snapshots`");
        }
        safe_component(component)?;
        let instance = SlotInstance::from_parts(namespace, component)
            .ok_or("no slot of that namespace renders that component")?;
        instance.slot.validate().map_err(|refusal| match refusal {
            Refusal::SlotName { why, .. } => why,
            _ => "the slot it names is refused",
        })?;
        if instance.slot.instance_id_text(instance.tag()) != value {
            return Err("it is not the canonical spelling of the slot it names");
        }
        Ok(instance)
    }
}

impl TryFrom<String> for SlotId {
    type Error = SlotIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match Self::parse(&value) {
            Ok(instance) => Ok(Self {
                kind: instance.slot.intent_kind(),
                text: value,
            }),
            Err(why) => Err(SlotIdError { value, why }),
        }
    }
}

impl Serialize for SlotId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text)
    }
}

impl From<SlotId> for String {
    fn from(path: SlotId) -> Self {
        path.text
    }
}

impl fmt::Display for SlotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// A string offered as a [`SlotId`] that is not one.
#[derive(Debug, thiserror::Error)]
#[error("`{value}` is not a slot id: {why}")]
pub struct SlotIdError {
    /// The value as it was offered.
    value: String,
    /// The first objection, from [`SlotId`]'s grammar.
    why: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One slot per shape the grammar has to tell apart: a key that contains,
    /// ends with, or is the `-g` separator's letter; the generation and
    /// sequence extremes; and all three snapshot constructors.
    fn every_shape() -> Vec<Slot> {
        vec![
            Slot::Task {
                key: "alpha".to_owned(),
                generation: 0,
            },
            Slot::Task {
                key: "alpha-g2".to_owned(),
                generation: 3,
            },
            Slot::Task {
                key: "alpha-g".to_owned(),
                generation: 3,
            },
            Slot::Task {
                key: "g".to_owned(),
                generation: u32::MAX,
            },
            Slot::Task {
                key: "0123456789abcdef".to_owned(),
                generation: 1,
            },
            Slot::Staging { sequence: 0 },
            Slot::Staging { sequence: u64::MAX },
            Slot::Snapshot {
                name: SnapshotName::gates(0, 1, 2),
            },
            Slot::Snapshot {
                name: SnapshotName::review(0, 1, 2, 3),
            },
            Slot::Snapshot {
                name: SnapshotName::integration(4),
            },
        ]
    }

    /// The slot an untagged intent name parses to: the shape every name had
    /// before instances existed, and still the grammar's base case.
    fn untagged(name: &str) -> Option<Slot> {
        SlotInstance::from_intent_name(name)
            .filter(|instance| instance.tag.is_none())
            .map(|instance| instance.slot)
    }

    /// The fixed incarnation ids the frozen recovery tests (`CREATOR`,
    /// `RESUMER`, `FIRST_RESUMER`) and the manager's fixture (`inc-1`) derive
    /// managers with, and the tags an implementation independent of this
    /// module computes for them: Python's `hashlib.sha256` over the domain
    /// string and the id, the first 60 bits in Crockford base32
    /// (`~/orch-pr11/logs/pr11_fuc_impl/vectors/vectors.py`).
    const TAG_VECTORS: &[(&str, &str)] = &[
        ("01KZTAAAAAAAAAAAAAAAAAAAAA", "BQ97HKQ6B47T"),
        ("01KZTBBBBBBBBBBBBBBBBBBBBB", "GQXAV07CXT0Z"),
        ("01KZTFFFFFFFFFFFFFFFFFFFFF", "M4A7Z8JN2T5Z"),
        ("inc-1", "WG9T4H5MXCSP"),
        ("", "8MQY56G7H8HD"),
    ];

    #[test]
    fn a_tag_is_twelve_crockford_characters_of_its_incarnations_hash() {
        for (incarnation, expected) in TAG_VECTORS {
            let tag = InstanceTag::of_incarnation(incarnation);
            assert_eq!(
                tag.as_str(),
                *expected,
                "the tag of `{incarnation}` differs from the independent vector"
            );
            assert_eq!(tag.as_str().len(), InstanceTag::LEN);
            assert!(
                tag.as_str().bytes().all(|byte| CROCKFORD.contains(&byte)),
                "{tag:?} is outside the alphabet"
            );
            assert_eq!(InstanceTag::parse(tag.as_str()).as_ref(), Some(&tag));
            assert_eq!(
                tag,
                InstanceTag::of_incarnation(incarnation),
                "a tag is a function of its incarnation id alone, so two managers of one \
                 incarnation render one instance"
            );
        }
        let distinct: std::collections::BTreeSet<InstanceTag> = TAG_VECTORS
            .iter()
            .map(|(incarnation, _)| InstanceTag::of_incarnation(incarnation))
            .collect();
        assert_eq!(
            distinct.len(),
            TAG_VECTORS.len(),
            "the fixed incarnations of the frozen tests and the fixture render distinct tags"
        );
    }

    #[test]
    fn only_twelve_crockford_characters_after_the_last_underscore_make_a_tag() {
        for text in [
            "BQ97HKQ6B47",
            "BQ97HKQ6B47TX",
            "bq97hkq6b47t",
            "BQ97HKQ6B47I",
            "BQ97HKQ6B47L",
            "BQ97HKQ6B47O",
            "BQ97HKQ6B47U",
            "BQ97HKQ6B4_T",
            "BQ97HKQ6B47-",
            "",
        ] {
            assert_eq!(InstanceTag::parse(text), None, "`{text}` was read as a tag");
        }
        assert_eq!(
            SlotInstance::from_intent_name("tasks.kalpha-g1_bq97hkq6b47t.intent"),
            None,
            "a lower-case tail is no tag, and `alpha-g1_bq97hkq6b47t` no task component"
        );
    }

    #[test]
    fn every_instance_shape_survives_the_split_at_its_last_underscore() {
        let resumer = InstanceTag::of_incarnation("01KZTBBBBBBBBBBBBBBBBBBBBB");
        let mut shapes = every_shape();
        shapes.push(Slot::Task {
            key: "my_key".to_owned(),
            generation: 2,
        });
        shapes.push(Slot::Task {
            key: "x_BQ97HKQ6B47T".to_owned(),
            generation: 1,
        });
        for slot in shapes {
            for tag in [None, Some(&resumer)] {
                let expected = SlotInstance {
                    slot: slot.clone(),
                    tag: tag.cloned(),
                };
                let intent = slot.instance_intent_name(tag);
                assert_eq!(
                    SlotInstance::from_intent_name(&intent).as_ref(),
                    Some(&expected),
                    "`{intent}` did not come back as the instance that rendered it"
                );
                let relative = slot.instance_relative(tag);
                let names: Vec<&str> = relative
                    .components()
                    .map(|component| {
                        component
                            .as_os_str()
                            .to_str()
                            .expect("every instance path component is UTF-8")
                    })
                    .collect();
                let [namespace, name] = names.as_slice() else {
                    panic!("an instance's path is its namespace and one name: {names:?}");
                };
                assert_eq!(
                    SlotInstance::from_entry(namespace, name).as_ref(),
                    Some(&expected),
                    "`{namespace}/{name}` did not come back as the instance it names"
                );
                let id = slot
                    .instance_id(tag)
                    .expect("every fixture slot is a valid one");
                assert_eq!(
                    id.as_str(),
                    names.join("/"),
                    "the id mirrors the instance's path"
                );
                assert_eq!(
                    intent,
                    format!("{}.intent", id.as_str().replace('/', ".")),
                    "and the intent name is its third spelling"
                );
                let again = SlotId::try_from(String::from(id.clone()))
                    .expect("what `instance_id` builds, the grammar admits");
                assert_eq!(again, id);
            }
        }
    }

    #[test]
    fn a_record_names_its_instance() {
        let tag = InstanceTag::of_incarnation("inc-1");
        let slot = Slot::Task {
            key: "alpha".to_owned(),
            generation: 1,
        };
        let record = IntentRecord::new(&slot, Some(&tag), "run".to_owned(), "inc-1".to_owned())
            .expect("a valid slot makes a record");
        let json = serde_json::to_string(&record).expect("a record serializes");
        assert_eq!(
            json,
            r#"{"kind":"task","slot":"tasks/kalpha-g1_WG9T4H5MXCSP","run_id":"run","incarnation":"inc-1"}"#
        );
        let back: IntentRecord = serde_json::from_str(&json).expect("the record reads back");
        assert_eq!(back, record);
        for bad in [
            "tasks/kalpha-g1_WG9T4H5MXCS",
            "tasks/kalpha-g01_WG9T4H5MXCSP",
            "tasks/kalpha_WG9T4H5MXCSP",
            "merge/s01_WG9T4H5MXCSP",
        ] {
            assert!(
                SlotId::try_from(bad.to_owned()).is_err(),
                "`{bad}` was accepted as an instance id"
            );
        }
    }

    #[test]
    fn every_slot_shape_survives_the_intent_name_round_trip() {
        for slot in every_shape() {
            slot.validate().expect("every fixture slot is a valid one");
            let name = slot.intent_name();
            assert_eq!(
                untagged(&name).as_ref(),
                Some(&slot),
                "`{name}` did not come back as the slot that rendered it"
            );
            assert_eq!(
                name,
                format!("{}.{}.intent", slot.kind_namespace(), slot.component()),
                "the intent name is the namespace, the component and the suffix"
            );
        }
    }

    #[test]
    fn two_tasks_judged_at_one_generation_and_attempt_name_different_snapshots() {
        let names = [
            SnapshotName::gates(0, 1, 1),
            SnapshotName::gates(1, 1, 1),
            SnapshotName::gates(0, 1, 2),
            SnapshotName::review(0, 1, 1, 0),
            SnapshotName::review(1, 1, 1, 0),
            SnapshotName::review(0, 1, 1, 1),
            SnapshotName::integration(1),
            SnapshotName::integration_review(1, 0),
        ];
        let unique: std::collections::BTreeSet<&str> =
            names.iter().map(SnapshotName::as_str).collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "two snapshots of different tasks, attempts, roles or reviewers share a name: \
             {names:?}"
        );
        for name in &names {
            let slot = Slot::Snapshot { name: name.clone() };
            slot.validate()
                .unwrap_or_else(|error| panic!("`{name}` is not a valid slot component: {error}"));
            assert_eq!(
                untagged(&slot.intent_name()).as_ref(),
                Some(&slot),
                "`{name}` does not survive the intent-name round trip"
            );
        }
        assert_eq!(SnapshotName::gates(7, 2, 3).as_str(), "k7-g2-a3-gates");
        assert_eq!(
            SnapshotName::review(7, 2, 3, 4).as_str(),
            "k7-g2-a3-review4"
        );
    }

    #[test]
    fn a_name_intent_name_did_not_produce_is_not_an_intent_name() {
        for name in [
            // The integer parser accepts these; the round trip does not,
            // because each re-renders to a different file name.
            "tasks.kalpha-g03.intent",
            "tasks.kalpha-g+3.intent",
            "merge.s007.intent",
            "merge.s+7.intent",
            // Malformed bodies under a known namespace.
            "tasks.kalpha.intent",
            "tasks.kalpha-g.intent",
            "tasks.kalpha-gx.intent",
            "tasks.kalpha-g4294967296.intent",
            "tasks.kalpha-g-1.intent",
            "merge.s.intent",
            "merge.s-1.intent",
            "merge.sx.intent",
            // Unknown or misspelled namespaces, and no namespace.
            "snapshot.g1-a1-gates.intent",
            "task.kalpha-g1.intent",
            "intents.x.intent",
            ".intent",
            "",
            // Not the suffix. The first is the name `write_synced` stages an
            // intent under before its rename; the parser refuses it like any
            // other stray file, and what the directory walk should make of
            // that residue is the parent's decision, not this parser's.
            "tasks.kalpha-g1.tmp",
            "tasks.kalpha-g1.intent.bak",
            "tasks.kalpha-g1",
            "tasks.kalpha-g1.intent ",
        ] {
            assert_eq!(
                SlotInstance::from_intent_name(name),
                None,
                "`{name}` was accepted as an intent name"
            );
        }
    }

    #[test]
    fn the_parser_reads_the_grammar_and_validate_reads_containment() {
        // Well-formed under the grammar, refused by containment: the parser
        // returns the slot and `validate` is where it is refused, so the
        // directory walk sees the refusal that names the component.
        for (name, objection) in [
            ("tasks.k-g3.intent", "it is empty"),
            ("snapshots..intent", "it is empty"),
            ("snapshots.a.b.intent", "only ASCII alphanumerics"),
            ("tasks.ka/b-g1.intent", "only ASCII alphanumerics"),
            ("snapshots.-x.intent", "a leading `-`"),
        ] {
            let slot = untagged(name)
                .unwrap_or_else(|| panic!("`{name}` is well-formed under the grammar"));
            assert_eq!(
                slot.intent_name(),
                name,
                "the round trip holds for `{name}`"
            );
            let refusal = slot
                .validate()
                .expect_err("containment refuses what the grammar admitted");
            assert!(
                slot.id().is_err(),
                "and `id` refuses it too, so no record can spell it"
            );
            assert!(
                matches!(&refusal, Refusal::SlotName { why, .. } if why.starts_with(objection)),
                "`{name}` was refused for the wrong reason: {refusal}"
            );
        }
    }

    #[test]
    fn safe_component_names_the_first_objection() {
        for name in ["a", "A-1_b", "0", "a-", "_"] {
            assert_eq!(safe_component(name), Ok(()), "`{name}` is a safe component");
        }
        assert_eq!(safe_component(""), Err("it is empty"));
        for name in ["a/b", "a\\b", "a.b", "..", "a b", "\u{e9}", "a\0"] {
            assert!(
                safe_component(name).is_err_and(|why| why.starts_with("only ASCII")),
                "`{name}` was accepted"
            );
        }
        assert!(
            safe_component("-x").is_err_and(|why| why.starts_with("a leading `-`")),
            "a leading `-` is refused after the character-set check"
        );
        // A staging slot's component is decimal digits by construction and is
        // the one `validate` never checks; the claim is kept honest here.
        for sequence in [0, 1, u64::MAX] {
            let slot = Slot::Staging { sequence };
            assert_eq!(safe_component(&slot.component()), Ok(()));
            slot.validate()
                .expect("a staging slot is valid by construction");
        }
    }

    /// What this pins: the exact bytes, so the field names and their order;
    /// that the four names deserialize; that no field has a default; that a
    /// field under another name is refused, the same value included; and
    /// that an unknown field is refused. Values are pinned by the two typed
    /// fields' own tests below.
    #[test]
    fn the_intent_record_schema_is_pinned() {
        let record = IntentRecord::new(
            &Slot::Task {
                key: "alpha".to_owned(),
                generation: 1,
            },
            None,
            "run".to_owned(),
            "01".to_owned(),
        )
        .expect("a valid slot makes a record");
        let json = serde_json::to_string(&record).expect("a record serializes");
        assert_eq!(
            json, r#"{"kind":"task","slot":"tasks/kalpha-g1","run_id":"run","incarnation":"01"}"#,
            "the persisted field names and order are the on-disk schema; a change here is a \
             compatibility decision, not a refactor"
        );
        let back: IntentRecord = serde_json::from_str(&json).expect("the record reads back");
        assert_eq!(back, record);

        // The field list is derived from the serialized record, not written
        // out again here, so a field added to the type is dropped below like
        // the others. Each field is dropped on its own: `#[serde(default)]`
        // on any one of them would turn that absence into a silently accepted
        // record, and only a per-field drop sees which one.
        let object = || -> serde_json::Map<String, serde_json::Value> {
            serde_json::from_str(&json).expect("the record is a JSON object")
        };
        // `serde_json::Map` sorts its keys; the order is pinned by the exact
        // string above, the set by this. The map is consumed for its keys.
        let fields: Vec<String> = object().into_iter().map(|(key, _)| key).collect();
        assert_eq!(
            fields,
            ["incarnation", "kind", "run_id", "slot"],
            "the field list this test varies is the type's"
        );
        for field in &fields {
            let mut without = object();
            without.remove(field);
            assert!(
                serde_json::from_value::<IntentRecord>(serde_json::Value::Object(without)).is_err(),
                "a record without `{field}` was accepted: no field has a default"
            );
        }
        // The key set from the deserializing side: the four names read back,
        // and a field offered under another name, its value unchanged, is an
        // unknown field plus a missing one and refuses. `#[serde(alias =
        // "legacy_kind")]` on `kind` would accept the first spelling below.
        let read: IntentRecord = serde_json::from_value(serde_json::Value::Object(object()))
            .expect("the four names deserialize");
        assert_eq!(read, record);
        for field in &fields {
            for other in [
                format!("legacy_{field}"),
                format!("{field}_"),
                "x".to_owned(),
            ] {
                let mut renamed = object();
                let value = renamed
                    .remove(field)
                    .expect("the field list came from this object");
                let accepted =
                    format!("`{field}` was accepted as `{other}`: no field has an alias");
                renamed.insert(other, value);
                assert!(
                    serde_json::from_value::<IntentRecord>(serde_json::Value::Object(renamed))
                        .is_err(),
                    "{accepted}"
                );
            }
        }
        let mut extra = object();
        extra.insert(
            "path".to_owned(),
            serde_json::Value::String("/x".to_owned()),
        );
        assert!(
            serde_json::from_value::<IntentRecord>(serde_json::Value::Object(extra)).is_err(),
            "an unknown field is refused: a record must not smuggle a path into reclaim"
        );
    }

    #[test]
    fn the_record_kind_is_one_of_three_words() {
        let with_kind = |kind: &str, slot: &str| {
            serde_json::from_str::<IntentRecord>(&format!(
                r#"{{"kind":{kind},"slot":"{slot}","run_id":"run","incarnation":"01"}}"#
            ))
        };
        for (word, kind, slot) in [
            ("task", IntentKind::Task, "tasks/kalpha-g1"),
            ("staging", IntentKind::Staging, "merge/s1"),
            ("snapshot", IntentKind::Snapshot, "snapshots/g1-a1-gates"),
        ] {
            let record = with_kind(&format!("\"{word}\""), slot).expect("a known kind reads");
            assert_eq!(record.kind(), kind);
            assert_eq!(kind.as_str(), word, "and renders back to the same word");
        }
        for bogus in ["\"bogus\"", "\"Task\"", "\"\"", "0", "null", "[\"task\"]"] {
            assert!(
                with_kind(bogus, "tasks/kalpha-g1").is_err(),
                "kind {bogus} was accepted: the record's kind is one of three words"
            );
        }
        for slot in every_shape() {
            assert_eq!(slot.intent_kind().as_str(), slot.kind());
        }
    }

    #[test]
    fn the_record_slot_is_refused_on_read_outside_its_grammar() {
        let with_slot = |slot: &str| {
            serde_json::from_str::<IntentRecord>(&format!(
                r#"{{"kind":"task","slot":"{slot}","run_id":"run","incarnation":"01"}}"#
            ))
        };
        let record = with_slot("tasks/kalpha-g1").expect("a slot path in the grammar reads");
        assert_eq!(record.slot().as_str(), "tasks/kalpha-g1");
        assert_eq!(record.slot().kind(), IntentKind::Task);
        assert_eq!(record.kind(), record.slot().kind());
        assert_eq!(
            String::from(SlotId::try_from("tasks/kalpha-g1".to_owned()).expect("a slot id")),
            "tasks/kalpha-g1",
            "and converts back to the same text"
        );
        for good in ["merge/s1", "snapshots/g1-a1-gates", "tasks/kalpha-g2-g3"] {
            SlotId::try_from(good.to_owned()).expect("a slot path in the grammar parses");
        }
        // JSON spelling on the left, so a backslash is `\\\\` here and one
        // backslash in the value the record reads.
        for (bad, objection) in [
            ("tasks/..", "only ASCII"),
            ("../tasks/kalpha-g1", "the namespace"),
            ("/tasks/kalpha-g1", "the namespace"),
            ("tasks\\\\kalpha-g1", "it has no `/`"),
            ("tasks/kalpha\\\\g1", "only ASCII"),
            ("tasks/", "it is empty"),
            ("tasks//kalpha-g1", "only ASCII"),
            ("tasks/kalpha-g1/", "only ASCII"),
            ("/", "the namespace"),
            ("", "it has no `/`"),
            ("kalpha-g1", "it has no `/`"),
            ("worktrees/kalpha-g1", "the namespace"),
            ("tasks/-g1", "a leading `-`"),
            // In the grammar of a component, not of any slot.
            ("tasks/kalpha", "no slot of that namespace"),
            ("tasks/alpha-g1", "no slot of that namespace"),
            ("merge/1", "no slot of that namespace"),
            ("merge/sx", "no slot of that namespace"),
            // A slot no validated one can be: the key is empty.
            ("tasks/k-g0", "it is empty"),
            // A slot, but not its canonical spelling.
            ("merge/s01", "not the canonical spelling"),
            ("tasks/kalpha-g01", "not the canonical spelling"),
        ] {
            let error = with_slot(bad).expect_err("a slot path outside the grammar is refused");
            assert!(
                error.to_string().contains(objection),
                "`{bad}` was refused for the wrong reason: {error}"
            );
            let direct = SlotId::try_from(bad.replace("\\\\", "\\"))
                .expect_err("and refused by the constructor itself");
            assert!(direct.to_string().contains(objection), "{direct}");
        }
    }

    #[test]
    fn the_record_refuses_a_kind_that_disagrees_with_its_slot() {
        let with = |kind: &str, slot: &str| {
            serde_json::from_str::<IntentRecord>(&format!(
                r#"{{"kind":"{kind}","slot":"{slot}","run_id":"run","incarnation":"01"}}"#
            ))
        };
        for (kind, slot) in [
            ("task", "tasks/kalpha-g1"),
            ("staging", "merge/s1"),
            ("snapshot", "snapshots/g1-a1-gates"),
        ] {
            let record = with(kind, slot).expect("a kind and a slot of that kind read");
            assert_eq!(record.kind(), record.slot().kind());
        }
        for (kind, slot) in [
            ("task", "merge/s1"),
            ("task", "snapshots/g1-a1-gates"),
            ("staging", "tasks/kalpha-g1"),
            ("snapshot", "merge/s1"),
        ] {
            let error = with(kind, slot).expect_err("a kind and a slot of another kind refuse");
            assert!(
                error.to_string().contains("but its slot"),
                "`{kind}` with `{slot}` was refused for the wrong reason: {error}"
            );
        }
    }

    /// The reader's accepted key set is the literal [`IntentRecord::FIELDS`];
    /// this pins that list to what a record actually writes, in order, so
    /// the two cannot drift, and that a key outside it, a duplicate of one
    /// in it, and a fourth kind word are refused.
    #[test]
    fn the_reader_accepts_exactly_the_fields_a_record_writes() {
        let record = IntentRecord::new(
            &Slot::Snapshot {
                name: SnapshotName::gates(0, 1, 1),
            },
            None,
            "run".to_owned(),
            "01".to_owned(),
        )
        .expect("a valid slot makes a record");
        let json = serde_json::to_string(&record).expect("a record serializes");
        let positions: Vec<usize> = IntentRecord::FIELDS
            .iter()
            .map(|field| {
                json.find(&format!("\"{field}\":"))
                    .unwrap_or_else(|| panic!("`{field}` is written"))
            })
            .collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "the fields are written in FIELDS order: {json}"
        );
        let written: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&json).expect("the record is a JSON object");
        let mut listed: Vec<&str> = IntentRecord::FIELDS.to_vec();
        listed.sort_unstable();
        let mut keys: Vec<&str> = written.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys, listed,
            "the keys a record writes are the keys the reader lists"
        );

        let duplicate = json.replacen("\"run_id\":", "\"run_id\":\"x\",\"run_id\":", 1);
        let error = serde_json::from_str::<IntentRecord>(&duplicate)
            .expect_err("a duplicate key is refused");
        assert!(error.to_string().contains("duplicate field"), "{error}");

        let unknown = json.replacen("\"run_id\":", "\"owner\":\"x\",\"run_id\":", 1);
        let error = serde_json::from_str::<IntentRecord>(&unknown)
            .expect_err("a key outside FIELDS is refused");
        assert!(
            error.to_string().contains("unknown field `owner`"),
            "{error}"
        );

        for (word, kind) in IntentKind::WORDS.iter().zip([
            IntentKind::Task,
            IntentKind::Staging,
            IntentKind::Snapshot,
        ]) {
            assert_eq!(kind.as_str(), *word, "WORDS is in variant order");
        }
        for word in ["worktree", "job", "Task", ""] {
            let error = serde_json::from_str::<IntentKind>(&format!("\"{word}\""))
                .expect_err("a word outside WORDS is refused");
            assert!(
                error.to_string().contains("unknown variant"),
                "{word}: {error}"
            );
        }
        for (word, kind) in IntentKind::WORDS.iter().zip(IntentKind::ALL) {
            let read: IntentKind =
                serde_json::from_str(&format!("\"{word}\"")).expect("each word reads");
            assert_eq!(read, kind, "and reads as the variant that writes it");
        }
        for key in ["old_kind", "Kind", "kind "] {
            let aliased = json.replacen("\"kind\":", &format!("\"{key}\":"), 1);
            let error = serde_json::from_str::<IntentRecord>(&aliased)
                .expect_err("a key outside FIELDS is refused, whatever it is called");
            assert!(
                error.to_string().contains("unknown field"),
                "{key}: {error}"
            );
        }
    }

    /// What a record writes, it reads back, and a record cannot be made to
    /// write what it would refuse: the fields are private, the constructor
    /// takes the kind from the slot, and the reader checks agreement.
    #[test]
    fn a_record_round_trips_and_cannot_be_built_disagreeing() {
        for slot in every_shape() {
            let record = IntentRecord::new(&slot, None, "run".to_owned(), "01".to_owned())
                .expect("every fixture slot makes a record");
            assert_eq!(record.kind(), slot.intent_kind());
            assert_eq!(
                record.slot().as_str(),
                slot.id().expect("a valid slot").as_str()
            );
            assert_eq!(record.run_id(), "run");
            assert_eq!(record.incarnation(), "01");
            let json = serde_json::to_string(&record).expect("serializes");
            let read: IntentRecord = serde_json::from_str(&json).expect("reads back");
            assert_eq!(read, record, "the record read back is the record written");
            assert_eq!(
                serde_json::to_string(&read).expect("serializes again"),
                json,
                "and writes the same bytes again"
            );
        }
        let invalid = Slot::Task {
            key: String::new(),
            generation: 0,
        };
        assert!(
            IntentRecord::new(&invalid, None, "run".to_owned(), "01".to_owned()).is_err(),
            "a slot that validate refuses makes no record"
        );
    }

    #[test]
    fn the_record_slot_id_mirrors_the_relative_path() {
        for slot in every_shape() {
            let id = slot.id().expect("every fixture slot is a valid one");
            let components: Vec<String> = slot
                .relative()
                .components()
                .map(|component| {
                    component
                        .as_os_str()
                        .to_str()
                        .expect(
                            "every slot path component is UTF-8: the fixtures are built from &str",
                        )
                        .to_owned()
                })
                .collect();
            assert_eq!(
                id.as_str(),
                components.join("/"),
                "`id` and `relative` are two spellings of the same parts"
            );
            assert!(
                !id.as_str().contains('\\'),
                "the schema string has no OS separator on any platform: {id}"
            );
            assert_eq!(
                slot.intent_name(),
                format!("{}.intent", id.as_str().replace('/', ".")),
                "and the intent name is the third spelling"
            );
            let again = SlotId::try_from(String::from(id)).expect("round trip");
            assert_eq!(
                again.as_str(),
                components.join("/"),
                "what `id` builds, the grammar admits"
            );
        }
    }

    impl Slot {
        /// The namespace half of the intent name, for the tests' own oracle.
        fn kind_namespace(&self) -> &'static str {
            match self {
                Self::Task { .. } => "tasks",
                Self::Staging { .. } => "merge",
                Self::Snapshot { .. } => "snapshots",
            }
        }

        /// The component half: the last path component of [`Self::relative`].
        fn component(&self) -> String {
            self.relative()
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .expect(
                    "every slot path ends in a UTF-8 component: the fixtures are built from &str",
                )
        }
    }
}
