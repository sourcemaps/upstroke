//! Extended notes: `docs/internals/ulid.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use sha2::{Digest, Sha256};
use std::hash::{BuildHasher, RandomState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

static NONCE: AtomicU64 = AtomicU64::new(0);

pub fn ulid() -> String {
    let now_ms = now_ms();
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    observe_sampled_parts(now_ms, pid, nonce);
    ulid_from_parts(now_ms, pid, nonce)
}

pub fn incarnation_ulid() -> String {
    let now_ms = now_ms();
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let draw = host_draw();
    observe_incarnation_parts(now_ms, pid, nonce, draw);
    incarnation_from_parts(now_ms, pid, nonce, draw)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn host_draw() -> [u8; 16] {
    let state = RandomState::new();
    let first = state.hash_one(0_u8).to_be_bytes();
    let second = state.hash_one(1_u8).to_be_bytes();
    let mut draw = [0_u8; 16];
    for (slot, byte) in draw.iter_mut().zip(first.into_iter().chain(second)) {
        *slot = byte;
    }
    draw
}

fn ulid_from_parts(now_ms: u64, pid: u32, nonce: u64) -> String {
    let mut digest = Sha256::new();
    digest.update(b"upstroke.ulid.v2\0");
    digest.update(now_ms.to_be_bytes());
    digest.update(pid.to_be_bytes());
    digest.update(nonce.to_be_bytes());
    render(now_ms, digest)
}

pub(crate) fn incarnation_from_parts(now_ms: u64, pid: u32, nonce: u64, draw: [u8; 16]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"upstroke.incarnation.v1\0");
    digest.update(now_ms.to_be_bytes());
    digest.update(pid.to_be_bytes());
    digest.update(nonce.to_be_bytes());
    digest.update(draw);
    render(now_ms, digest)
}

fn render(now_ms: u64, digest: Sha256) -> String {
    let random_80 = digest
        .finalize()
        .into_iter()
        .take(10)
        .fold(0_u128, |value, byte| (value << 8) | u128::from(byte));
    let value = (u128::from(now_ms & 0xFFFF_FFFF_FFFF) << 80) | random_80;
    (0..26)
        .rev()
        .map(|i| {
            #[expect(
                clippy::indexing_slicing,
                reason = "the five-bit mask bounds this index into the 32-entry alphabet"
            )]
            let byte = CROCKFORD[((value >> (i * 5)) & 0x1F) as usize];
            char::from(byte)
        })
        .collect()
}

#[cfg(not(test))]
fn observe_sampled_parts(_now_ms: u64, _pid: u32, _nonce: u64) {}

#[cfg(not(test))]
fn observe_incarnation_parts(_now_ms: u64, _pid: u32, _nonce: u64, _draw: [u8; 16]) {}

#[cfg(test)]
mod observation {
    use std::cell::Cell;

    thread_local! {
        pub(super) static SAMPLED_PARTS: Cell<Option<(u64, u32, u64)>> =
            const { Cell::new(None) };
    }

    pub(crate) type IncarnationParts = (u64, u32, u64, [u8; 16]);

    thread_local! {
        pub(super) static SAMPLED_INCARNATION_PARTS: Cell<Option<IncarnationParts>> =
            const { Cell::new(None) };
    }

    pub(super) fn observe_sampled_parts(now_ms: u64, pid: u32, nonce: u64) {
        SAMPLED_PARTS.with(|cell| cell.set(Some((now_ms, pid, nonce))));
    }

    pub(super) fn observe_incarnation_parts(now_ms: u64, pid: u32, nonce: u64, draw: [u8; 16]) {
        SAMPLED_INCARNATION_PARTS.with(|cell| cell.set(Some((now_ms, pid, nonce, draw))));
    }

    pub(crate) fn take_sampled_incarnation_parts() -> Option<IncarnationParts> {
        SAMPLED_INCARNATION_PARTS.with(Cell::take)
    }
}

#[cfg(test)]
use observation::{observe_incarnation_parts, observe_sampled_parts};

#[cfg(test)]
pub(crate) use observation::take_sampled_incarnation_parts;

#[cfg(test)]
mod tests {
    use super::*;

    type Vector = (u64, u32, u64, &'static str);

    const FIRST_MS: u64 = 1_788_084_161_241;

    const HASH_CONSTRUCTION_VECTORS: &[Vector] = &[
        (FIRST_MS, 2_437_999, 0, "01M191Y2PSP8400DBF5QSFFJT3"),
        (FIRST_MS + 5, 2_437_999, 5, "01M191Y2PYZCAHQWFFE0B0ZNEN"),
        (FIRST_MS + 6, 2_438_000, 0, "01M191Y2PZX38SSRHP2N0WH5HD"),
        (FIRST_MS + 11, 2_438_000, 5, "01M191Y2Q4J64Y3T1XQ81VQ07G"),
        (FIRST_MS + 13, 2_438_001, 0, "01M191Y2Q6YQRV0G4FJPPG7G7P"),
        (FIRST_MS + 18, 2_438_001, 5, "01M191Y2QB8FP00XRJD39669NS"),
    ];

    const PARTS_AT_THEIR_BOUNDARIES: &[Vector] = &[
        (0, 0, 0, "0000000000XE2AXP270W5G6HJM"),
        (0xFFFF_FFFF_FFFF, 0, 0, "7ZZZZZZZZZWBB5YA5GXJK04BYR"),
        (1 << 48, 0, 0, "0000000000CKGJF0S4BQ5FRX2J"),
        (FIRST_MS, u32::MAX, 0, "01M191Y2PS9PHQ1G68XZYHJEE2"),
        (FIRST_MS, 1, u64::MAX, "01M191Y2PS5ZV781F18MXR5HQ6"),
        (FIRST_MS, 1, 1 << 47, "01M191Y2PSV05K2R9B9S755N9K"),
    ];

    type IncarnationVector = (u64, u32, u64, [u8; 16], &'static str);

    const INCARNATION_VECTORS: &[IncarnationVector] = &[
        (
            FIRST_MS,
            2_437_999,
            0,
            [0; 16],
            "01M191Y2PS2SW0RSZFXRKP6MMR",
        ),
        (
            FIRST_MS,
            2_437_999,
            0,
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            "01M191Y2PSF0NA3BSWBQEF61G3",
        ),
        (
            FIRST_MS,
            2_437_999,
            0,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            "01M191Y2PSRH29V68WGDSHTXDB",
        ),
        (
            FIRST_MS,
            2_437_999,
            0,
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            "01M191Y2PSVJ54E4XVHF3GNVB4",
        ),
        (
            FIRST_MS,
            2_437_999,
            0,
            [0xFF; 16],
            "01M191Y2PS1KD4EDBNEA6NNJF3",
        ),
        (0, 0, 0, [0; 16], "0000000000SRQWK7GPW7C3MX5C"),
        (
            0xFFFF_FFFF_FFFF,
            4_242,
            7,
            [0xA5; 16],
            "7ZZZZZZZZZ7F6Y7BXT47SR55AD",
        ),
    ];

    fn take_sampled_parts() -> Option<(u64, u32, u64)> {
        observation::SAMPLED_PARTS.with(std::cell::Cell::take)
    }

    #[test]
    fn pid_and_nonce_cannot_cancel_in_the_same_millisecond() {
        for ((pid_a, nonce_a), (pid_b, nonce_b)) in [
            ((100, 32_768), (101, 0)),
            ((100, 65_536), (102, 0)),
            ((100, 32_769), (101, 1)),
        ] {
            assert_ne!(
                ulid_from_parts(FIRST_MS, pid_a, nonce_a),
                ulid_from_parts(FIRST_MS, pid_b, nonce_b),
                "pid/nonce pairs ({pid_a}, {nonce_a}) and ({pid_b}, {nonce_b}) cancel"
            );
        }
    }

    #[test]
    fn ulids_are_26_crockford_chars() {
        let id = ulid();
        assert_eq!(id.len(), 26);
        assert!(id.bytes().all(|b| CROCKFORD.contains(&b)), "got: {id}");
    }

    #[test]
    fn ulids_do_not_collide_casually() {
        const PID: u32 = 4_242;
        let mut seen = std::collections::BTreeSet::new();
        for nonce in 0..200 {
            assert!(
                seen.insert(ulid_from_parts(FIRST_MS, PID, nonce)),
                "collision at nonce {nonce}"
            );
        }
        assert_eq!(seen.len(), 200);
    }

    #[test]
    fn the_public_wrapper_returns_exactly_a_parts_construction() {
        let _ = take_sampled_parts();
        let id = ulid();
        let Some((now_ms, pid, nonce)) = take_sampled_parts() else {
            panic!("`ulid` returned {id} without reaching the observation seam");
        };
        assert_eq!(
            ulid_from_parts(now_ms, pid, nonce),
            id,
            "the id is not what the parts it sampled construct"
        );
        assert_eq!(pid, std::process::id(), "the pid sampled was not this one");

        let _ = ulid();
        let Some((_, _, next_nonce)) = take_sampled_parts() else {
            panic!("the second call did not reach the observation seam");
        };
        assert!(
            next_nonce > nonce,
            "the second call reserved {next_nonce}, which does not follow {nonce}"
        );
    }

    #[test]
    fn reserving_a_nonce_yields_the_previous_value_and_wraps_at_the_top() {
        let counter = AtomicU64::new(41);
        assert_eq!(counter.fetch_add(1, Ordering::Relaxed), 41);
        assert_eq!(counter.load(Ordering::Relaxed), 42);

        let at_top = AtomicU64::new(u64::MAX);
        assert_eq!(at_top.fetch_add(1, Ordering::Relaxed), u64::MAX);
        assert_eq!(at_top.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn parts_construct_the_independently_computed_hash_vectors() {
        for &(now_ms, pid, nonce, expected) in HASH_CONSTRUCTION_VECTORS {
            assert_eq!(
                ulid_from_parts(now_ms, pid, nonce),
                expected,
                "({now_ms}, {pid}, {nonce}) differs from the independent SHA-256 vector"
            );
        }
    }

    #[test]
    fn incarnation_parts_and_draws_construct_the_independently_computed_vectors() {
        for &(now_ms, pid, nonce, draw, expected) in INCARNATION_VECTORS {
            assert_eq!(
                incarnation_from_parts(now_ms, pid, nonce, draw),
                expected,
                "({now_ms}, {pid}, {nonce}, {draw:?}) differs from the independent SHA-256 vector"
            );
        }
    }

    #[test]
    fn equal_clock_pid_and_nonce_with_different_draws_construct_different_ids() {
        let draws: [[u8; 16]; 5] = [
            [0; 16],
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            [0xFF; 16],
        ];
        let ids: std::collections::BTreeSet<String> = draws
            .iter()
            .map(|draw| incarnation_from_parts(FIRST_MS, 2_437_999, 0, *draw))
            .collect();
        assert_eq!(
            ids.len(),
            draws.len(),
            "two incarnations that share clock, pid and nonce share an id unless the draw tells \
             them apart: {ids:?}"
        );
        for id in &ids {
            assert_eq!(id[..10], ulid_from_parts(FIRST_MS, 2_437_999, 0)[..10]);
            assert_ne!(
                *id,
                ulid_from_parts(FIRST_MS, 2_437_999, 0),
                "the incarnation construction is its own domain, never a run id's"
            );
        }
    }

    #[test]
    fn the_incarnation_wrapper_returns_exactly_a_parts_and_draw_construction() {
        let _ = take_sampled_incarnation_parts();
        let id = incarnation_ulid();
        let Some((now_ms, pid, nonce, draw)) = take_sampled_incarnation_parts() else {
            panic!("`incarnation_ulid` returned {id} without reaching the observation seam");
        };
        assert_eq!(
            incarnation_from_parts(now_ms, pid, nonce, draw),
            id,
            "the id is not what the parts and the draw it sampled construct"
        );
        assert_eq!(pid, std::process::id(), "the pid sampled was not this one");
        assert_ne!(draw, [0; 16], "the host draw is a draw, not a constant");

        let second = incarnation_ulid();
        let Some((_, _, next_nonce, next_draw)) = take_sampled_incarnation_parts() else {
            panic!("the second call returned {second} without reaching the observation seam");
        };
        assert!(
            next_nonce > nonce,
            "the second call reserved {next_nonce} after {nonce}"
        );
        assert_ne!(
            next_draw, draw,
            "each incarnation draws afresh from the host: a fresh RandomState steps its key"
        );
        assert_ne!(second, id);
    }

    #[test]
    fn parts_at_their_boundaries_construct_their_recorded_ids() {
        for &(now_ms, pid, nonce, expected) in PARTS_AT_THEIR_BOUNDARIES {
            assert_eq!(
                ulid_from_parts(now_ms, pid, nonce),
                expected,
                "({now_ms}, {pid}, {nonce}) constructs something other than its recorded id"
            );
        }
        let epoch = ulid_from_parts(0, 0, 0);
        let one_field_later = ulid_from_parts(1 << 48, 0, 0);
        assert_eq!(epoch[..10], one_field_later[..10]);
        assert_ne!(epoch[10..], one_field_later[10..]);
    }
}
