//! An isolated CLI fixture; the production adapter still constructs and validates every probe.

#![allow(clippy::disallowed_methods)]

use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::runner::{RunnerError, RunnerRequest};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Roster {
    Full,
    Earlier,
    AddedEffortMissing,
    EarlierModelMissing,
    EarlierEffortMissing,
    UnrelatedFailure,
    Malformed,
    Absent,
}

pub(crate) struct ConfiguredCodex {
    directory: PathBuf,
}

impl ConfiguredCodex {
    pub(crate) fn new(directory: &Path) -> Self {
        fs::create_dir_all(directory).expect("isolated Codex fixture");
        fs::write(
            directory.join("cli.sh"),
            r#"printf '%s\n' "$*" >> "$CODEX_FIXTURE/calls"
case "$*" in
  --version) echo 'codex-cli 0.147.0';;
  'exec --help') echo '--json --sandbox --model -c, --config';;
  'exec resume --help') echo '--json --model -c, --config';;
  *upstroke_probe_deliberately_unknown*)
    echo 'unknown key upstroke_probe_deliberately_unknown' >&2; exit 2;;
  *upstroke-output-schema-must-not-exist.json*)
    echo 'output schema upstroke-output-schema-must-not-exist.json does not exist' >&2; exit 2;;
  'debug models')
    if test -f "$CODEX_FIXTURE/failure"; then
      cat "$CODEX_FIXTURE/failure" >&2; exit 2
    fi
    cat "$CODEX_FIXTURE/models.json";;
  'login status') echo 'Logged in using ChatGPT';;
  exec*) cat >/dev/null; cat "$CODEX_FIXTURE/verdict.jsonl";;
  *) echo 'unexpected fixture command' >&2; exit 3;;
esac
"#,
        )
        .expect("fixture CLI");
        let verdict = json!({
            "type": "item.completed",
            "item": {"type": "agent_message", "text":
                "```json\n{\"pass\":true,\"reasons\":[\"accepted\"],\"required_changes\":[]}\n```"}
        });
        fs::write(
            directory.join("verdict.jsonl"),
            format!("{verdict}\n{{\"type\":\"turn.completed\"}}\n"),
        )
        .expect("fixture verdict");
        let stub = Self {
            directory: directory.to_path_buf(),
        };
        stub.roster(Roster::Full);
        stub
    }

    pub(crate) fn roster(&self, roster: Roster) {
        let failure = self.directory.join("failure");
        if failure.exists() {
            fs::remove_file(&failure).expect("clear the scripted failure");
        }
        let absent = self.directory.join("absent");
        if absent.exists() {
            fs::remove_file(&absent).expect("restore the configured CLI");
        }
        match roster {
            Roster::Absent => fs::write(absent, "absent").expect("absent CLI mode"),
            Roster::UnrelatedFailure => fs::write(
                failure,
                "codex roster upgrade: local model catalog does not contain gpt-6.2-sol",
            )
            .expect("stderr that must not classify the failure"),
            _ => {}
        }
        let models: Vec<_> = catalog::known_models(ADAPTER_ID)
            .into_iter()
            .filter(|slug| match roster {
                Roster::Earlier => ["gpt-6-astra", "gpt-5.6-sol"].contains(slug),
                Roster::EarlierModelMissing => *slug != "gpt-5.6-sol",
                _ => true,
            })
            .map(|slug| {
                let levels: Vec<_> = Effort::ALL
                    .into_iter()
                    .filter(|effort| {
                        !(*effort == Effort::Max
                            && match roster {
                                Roster::AddedEffortMissing => slug == "gpt-6.2-astra",
                                Roster::EarlierEffortMissing => slug == "gpt-5.6-sol",
                                _ => false,
                            })
                    })
                    .map(|effort| json!({"effort": effort.to_string()}))
                    .collect();
                json!({"slug": slug, "supported_reasoning_levels": levels})
            })
            .collect();
        let text = if matches!(roster, Roster::Malformed) {
            "not JSON".to_owned()
        } else {
            json!({"models": models}).to_string()
        };
        fs::write(self.directory.join("models.json"), text).expect("configured local roster");
    }

    pub(crate) fn calls(&self) -> String {
        fs::read_to_string(self.directory.join("calls")).expect("CLI calls")
    }

    fn command(&self, mut spec: CommandSpec) -> CommandSpec {
        assert_eq!(spec.program, "codex");
        if self.directory.join("absent").exists() {
            spec.program = self.directory.join("missing-cli").display().to_string();
        } else {
            spec.program = "sh".to_owned();
            spec.args
                .insert(0, self.directory.join("cli.sh").display().to_string());
        }
        spec.env.push((
            "CODEX_FIXTURE".to_owned(),
            self.directory.display().to_string(),
        ));
        spec
    }
}

struct ConfiguredRunner<'a> {
    cli: &'a ConfiguredCodex,
    runner: &'a dyn Runner,
}

impl crate::runner::contract::tests::InlineRunner for ConfiguredRunner<'_> {
    fn run_inline(&self, request: &RunnerRequest) -> Result<ProcessOutput, RunnerError> {
        // The request snapshot retains the real invocation identity and execution boundary.
        let mut configured = request.clone();
        configured.command = self.cli.command(configured.command);
        self.runner.run_blocking(&configured)
    }
}

impl AgentAdapter for ConfiguredCodex {
    fn id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {
        CodexAdapter.probe(&ConfiguredRunner { cli: self, runner })
    }

    fn discover(&self, runner: &dyn Runner, caps: &Caps) -> Result<Discovery, UpstrokeError> {
        CodexAdapter.discover(&ConfiguredRunner { cli: self, runner }, caps)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        CodexAdapter.build(run).map(|spec| self.command(spec))
    }

    fn parse(&self, output: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        CodexAdapter.parse(output)
    }
}

impl AdapterSource for ConfiguredCodex {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == ADAPTER_ID).then_some(self)
    }
}
