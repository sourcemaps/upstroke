## 4. Invariants

1. **Agents edit files; the engine owns git.** Agents are instructed never to commit. The engine creates branches, stages, commits, and (v0.2) merges.
2. **The engine never speaks HTTP.** All model interaction happens inside agent subprocesses. The capacity tracker (§13) is not the engine: it asks only explicitly selected first-party read-only capacity endpoints, including the owner-admitted undocumented reset reader, through a `curl` subprocess or the official CLI's read-only usage control protocol, and nothing in `run` or `resume` calls it. A run reads the reports the tracker stored, as it reads any file; it never asks for one, and never waits on one. The capacity handler (§13; decided 2026-11-25, not built) is not the engine either: a run will ask it for capacity over a local connection and may wait on its answer, and the run records each grant in its event log: a replay re-reads the grants recorded and asks nothing, and a resume asks for the actions it goes on to take, as any run does.
3. **Ground truth is the diff, not the transcript.** Gates check, reviewers judge, and feedback quotes `git diff` captured by the engine.
4. **Every state transition is an event.** State is derived by replaying `events.jsonl`. Resume = replay + continue.
5. **Official CLIs only.** No ToS-violating proxies, ever — the trust wedge is part of the product.
6. **Questions never stop the runnable frontier.** A question parks exactly the tasks it affects; the run hard-blocks only when nothing remains runnable.
7. **Capacity is estimated conservatively.** Safety margins on every pool, a reserve floor for the user's own interactive use, and rate-limit signals treated as ground truth over any estimate.
