---
id: GATE-PR-POLICY-BODY-EXCEEDS-MAX-ARG-STRLEN
severity: P2
disposition: deferred
category: security-trust
pr: none
reviewed_sha: 8b28944f9607447448f5d9b9950ee49596073e58
location: .github/workflows/pr-policy.yml:61
provenance: pre_existing   # observed by the orchestrator; reproduced on sourcemaps/upstroke#280, run 34908130306, 2026-09-14T23:17Z
first_bad: unknown
guard: a pr-policy run over a pull request body larger than 131072 bytes in which steps 4 and 5 execute and report their own verdict, failing before the change and passing after
---

## Failure sequence

`upstroke-pr-policy` is a **required** status check. Its step 3, *"Resolve the pull request under
review"*, receives the pull request body as a **single environment variable**. Linux caps one
environment string at `MAX_ARG_STRLEN` — **131072 bytes** (32 pages). A body above that makes the
step's `bash` fail to start:

```
##[error]An error occurred trying to start process '/usr/bin/bash' with working directory
'/home/runner/work/upstroke/upstroke'. Argument list too long
```

## Why this is a trust defect and not merely a size limit

**Steps 4 and 5 are then SKIPPED** — *"Validate title, evidence sections, and finding ledger"* and
*"Validate the head branch name"*. The required check therefore **reports a result without having
judged the pull request at all**, and the result it reports names `bash` rather than any policy rule.

That is `CLAUDE.md`'s first limb almost verbatim: *"were this file written to deceive, could a
required check report success without having done its work?"* Here the check reports **failure**
without having done its work — the same class, and the same loss of meaning in the signal.

The failure is also **reachable from outside any diff**: appending prose to a pull request body, which
changes no tracked file, is enough to stop the gate judging it.

## Reproduction (executed, on #280)

| body size | outcome |
|---:|---|
| **126,496 bytes** | `upstroke-pr-policy` **passes**; steps 4 and 5 run |
| **132,740 bytes** | step 3 fails *"Argument list too long"*; steps 4 and 5 **skipped** |
| **128,751 bytes** | passes again after the body was rewritten smaller |

Run `34908130306`, job `104189381692`, 2026-09-14T23:17:02Z.

## Why it will recur

The bodies that approach the ceiling are exactly the long-running repair pull requests that
accumulate round-by-round evidence, and `MAINTAINING.md` step 7 **requires** appending a merge
delegation record to the body before merging. #280 sat at **98%** of the cap on the day this was
found, so the record that authorises the merge is the very edit that can stop the gate judging it.

## Fix direction

**Pass the body by file rather than by environment variable** — write it to a path under
`$RUNNER_TEMP` and have the step read that path — so body length stops being bounded by
`MAX_ARG_STRLEN`. Failing that, the step must **detect the oversize case and fail as a policy
verdict in its own terms**, rather than dying before its validations run.
