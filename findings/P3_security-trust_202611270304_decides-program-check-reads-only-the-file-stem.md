---
id: PR435-REVIEWER-FILE-STEM-REMARK
severity: P3
disposition: deferred
category: security-trust
pr: 435
reviewed_sha: 5ca9531f56e6eb0483bb43f6fb01e3672a0dab44
location: src/decide.rs:889
provenance: introduced_by_feature
first_bad: introduced with `upstroke decide` on this branch (PR #435); unchanged by the round-two repair, where it is src/decide.rs:1177
guard: the next change to how `upstroke decide` checks the built command (`Built::read` in src/decide.rs), or to how the Claude Code adapter names its program
---

## Failure sequence

Split from `PR435-BUILT-REVIEWER-IDENTITY` in PR #435's round two and filed under ruling R79 at
P3. The identity defect itself, a built command whose `--model` or `--effort` is not the
recorded reviewer's, was repaired: `prepare` now refuses it
(`decide::tests::a_mocked_adapter_whose_command_breaks_a_promise_is_refused_before_anything`,
cases `another model`, `another effort` and `no effort`). This file holds only the remark about
the program check.

    an adapter builds the decide call's CommandSpec
    -> Built::read admits it when Path::new(&command.program).file_stem() is "claude"
    -> the stem is the last component's name without its extension, whatever directory it is in
    -> so `claude`, `/any/directory/claude`, `claude.cmd` and `./claude.sh` are all read as
       Claude Code's CLI
    -> the check identifies the program by its name and says nothing about which binary starts

At 5ca9531f, `src/decide.rs:889`, and unchanged after the repair at `src/decide.rs:1177`:

```rust
        let program = Path::new(&command.program)
            .file_stem()
            .and_then(std::ffi::OsStr::to_str);
        if program != Some("claude") {
            return Err(format!(
                "starts `{}`, not Claude Code's CLI",
                command.program
            ));
        }
```

The built-in Claude Code adapter names its program `claude` (`CLI` in `src/agent/claude.rs`), and
the check's one refusal case starts `sh`
(`a_mocked_adapter_whose_command_breaks_a_promise_is_refused_before_anything`, case
`another program`). No case builds a path or an extension whose stem is `claude`.

The remark is the reviewer's reading; nothing was run, in the review or here.

## What the change that takes this up should do

- State what the program check is for: that the built command names the program the adapter
  names, as the adapter names it, or that it names Claude Code's CLI wherever it is found.
- Check that, rather than the stem: for the first, compare `command.program` with the adapter's
  own program name exactly.
- Make it observable with cases whose program is a path or an extension around `claude`,
  refused, beside the undoctored command, admitted.
