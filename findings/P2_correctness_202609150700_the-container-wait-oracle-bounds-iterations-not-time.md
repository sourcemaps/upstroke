---
id: TESTS-CONTAINER-WAIT-HAS-NO-TIME-BOUND
severity: P2
disposition: deferred
category: correctness
pr: none
reviewed_sha: 00856bbe3a12896c1e01b490c75bacb3ea2b9c1d
location: src/runner/container/tests.rs:3218
provenance: pre_existing
first_bad: unknown
guard: a change to `wait_until_terminated` that bounds the wait in TIME and sleeps between observations, with a witness that both `real_docker_*` callers pass while the machine carries the load of the full lib suite -- the missing outcome is a stated, measured timeout rather than whatever 200 `docker` round-trips happen to cost
---

## Failure sequence

`wait_until_terminated` at `src/runner/container/tests.rs:3218` is the shared oracle that decides a
container has stopped:

```rust
fn wait_until_terminated(docker: &dyn ContainerRuntime, name: &str) -> Liveness {
    for _ in 0..200 {
        let state = docker.observe(name).expect("reachable");
        if state.is_terminated() {
            return state;
        }
        std::thread::yield_now();
    }
    panic!("`{name}` is still running after 200 observations");
}
```

**The loop has no time bound and no sleep.** Its budget is an iteration count, so the effective
timeout is whatever 200 observations happen to cost on that box at that moment.

For the `real_docker_*` callers those observations are not cheap spins. `docker_gate` hands these
tests `DockerCli` (`src/runner/container.rs:1276`), whose `observe` (`:1329`) goes through
`listing` -> `exec` -> `Command::new(DOCKER_PROGRAM).output()` (`:994`) — **a process spawn per
iteration**, tens of milliseconds each.

So the oracle's timeout is **unstated, unmeasured, and load-dependent in both directions at once**:
contention stretches the budget (each `docker inspect` round-trip takes longer) *and* slows the
teardown the budget is racing. **There is no reason those two scale together**, and nothing in the
test says what wait was intended.

-> the container has not reached a terminated state within 200 round-trips -> the helper panics
`"... is still running after 200 observations"` -> the calling test fails, reporting a **count** and
no elapsed time, because the helper never measured any.

## Why this presents as a flake and is not one

The helper is called from **two** sites, `:3357` and `:3419`, which is why **two unrelated tests
panic at the same line**. Two unrelated tests failing at one shared site is consistent with **a
marginal budget**, not with two test bugs.

**Witnesses, all at `tests.rs:3226`:**

| test | when | note |
|---|---|---|
| `real_docker_returns_both_streams_of_a_container_separately` | earlier batch | *"`upstroke-f1-two-streams` is still running after 200 observations"* |
| `real_docker_returns_both_streams_of_a_container_separately` | 2026-09-15, baseline at `19b7e0f2` | head's diff contained **no Rust**; passed **3/3** on immediate re-run |
| `real_docker_kill_on_an_already_exited_container_is_tolerated` | 2026-09-15, two baselines at `4ed2cb79` | head's diff is **one deleted markdown file**; passed **3/3** on immediate re-run |

**Measured asymmetry:** each test passes **3 of 3** when run alone at the same head, and fails when
run inside the full `--lib` suite. The failures therefore track the machine's state, not the tree's.

## What the change that takes this up should do

Bound the wait in **time** and **sleep** between observations, so the budget means the same thing on
an idle box and a loaded one, and so the panic can report how long it actually waited. A helper that
says *"still running after 200 observations"* cannot distinguish "the teardown is slow" from "these
round-trips were cheap today" — and those need opposite responses.

**Note on an earlier misreading, recorded so it is not repeated.** This was first explained as 200
`yield_now()` spins elapsing "in microseconds on an idle core". That is the `FakeRuntime` path, not
this one: for `DockerCli` each iteration is a process spawn. **The defect is the absence of a time
bound, not the speed of a spin** — and the measured evidence points the other way from the
microseconds story, since these tests pass alone and fail under load.
