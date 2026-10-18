#!/usr/bin/env python3
"""Run one shard of the test suite, or verify that a lane's shards ran all of it.

    python3 .github/scripts/partition.py run <group> <shard> <record-dir>
    python3 .github/scripts/partition.py verify <records-dir>

Both run from the repository root and read the predicate table beside this
file, `.github/scripts/partition.table`. Python 3.11 or later (`tomllib`).

`run` executes on a partitioned test leg of .github/workflows/ci.yml:

  1. `cargo metadata --format-version 1 --no-deps` declares the package's
     targets, and `cargo test --all-targets --all-features --no-run
     --message-format=json-render-diagnostics` builds them and reports each
     compiled unit as a JSON artifact. The test units -- every unit built
     with `--test` that has an executable -- are the leg's harnesses. Their
     inventory comes from Cargo's own records of the build, never from what a
     later command happened to print.
  2. For each test unit, by Cargo's own target selector (`--lib`, `--bin
     <name>`, `--example <name>`, `--test <name>`), it runs `cargo test
     --all-features <selector>` four times: list every test, list the ignored
     ones, list what this shard's libtest filters select, and run that
     selection with `--logfile`, so the harness itself writes one line per
     test it ran or ignored -- a file the nested child processes a test may
     start are never given. Only the shard's filters and `--logfile` are added
     after `--`; Cargo still sets the working directory and environment, and
     nothing is added to the environment this script inherited.
  3. It records every command's argv, exit status and output (stdout and
     stderr of the listing and run commands share one OS handle, so Cargo's
     `Running` line heads its harness's output), the build's JSON, the
     metadata, and each test executable's sha256, size and modification time
     observed after the build and before and after every command that runs
     it. Beside them: the checkout, the table, script and manifest it ran (as
     Git blobs), the run and attempt, the runner, the compiler, and the leg's
     working and record directories. It reads exactly the `GITHUB_*` and
     `RUNNER_*` variables named below and enumerates no environment.
  4. The compiler is Cargo's own answer, asked before the metadata and build
     steps (it compiles the library's dependencies first, which the build
     step would compile next anyway):
     `cargo rustc -q --lib -- --version --verbose` hands rustc's
     `--version --verbose` to the compiler Cargo selects (`RUSTC`,
     `build.rustc` or the toolchain's), through whatever compiler wrappers
     Cargo applies, and the leg is refused unless the answer opens with a
     `rustc` line and names exactly one host and one release. The two flags
     are spelled out, as the Windows legs' own compiler check spells
     `--version`, rather than combined as `-vV`, which a compiler wrapper may
     read as part of a compilation. The answer is the selected compiler's
     report of itself, not an observation of the executables the build ran,
     of a wrapper or of a cache; `rustc -vV` (PATH's rustc) and `cargo -V`
     are recorded beside it as second questions, not substitutes.

`verify` executes in the `test-coverage` job, on this attempt's uploaded
records. It refuses -- exit 1, every reason printed -- unless, for every group
the event's lane partitions:

  * each of the group's shards has exactly one record, from this run and this
    attempt (a record from any other attempt is refused), made at this
    checkout with this table, script and manifest, on the group's platform
    and compiler, holding exactly the files its commands imply;
  * each record's test units are exactly those the record's Cargo metadata
    declares for `--all-targets` -- every lib, bin, example and integration
    test target -- matched one for one with the build's own JSON test
    artifacts; a manifest `harness = false`, an integration test with
    `test = false`, a bench target or another target kind is refused rather
    than guessed at; every other unit of the package the build JSON reports
    is recorded as compile-only, as observed there, and every build script the
    metadata declares must be among those observed units;
  * each record's commands are exactly the ones those units and the table
    imply, each exited 0 with its log at the digest the leg recorded; each
    command ran exactly one harness, the unit's own source and executable;
    and each executable's identity did not change from the build to the last
    command;
  * each unit's harness selected exactly what the table's predicate selects
    from the names it listed; its run opened with its own `running N tests`
    for that selection; its logfile holds exactly one line per selected name
    -- `ok` for a test, `ignored` for an ignored one, nothing failed -- and
    its stdout carries the matching summary;
  * every leg of a group declares the same units, compiles the same
    compile-only units, lists the same names and ignores the same ones, from
    one compiler; the group's selections are disjoint and together hold every
    listed name of every unit; and the legs together report at least the
    table's floor of passing tests.
"""

import hashlib
import json
import ntpath
import os
import posixpath
import re
import stat
import subprocess
import sys
import threading
import time

TABLE_PATH = '.github/scripts/partition.table'
SCRIPT_PATH = '.github/scripts/partition.py'
MANIFEST_PATH = 'Cargo.toml'
EVENTS = ('merge_group', 'pull_request', 'push')
NAME = re.compile(r'[a-z][a-z0-9]*')
FILTER = re.compile(r'[A-Za-z0-9_][A-Za-z0-9_:]*')
METADATA = ['cargo', 'metadata', '--format-version', '1', '--no-deps']
BUILD = ['cargo', 'test', '--all-targets', '--all-features', '--no-run', '--message-format=json-render-diagnostics']
UNIT_TEST = ['cargo', 'test', '--all-features']
COMPILER = ['cargo', 'rustc', '-q', '--lib', '--', '--version', '--verbose']
STEPS = ('list', 'ignored', 'selected', 'run')
RECORD = 'record.json'
FIXED_FILES = ('build.json', 'build.log', 'metadata.json', 'metadata.log', RECORD)
ARTIFACT = re.compile(r'test-partition-([a-z][a-z0-9]*)-attempt-([1-9][0-9]{0,5})')
FORMAT = 2
TEST_KINDS = ('lib', 'bin', 'example', 'test')
GITHUB_KEYS = (
    'GITHUB_EVENT_NAME',
    'GITHUB_JOB',
    'GITHUB_REF',
    'GITHUB_REPOSITORY',
    'GITHUB_RUN_ATTEMPT',
    'GITHUB_RUN_ID',
    'GITHUB_SHA',
    'GITHUB_WORKFLOW',
)
RUNNER_KEYS = (
    'ImageOS',
    'ImageVersion',
    'RUNNER_ARCH',
    'RUNNER_ENVIRONMENT',
    'RUNNER_NAME',
    'RUNNER_OS',
)
ANSI = re.compile(rb'\x1b\[[0-9;]*[A-Za-z]')
RUNNING = re.compile(r'\s*Running (?:unittests )?(\S+) \((.+)\)')
LISTED = re.compile(r'(\S+): test')
COUNTED = re.compile(r'([0-9]+) tests?, ([0-9]+) benchmarks?')
HEADER = re.compile(r'running ([0-9]+) tests?')
# libtest writes this to stderr, with no newline, when it parses `--logfile`, before its own stdout opens with
# `\nrunning N tests`: in the run's combined output it is the line before the header, or the header's prefix.
DEPRECATION = 'warning: `--logfile` is deprecated'
SUMMARY = re.compile(
    r'test result: (ok|FAILED)\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored; '
    r'([0-9]+) measured; ([0-9]+) filtered out; finished in [0-9.]+s'
)


class Refused(Exception):
    pass


class Table(object):
    def __init__(self):
        self.groups = {}
        self.lanes = {}
        self.shards = {}
        self.order = {}


def predicate(args, where):
    positives = []
    skips = []
    index = 0
    while index < len(args):
        word = args[index]
        if word == '--skip' and index + 1 < len(args) and FILTER.fullmatch(args[index + 1]):
            skips.append(args[index + 1])
            index += 2
        elif FILTER.fullmatch(word):
            positives.append(word)
            index += 1
        else:
            raise Refused('%s: %r is neither a name filter nor `--skip <filter>`' % (where, word))
    return positives, skips


def selects(positives, skips, name):
    if positives and not any(positive in name for positive in positives):
        return False
    return not any(skip in name for skip in skips)


def load_table():
    if not os.path.isfile(TABLE_PATH) or not os.path.samefile(
        TABLE_PATH, os.path.join(os.path.dirname(os.path.abspath(__file__)), 'partition.table')
    ):
        raise Refused('run this from the repository root, as %s' % SCRIPT_PATH)
    with open(TABLE_PATH, 'rb') as handle:
        data = handle.read()
    try:
        text = data.decode('ascii')
    except UnicodeDecodeError:
        raise Refused('%s is not ASCII' % TABLE_PATH)
    table = Table()
    for number, raw in enumerate(text.split('\n'), 1):
        words = raw.rstrip('\r').split()
        if not words or words[0].startswith('#'):
            continue
        where = '%s:%d' % (TABLE_PATH, number)
        kind = words[0]
        if kind == 'group' and len(words) == 6:
            group, runner_os, triple, release, floor = words[1:]
            if group in table.groups or not NAME.fullmatch(group) or not floor.isdigit():
                raise Refused('%s: a malformed or repeated group' % where)
            table.groups[group] = {'os': runner_os, 'triple': triple, 'release': release, 'floor': int(floor)}
            table.order[group] = []
        elif kind == 'lane' and len(words) >= 2:
            event = words[1]
            groups = words[2:]
            if event not in EVENTS or event in table.lanes or len(set(groups)) != len(groups):
                raise Refused('%s: a malformed or repeated lane' % where)
            table.lanes[event] = groups
        elif kind == 'shard' and len(words) >= 4:
            group, shard, args = words[1], words[2], words[3:]
            if group not in table.groups:
                raise Refused('%s: shard %r names group %r, which no earlier line declares' % (where, shard, group))
            if shard in table.shards or shard == 'all' or not NAME.fullmatch(shard):
                raise Refused('%s: a malformed or repeated shard %r' % (where, shard))
            predicate(args, where)
            table.shards[shard] = (group, args)
            table.order[group].append(shard)
        else:
            raise Refused('%s: an unreadable line: %r' % (where, raw))
    for event in EVENTS:
        if event not in table.lanes:
            raise Refused('%s has no lane for %s' % (TABLE_PATH, event))
        for group in table.lanes[event]:
            if group not in table.groups or not table.order[group]:
                raise Refused('%s: lane %s names %r, which is no group with shards' % (TABLE_PATH, event, group))
    return table


def git(*args):
    result = subprocess.run(['git'] + list(args), stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        raise Refused('git %s exited %d: %s' % (' '.join(args), result.returncode, result.stderr.decode('utf-8', 'replace').strip()))
    return result.stdout.decode('utf-8', 'replace').strip()


def bound_blobs():
    blobs = {}
    for path in (TABLE_PATH, SCRIPT_PATH, MANIFEST_PATH):
        committed = git('rev-parse', '--verify', '--quiet', 'HEAD:' + path)
        working = git('hash-object', '--', path)
        if committed != working:
            raise Refused('%s is not the checkout\'s: blob %s in the tree, %s at HEAD' % (path, working, committed))
        blobs[path] = committed
    return blobs


def manifest_shapes(path):
    """Refuse the target shapes this partition does not read: a custom harness, or a target kind or flag that
    makes `--all-targets` mean something the inventory below does not model."""
    try:
        import tomllib
    except ImportError:
        raise Refused('Python %s has no tomllib, so %s cannot be read for harness shapes' % (sys.version.split()[0], path))
    with open(path, 'rb') as handle:
        try:
            manifest = tomllib.load(handle)
        except tomllib.TOMLDecodeError as error:
            raise Refused('%s does not parse: %s' % (path, error))
    if 'workspace' in manifest:
        raise Refused('%s declares a workspace, which this partition does not model' % path)
    declared = []
    if isinstance(manifest.get('lib'), dict):
        declared.append(('lib', manifest['lib']))
    for section in ('bin', 'example', 'test', 'bench'):
        for entry in manifest.get(section, []) or []:
            declared.append((section, entry if isinstance(entry, dict) else {}))
    for section, entry in declared:
        if entry.get('harness') is False:
            raise Refused('%s: a %s target sets harness = false, a custom harness this partition cannot read' % (path, section))
        if section == 'bench':
            raise Refused('%s: declares a bench target, which this partition does not model' % path)


def flavour(group_os):
    return ntpath if group_os == 'Windows' else posixpath


def is_absolute(path):
    return path.startswith('/') or path.startswith('\\') or bool(re.match(r'[A-Za-z]:', path))


def normal(paths, path, base=None):
    if base is not None and not is_absolute(path):
        path = paths.join(base, path)
    return paths.normcase(paths.normpath(path))


def relative(paths, path, root):
    full = normal(paths, path)
    top = normal(paths, root)
    if not full.startswith(top + paths.sep):
        raise Refused('%s is not under the package root %s' % (path, root))
    return full[len(top) + 1:].replace('\\', '/')


def inventory(metadata_text, build_text, cwd, group_os, where):
    """The leg's test units and compile-only units, from Cargo's metadata and the build's JSON artifacts alone.

    Two claims, kept apart. The test units are the declared lib, bin, example and integration-test targets, each
    matched one for one with a `profile.test` artifact. The compile-only units are exactly the package's other
    artifacts the build JSON reports, each observed there; a build script the metadata declares is not entered on
    the metadata's word, it must be one of those observed artifacts."""
    paths = flavour(group_os)
    manifest = normal(paths, 'Cargo.toml', cwd)
    try:
        metadata = json.loads(metadata_text)
    except ValueError as error:
        raise Refused('%s: the metadata is not JSON: %s' % (where, error))
    packages = [p for p in metadata.get('packages', []) if isinstance(p, dict) and normal(paths, str(p.get('manifest_path'))) == manifest]
    if len(packages) != 1:
        raise Refused('%s: the metadata declares %d packages at %s, not one' % (where, len(packages), manifest))
    root = paths.dirname(normal(paths, packages[0]['manifest_path']))
    expected = {}
    declared_builds = []
    compile_only = []
    for target in packages[0].get('targets', []):
        kinds = target.get('kind')
        if kinds == ['custom-build']:
            declared_builds.append(('custom-build', str(target.get('name'))))
            continue
        kind = 'lib' if kinds in (['lib'], ['rlib']) else (kinds[0] if isinstance(kinds, list) and len(kinds) == 1 else None)
        if kind not in TEST_KINDS:
            raise Refused('%s: target %r is of kind %r, which this partition does not model' % (where, target.get('name'), kinds))
        if kind == 'test' and target.get('test') is not True:
            raise Refused('%s: integration test %r sets test = false, which `--all-targets` would leave untested' % (where, target.get('name')))
        key = (kind, target.get('name'))
        if key in expected:
            raise Refused('%s: two %s targets named %r' % (where, kind, target.get('name')))
        expected[key] = relative(paths, str(target.get('src_path')), root)
    units = {}
    finished = None
    for line in build_text.split('\n'):
        line = line.strip()
        if not line.startswith('{'):
            continue
        try:
            message = json.loads(line)
        except ValueError as error:
            raise Refused('%s: a build JSON line does not parse: %s' % (where, error))
        if message.get('reason') == 'build-finished':
            finished = message.get('success')
            continue
        if message.get('reason') != 'compiler-artifact' or normal(paths, str(message.get('manifest_path'))) != manifest:
            continue
        target = message.get('target') or {}
        kinds = target.get('kind')
        kind = 'lib' if kinds in (['lib'], ['rlib']) else (kinds[0] if isinstance(kinds, list) and len(kinds) == 1 else str(kinds))
        if (message.get('profile') or {}).get('test') is True:
            key = (kind, target.get('name'))
            if key in units or not message.get('executable'):
                raise Refused('%s: the build reports test unit %r twice or without an executable' % (where, key))
            units[key] = {'kind': kind, 'name': target.get('name'), 'source': relative(paths, str(target.get('src_path')), root),
                          'executable': message['executable']}
        else:
            compile_only.append((str(kind), str(target.get('name'))))
    if finished is not True:
        raise Refused('%s: the build JSON has no successful `build-finished` message' % where)
    unbuilt = sorted(set(declared_builds) - set(compile_only))
    if unbuilt:
        raise Refused('%s: the metadata declares build scripts %r that the build JSON never reports' % (where, unbuilt))
    if sorted(units) != sorted(expected):
        raise Refused('%s: the build compiled test units %r for the declared targets %r'
                      % (where, sorted(units), sorted(expected)))
    for key, source in expected.items():
        if units[key]['source'] != source:
            raise Refused('%s: test unit %r built %r, not the declared %r' % (where, key, units[key]['source'], source))
    ordered = [units[key] for key in sorted(units)]
    return ordered, sorted(set(compile_only)), root


def selector(unit):
    if unit['kind'] == 'lib':
        return ['--lib']
    return ['--' + unit['kind'], unit['name']]


def unit_commands(unit, index, args, directory, sep):
    base = UNIT_TEST + selector(unit) + ['--']
    return [
        ('list', base + ['--list']),
        ('ignored', base + ['--list', '--ignored']),
        ('selected', base + ['--list'] + args),
        ('run', base + args + ['--logfile', directory + sep + 'u%d-results.log' % index]),
    ]


def same_source(paths, recorded, running):
    return paths.normcase(recorded.replace('\\', '/')) == paths.normcase(running.replace('\\', '/'))


def unit_files(count):
    names = list(FIXED_FILES)
    for index in range(count):
        names += ['u%d-%s.log' % (index, step) for step in STEPS] + ['u%d-results.log' % index]
    return sorted(names)


def digest(path):
    sha = hashlib.sha256()
    with open(path, 'rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            sha.update(block)
    return sha.hexdigest()


def identity(path):
    try:
        status = os.stat(path)
        return [digest(path), status.st_size, status.st_mtime_ns]
    except OSError as error:
        return ['missing: %s' % error.strerror, None, None]


def compiler_facts(text, where):
    """The facts of a compiler query's answer: one `rustc ...` line, then one `key: value` line per fact. An answer
    with no `rustc` line, or with no host or release, or with either twice, is refused, never read as a compiler."""
    lines = text.splitlines()
    facts = {}
    for line in lines[1:]:
        if ': ' in line:
            key, value = line.split(': ', 1)
            if key in ('host', 'release') and key in facts:
                raise Refused('%s: the compiler query answered %s twice' % (where, key))
            facts[key] = value
    if not lines or not lines[0].startswith('rustc ') or not facts.get('host') or not facts.get('release'):
        raise Refused('%s: the compiler query printed no rustc version, host and release: %r' % (where, text[:200]))
    return facts


def capture(argv):
    result = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        raise Refused('%s exited %d: %s' % (' '.join(argv), result.returncode, result.stderr.decode('utf-8', 'replace').strip()))
    return {'argv': argv, 'exit': result.returncode, 'stdout': result.stdout.decode('utf-8', 'replace')}


def stream(argv, out_path, err_path, live):
    """stdout to out_path; stderr to err_path, copied to this script's own output when `live`."""
    sys.stdout.flush()
    with open(out_path, 'wb') as out, open(err_path, 'wb') as err:
        process = subprocess.Popen(argv, stdout=out, stderr=subprocess.PIPE)
        while True:
            chunk = process.stderr.read1(1 << 16)
            if not chunk:
                break
            err.write(chunk)
            if live:
                sys.stdout.buffer.write(chunk)
                sys.stdout.flush()
        process.stderr.close()
        return process.wait()


def tee(argv, path):
    if DIAG.active:
        return DIAG.tee(argv, path)
    sys.stdout.flush()
    out = sys.stdout.buffer
    with open(path, 'wb') as log:
        process = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        while True:
            chunk = process.stdout.read1(1 << 16)
            if not chunk:
                break
            log.write(chunk)
            out.write(chunk)
            out.flush()
        process.stdout.close()
        return process.wait()


def into(argv, path):
    with open(path, 'wb') as log:
        return subprocess.run(argv, stdout=log, stderr=subprocess.STDOUT).returncode


# DIAGNOSTIC, temporary: #339's diagnostic head only, never its final head.
#
# On Windows, and only there (os.name == 'nt'), `run` is observed. Off Windows nothing here runs.
#
# The hard guard comes first: one thread and one absolute deadline, DIAG_HARD_DEADLINE after the diagnostics begin (relative to
# this script, not to the job: the job's prelude is not observed here), armed before the directory, the job or the controls,
# and never disarmed while the script lives. It shares no lock with the observed path and waits on no diagnostic I/O beyond
# DIAG_HARD_LOG_WAIT for one best-effort line; then it ends the script by terminating the job, every owned member with it. Any
# exception in a watcher, or in the guard itself, ends the script through the same path: protection is never silently lost.
# That is the EMERGENCY end, recorded as `hard-end`. Every other end is OWNED and recorded as `end`, in order, in the event
# file: a stop recorded in full, a refusal, or the script's own return. Once a guard is armed the script never ends through
# the interpreter's exit, whose finalization runs after the guard's thread has stopped: `main` hands whatever it returns or
# raises to diag_main, which ends the script through the guard's own termination, with the status Python would have given
# it, while the deadline is still armed.
#
# Ownership is required. The script makes a job object and joins it, so that everything it starts afterwards, and everything
# those start, is a member from its creation unless it breaks away; the job also ends its members when its last handle closes.
# Without the job the leg is refused (exit DIAG_NO_JOB_EXIT) before any control or test runs. A pid is acted on only after a
# handle opened by it is checked against the job and its creation time read; that same handle is terminated and waited on.
#
# Three controls run first, as the instrument's own self-test; the leg is refused (exit DIAG_SELFTEST_EXIT) unless each comes
# out as predicted. Then each command `run` tees has its direct Cargo child recorded at creation, its exit observed by its own
# handle apart from the read loop, its merged pipe's EOF, its bytes and each chunk's forwarding step, and every later phase.
#
# One terminal state decides how the leg ends: a stop and normal completion each claim it, and the first claim wins. A stop
# that wins fails the leg (exit DIAG_STOP_EXIT) and holds the main thread at its next phase; completion that wins makes any
# later stop stale; an ordinary exit after a stop has won waits, under the guard, for the stop to end the script. A stop takes
# at most DIAG_ACTIONS actions in all, newest member first, and records each check, call and wait, the listings before and
# after, what it skipped and what remained: collection is complete only when all of that says so; a wait that failed or
# answered anything unexpected is kept as it came and earns no credit. When EOF was first seen is recorded against the
# actions' start times only: after which action began, and that action's eventual result, never whether EOF came before or
# after its termination call; it names no holder, and no EOF names no outside process.
#
# Milestones go to stderr, which the job log keeps up to a cancellation, and to DIAG_DIRECTORY/<shard>.jsonl, beside the
# record directory and never inside it, as best effort: a failing sink is counted, a blocked one is bounded by the guard.
# Nothing here reads a test result: a summary line is counted as an observation and decides nothing.

DIAG_DIRECTORY = 'test-partition-diag'
DIAG_HARD_DEADLINE = 45 * 60
DIAG_SELFTEST_HARD_DEADLINE = 5 * 60
DIAG_HARD_LOG_WAIT = 2
DIAG_RUN_DEADLINE = 38 * 60
DIAG_EOF_GRACE = 120
DIAG_SNAPSHOTS = (120, 300)
DIAG_NO_PROGRESS = 600
DIAG_ACTIVE_WINDOW = 60
DIAG_TERMINATION_WAIT = 5
DIAG_ACTIONS = 16
DIAG_FINAL_EOF_WAIT = 10
DIAG_STOP_BUDGET = 240
DIAG_CONTROL_STOP_BUDGET = 30
DIAG_STOP_EXIT = 97
DIAG_HARD_EXIT = 96
DIAG_SELFTEST_EXIT = 3
DIAG_NO_JOB_EXIT = 4
DIAG_CONTROL_C_EXIT = -1073741510
DIAG_TERMINATED = 0xD1A6
DIAG_LIST_CAPACITY = 4096
DIAG_MESSAGES = 256
DIAG_RUN_BOUNDS = (DIAG_RUN_DEADLINE, DIAG_EOF_GRACE, DIAG_SNAPSHOTS, DIAG_NO_PROGRESS, DIAG_STOP_BUDGET)
DIAG_SELFTEST_BOUNDS = (60, 3, (), 4, DIAG_CONTROL_STOP_BUDGET)
DIAG_SUMMARY = re.compile(rb'test result: (?:ok|FAILED)\. [0-9]+ passed;')
DIAG_CONTROLS = (
    ('c3', 'import sys; sys.stdout.write("partition-diag control c3: prints, then exits\\n"); sys.stdout.flush()'),
    ('c1', 'import sys, time; sys.stdout.write("partition-diag control c1: prints, then stays\\n"); sys.stdout.flush(); '
           'time.sleep(600)'),
    ('c2', 'import subprocess, sys; subprocess.Popen([sys.executable, "-I", "-c", "import time; time.sleep(600)"], '
           'stdin=subprocess.DEVNULL, stdout=sys.stdout.fileno(), stderr=subprocess.DEVNULL); '
           'sys.stdout.write("partition-diag control c2: leaves a child holding this pipe, then exits\\n"); sys.stdout.flush()'),
)


class DiagWatch(object):
    """What was seen of one command's direct child and its merged pipe, in seconds of the observer's clock; None until seen."""

    def __init__(self, label, process, spawned):
        self.label = label
        self.process = process
        self.pid = process.pid
        self.spawned = spawned
        self.first_byte = None
        self.last_byte = None
        self.bytes = 0
        self.summaries = 0
        self.tail = b''
        self.forward = None
        self.exited = None
        self.code = None
        self.eof = None
        self.waited = None


def diag_progress(watch, phase_since):
    """The last time anything was seen to move: the phase, or the watched command's spawn, a byte, its exit, EOF or wait."""
    seen = [phase_since]
    if watch is not None:
        seen += [t for t in (watch.spawned, watch.last_byte, watch.exited, watch.eof, watch.waited) if t is not None]
    return max(seen)


def diag_decide(now, started, watch, phase_since, taken, bounds):
    """The watchdog's next act at `now`: None, ('snapshot', why, key) or ('stop', why). `taken` holds the keys of the snapshots
    already taken. It reads no output: a printed summary moves nothing here."""
    deadline, eof_grace, snapshots, no_progress = bounds[:4]
    if now - started >= deadline:
        return ('stop', 'deadline')
    if watch is not None and watch.exited is not None and watch.eof is None and now - watch.exited >= eof_grace:
        return ('stop', 'exit-without-eof')
    since = diag_progress(watch, phase_since)
    if now - since >= no_progress:
        return ('stop', 'no-progress')
    for quiet in snapshots:
        if now - since >= quiet and (since, quiet) not in taken:
            return ('snapshot', 'no progress for %ds' % quiet, (since, quiet))
    return None


def diag_classify(why, watch, phase_since, now):
    """The state a stop found, named from what was seen of the watched command (None between commands). A name for what was
    observed, never a cause."""
    if why == 'deadline' and now - diag_progress(watch, phase_since) < DIAG_ACTIVE_WINDOW:
        return 'active-at-deadline'
    if watch is None:
        return 'between-commands'
    if watch.forward is not None:
        return 'forward-blocked'
    if watch.exited is not None and watch.eof is None:
        return 'exit-without-eof'
    if watch.exited is None and watch.eof is None:
        return 'no-exit'
    if watch.exited is None:
        return 'eof-without-exit'
    if watch.waited is None:
        return 'wait-blocked'
    return 'post-eof'


def diag_order(members, own):
    """The members a stop acts on, one at a time, newest first: only those whose own handle was checked to be in the job."""
    return sorted((m for m in members if m.get('verified') and m['pid'] != own), key=lambda m: (-m['created'], m['pid']))


def diag_sighted(watch, chunk):
    """How many libtest summary lines end in `chunk`, one split across two reads counted once. An observation only."""
    joined = watch.tail + chunk
    found = len(DIAG_SUMMARY.findall(joined)) - len(DIAG_SUMMARY.findall(watch.tail))
    watch.tail = joined[-256:]
    return found


def diag_wait(api, value):
    """One WaitForSingleObject answer, kept as it came: `signalled` (WAIT_OBJECT_0), `timeout` (WAIT_TIMEOUT), `failed`
    (WAIT_FAILED, with the error GetLastError gives at once) or `unexpected` (any other value). Only `signalled` observes an
    exit; `failed` and `unexpected` observe nothing."""
    if value == api.WAIT_OBJECT_0:
        return {'wait': 'signalled', 'result': value, 'error': None}
    if value == api.WAIT_TIMEOUT:
        return {'wait': 'timeout', 'result': value, 'error': None}
    if value == api.WAIT_FAILED:
        return {'wait': 'failed', 'result': value, 'error': api.last()}
    return {'wait': 'unexpected', 'result': value, 'error': None}


def diag_confirmed(result):
    """Whether one action is a confirmed termination: the member checked, the call returned true, its handle signalled."""
    return result.get('checked') is True and result.get('call') is True and result.get('wait') == 'signalled'


def diag_gone(result):
    """Whether one action found its member already gone: checked, and its zero-time wait signalled. A failed or unexpected
    wait, or none, is not an exit observation and earns nothing here."""
    return (result.get('checked') is True and result.get('already_exited') is True
            and (result.get('prewait') or {}).get('wait') == 'signalled')


def diag_collection(first, actions, last, skipped):
    """Whether a stop's owned collection is complete, with every reason it is not. Complete needs both listings read whole
    (no error, nothing truncated, every listed pid checked), every action a confirmed termination or a checked member whose
    zero-time wait signalled (diag_gone), nothing skipped for the cap or the budget, and no checked member left in the last
    listing."""
    reasons = []
    for name, listing in (('first', first), ('last', last)):
        if listing is None:
            reasons.append('the %s listing was not read' % name)
            continue
        if listing.get('error'):
            reasons.append('the %s listing failed: %s' % (name, listing['error']))
        if listing.get('truncated'):
            reasons.append('the %s listing was truncated' % name)
        unchecked = [m.get('pid') for m in listing.get('members', []) if not m.get('verified')]
        if unchecked:
            reasons.append('the %s listing holds unchecked pids %r' % (name, unchecked))
    for action in actions:
        result = action['result']
        if not (diag_confirmed(result) or diag_gone(result)):
            reasons.append('action %d on pid %d is not confirmed: %s' % (
                action['index'], action['pid'], json.dumps(result, sort_keys=True)))
    if skipped:
        reasons.append('not acted on (the cap or the budget): %r' % skipped)
    if last is not None and not last.get('error'):
        remaining = [m['pid'] for m in last.get('members', []) if m.get('verified')]
        if remaining:
            reasons.append('checked members remain: %r' % remaining)
    return {'complete': not reasons, 'reasons': reasons}


def diag_eof_timing(actions, eof, stop_at):
    """When EOF was first seen, against a stop's actions, as time only: before the stop, before any action, or after the
    start of the last action begun before it, with that action's eventual `result` (`confirmed` only when its own check,
    call and signalled wait say so; any other is `unconfirmed`) and `within_action` (whether that action had not yet
    returned). An action starts before its membership check and its termination call, so the label does not say that EOF
    came after the call. It names no holder: EOF after an action began is not evidence that the member held the pipe, and
    no EOF is not evidence of a holder outside the job."""
    if eof is None:
        return {'eof': 'not-observed'}
    if eof < stop_at:
        return {'eof': 'before-the-stop'}
    begun = [a for a in actions if a['began'] <= eof]
    if not begun:
        return {'eof': 'before-any-action'}
    last = begun[-1]
    return {'eof': 'after-an-action-began', 'action': last['index'], 'pid': last['pid'],
            'result': 'confirmed' if diag_confirmed(last['result']) else 'unconfirmed', 'within_action': eof <= last['ended']}


def diag_selftest_accept(results):
    """Whether the controls came out as the instrument predicts: c3 completed; c1 stopped as no-exit and c2 as
    exit-without-eof, each with EOF first seen after an action began whose own results confirm a termination (c1: an action
    on its direct child; c2: on another member) and each collection complete; every control's direct child a checked
    member. Whether EOF came before or after that action's termination call is not part of the prediction."""
    c1, c2, c3 = (results.get(name) or {} for name in ('c1', 'c2', 'c3'))
    if c3.get('class') != 'completed':
        return False
    for control, expected, direct in ((c1, 'no-exit', True), (c2, 'exit-without-eof', False)):
        timing = control.get('timing') or {}
        if control.get('class') != expected or timing.get('eof') != 'after-an-action-began' \
                or timing.get('result') != 'confirmed':
            return False
        if (timing.get('pid') == control.get('pid')) is not direct:
            return False
        if (control.get('collection') or {}).get('complete') is not True:
            return False
    return all(r.get('direct_verified') is True for r in (c1, c2, c3))


def diag_try(sink, event):
    """One best-effort event: a failing sink is not the diagnostic's outcome."""
    try:
        sink(event)
        return True
    except Exception:
        return False


class DiagSink(object):
    """Each event as one JSON line in the leg's event file, flushed and synced at once, and on stderr unless `quiet`."""

    def __init__(self, path):
        self.lock = threading.Lock()
        self.handle = open(path, 'x', encoding='utf-8')

    def __call__(self, event, quiet=False):
        line = json.dumps(event, sort_keys=True, separators=(',', ':'), default=str)
        with self.lock:
            self.handle.write(line + '\n')
            self.handle.flush()
            os.fsync(self.handle.fileno())
            if not quiet:
                sys.stderr.write('partition-diag: ' + line + '\n')
                sys.stderr.flush()


class DiagGuard(object):
    """The hard bound: one thread and one absolute deadline on the monotonic clock, never disarmed. It takes no lock the
    observed path takes and waits on no diagnostic I/O beyond `log_wait` for one best-effort line, written by a short-lived
    thread straight to file descriptor 2; then it ends the script with its code. Two kinds of end share one termination:
    `end` is the EMERGENCY end (the deadline, or a watcher's or the guard's own failure) and records `hard-end` that way;
    `close` is an OWNED end (a stop recorded in full, a refusal, or the script's own return through diag_main) and records
    `end` in order through the event file (`note`), best effort. Both then `halt`: through `job_end` (terminating the job,
    so every owned member with it) once the job exists, else through `ends` (this process alone), else os._exit. Neither
    disarms the deadline: an owned end that blocks is still cut by it."""

    def __init__(self, shard, seconds, clock=time.monotonic, sleep=time.sleep, write=None, ends=(), log_wait=DIAG_HARD_LOG_WAIT):
        self.shard = shard
        self.clock = clock
        self.sleep = sleep
        self.deadline = clock() + seconds
        self.write = write if write is not None else (lambda data: os.write(2, data))
        self.ends = list(ends)
        self.log_wait = log_wait
        self.job_end = None
        self.note = None

    @staticmethod
    def arm(shard, seconds):
        global DIAG_GUARD
        guard = DiagGuard(shard, seconds, ends=diag_ends())
        DIAG_GUARD = guard
        threading.Thread(target=guard.run, daemon=True).start()
        return guard

    def due(self, now):
        return now >= self.deadline

    def run(self):
        try:
            while not self.due(self.clock()):
                self.sleep(max(0.0, min(1.0, self.deadline - self.clock())))
            why = 'the hard deadline'
        except BaseException as error:
            why = 'the guard failed: %s' % type(error).__name__
        self.end(DIAG_HARD_EXIT, why)

    def end(self, code, why):
        """The emergency end: one best-effort `hard-end` line on file descriptor 2, waited for at most `log_wait`; then halt."""
        event = {'kind': 'hard-end', 'shard': self.shard, 'why': why, 'code': code}
        line = ('partition-diag: ' + json.dumps(event, sort_keys=True, separators=(',', ':')) + '\n').encode('utf-8')
        try:
            writer = threading.Thread(target=self.write, args=(line,), daemon=True)
            writer.start()
            writer.join(self.log_wait)
        except BaseException:
            pass
        self.halt(code)

    def close(self, code, why):
        """An owned end: its `end` event, in order through the event file and best effort, then halt. The deadline stays
        armed while the event is written and the script halts."""
        if self.note is not None:
            diag_try(self.note, {'kind': 'end', 'shard': self.shard, 'code': code, 'why': why})
        self.halt(code)

    def halt(self, code):
        """End the script with `code`: the job's end, else this process's own, else os._exit; a failing end falls through
        to the next. None of them runs the interpreter's finalization. An end that succeeds does not return natively."""
        for end in ([self.job_end] if self.job_end is not None else []) + self.ends:
            try:
                if end(code):
                    return
            except BaseException:
                pass
        os._exit(code)


def diag_ends():
    """This process's own end, for the guard before the job exists: TerminateProcess on itself, which runs no handler and
    waits on nothing."""
    try:
        api = diag_win32()
        current = api.GetCurrentProcess()
    except BaseException:
        return []
    return [lambda code: bool(api.TerminateProcess(current, code))]


def diag_win32():
    """kernel32 and ntdll through ctypes, typed (Windows only)."""
    import ctypes
    from ctypes import wintypes

    class Api(object):
        pass

    api = Api()
    api.ref, api.size, api.buffer = ctypes.byref, ctypes.sizeof, ctypes.create_unicode_buffer
    api.DWORD, api.BOOL, api.FILETIME, api.c_size_t, api.c_void_p = (
        wintypes.DWORD, wintypes.BOOL, wintypes.FILETIME, ctypes.c_size_t, ctypes.c_void_p)
    api.last = ctypes.get_last_error
    api.error = lambda name: OSError(ctypes.get_last_error(), name)
    api.INVALID_HANDLE_VALUE = wintypes.HANDLE(-1)
    api.INFINITE, api.WAIT_OBJECT_0, api.WAIT_TIMEOUT, api.WAIT_FAILED, api.ERROR_MORE_DATA = 0xFFFFFFFF, 0, 0x102, 0xFFFFFFFF, 234
    api.PROCESS_TERMINATE, api.PROCESS_QUERY_LIMITED_INFORMATION, api.SYNCHRONIZE = 0x0001, 0x1000, 0x00100000
    api.JOB_OBJECT_LIMIT_BREAKAWAY_OK, api.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00000800, 0x00002000

    class BasicLimit(ctypes.Structure):
        _fields_ = [('PerProcessUserTimeLimit', ctypes.c_int64), ('PerJobUserTimeLimit', ctypes.c_int64),
                    ('LimitFlags', wintypes.DWORD), ('MinimumWorkingSetSize', ctypes.c_size_t),
                    ('MaximumWorkingSetSize', ctypes.c_size_t), ('ActiveProcessLimit', wintypes.DWORD),
                    ('Affinity', ctypes.c_size_t), ('PriorityClass', wintypes.DWORD), ('SchedulingClass', wintypes.DWORD)]

    class IoCounters(ctypes.Structure):
        _fields_ = [(name, ctypes.c_uint64) for name in ('ReadOperationCount', 'WriteOperationCount', 'OtherOperationCount',
                                                          'ReadTransferCount', 'WriteTransferCount', 'OtherTransferCount')]

    class ExtendedLimit(ctypes.Structure):
        _fields_ = [('BasicLimitInformation', BasicLimit), ('IoInfo', IoCounters), ('ProcessMemoryLimit', ctypes.c_size_t),
                    ('JobMemoryLimit', ctypes.c_size_t), ('PeakProcessMemoryUsed', ctypes.c_size_t),
                    ('PeakJobMemoryUsed', ctypes.c_size_t)]

    class CompletionPort(ctypes.Structure):
        _fields_ = [('CompletionKey', ctypes.c_void_p), ('CompletionPort', wintypes.HANDLE)]

    class Accounting(ctypes.Structure):
        _fields_ = [('TotalUserTime', ctypes.c_int64), ('TotalKernelTime', ctypes.c_int64),
                    ('ThisPeriodTotalUserTime', ctypes.c_int64), ('ThisPeriodTotalKernelTime', ctypes.c_int64),
                    ('TotalPageFaultCount', wintypes.DWORD), ('TotalProcesses', wintypes.DWORD),
                    ('ActiveProcesses', wintypes.DWORD), ('TotalTerminatedProcesses', wintypes.DWORD)]

    class Listing(ctypes.Structure):
        _fields_ = [('NumberOfAssignedProcesses', wintypes.DWORD), ('NumberOfProcessIdsInList', wintypes.DWORD),
                    ('ProcessIdList', ctypes.c_size_t * DIAG_LIST_CAPACITY)]

    class BasicInformation(ctypes.Structure):
        _fields_ = [('ExitStatus', ctypes.c_void_p), ('PebBaseAddress', ctypes.c_void_p), ('Reserved2', ctypes.c_void_p * 2),
                    ('UniqueProcessId', ctypes.c_size_t), ('InheritedFromUniqueProcessId', ctypes.c_size_t)]

    api.ExtendedLimit, api.CompletionPort, api.Accounting, api.Listing, api.BasicInformation = (
        ExtendedLimit, CompletionPort, Accounting, Listing, BasicInformation)
    kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)
    ntdll = ctypes.WinDLL('ntdll')

    def bind(dll, name, result, *arguments):
        function = getattr(dll, name)
        function.restype, function.argtypes = result, list(arguments)
        setattr(api, name, function)

    H, D, B, P, R = wintypes.HANDLE, wintypes.DWORD, wintypes.BOOL, ctypes.c_void_p, ctypes.POINTER
    bind(kernel32, 'CreateJobObjectW', H, P, wintypes.LPCWSTR)
    bind(kernel32, 'SetInformationJobObject', B, H, ctypes.c_int, P, D)
    bind(kernel32, 'QueryInformationJobObject', B, H, ctypes.c_int, P, D, R(D))
    bind(kernel32, 'AssignProcessToJobObject', B, H, H)
    bind(kernel32, 'TerminateJobObject', B, H, wintypes.UINT)
    bind(kernel32, 'GetCurrentProcess', H)
    bind(kernel32, 'CreateIoCompletionPort', H, H, H, ctypes.c_size_t, D)
    bind(kernel32, 'GetQueuedCompletionStatus', B, H, R(D), R(ctypes.c_size_t), R(ctypes.c_void_p), D)
    bind(kernel32, 'OpenProcess', H, D, B, D)
    bind(kernel32, 'IsProcessInJob', B, H, H, R(B))
    bind(kernel32, 'QueryFullProcessImageNameW', B, H, D, wintypes.LPWSTR, R(D))
    bind(kernel32, 'GetProcessTimes', B, H, R(wintypes.FILETIME), R(wintypes.FILETIME), R(wintypes.FILETIME),
         R(wintypes.FILETIME))
    bind(kernel32, 'GetExitCodeProcess', B, H, R(D))
    bind(kernel32, 'WaitForSingleObject', D, H, D)
    bind(kernel32, 'TerminateProcess', B, H, wintypes.UINT)
    bind(kernel32, 'CloseHandle', B, H)
    bind(ntdll, 'NtQueryInformationProcess', ctypes.c_long, H, ctypes.c_int, P, ctypes.c_ulong, R(ctypes.c_ulong))
    return api


class DiagJob(object):
    """A job object this script makes and joins (Windows), so that every process it starts afterwards, and every process those
    start, is a member from its creation unless it breaks away. Breakaway stays allowed, as it was outside this job: a process
    that breaks away is outside this observation. The job ends its remaining members when its last handle closes, which is
    this script's own end, however it comes. A pid is named a member only after a handle opened by it is checked with
    IsProcessInJob against this job and its creation time read; a pid alone is never taken as one of ours. The job's own
    new-process and exit messages are counted, never relied on: Windows does not guarantee their delivery."""

    def __init__(self):
        self.api = api = diag_win32()
        self.own = os.getpid()
        self.lock = threading.Lock()
        self.counts = {}
        self.messages = []
        self.job = api.CreateJobObjectW(None, None)
        if not self.job:
            raise api.error('CreateJobObjectW')
        limit = api.ExtendedLimit()
        limit.BasicLimitInformation.LimitFlags = api.JOB_OBJECT_LIMIT_BREAKAWAY_OK | api.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not api.SetInformationJobObject(self.job, 9, api.ref(limit), api.size(limit)):
            raise api.error('SetInformationJobObject (limits)')
        self.port = api.CreateIoCompletionPort(api.INVALID_HANDLE_VALUE, None, 0, 1)
        if not self.port:
            raise api.error('CreateIoCompletionPort')
        port = api.CompletionPort(None, self.port)
        if not api.SetInformationJobObject(self.job, 7, api.ref(port), api.size(port)):
            raise api.error('SetInformationJobObject (completion port)')
        if not api.AssignProcessToJobObject(self.job, api.GetCurrentProcess()):
            raise api.error('AssignProcessToJobObject (this script)')
        threading.Thread(target=self.listen, daemon=True).start()

    def end(self, code):
        """Terminate the job: every member, this script included. It does not return when it succeeds."""
        return bool(self.api.TerminateJobObject(self.job, code))

    def listen(self):
        api = self.api
        message, key, pid = api.DWORD(), api.c_size_t(), api.c_void_p()
        while api.GetQueuedCompletionStatus(self.port, api.ref(message), api.ref(key), api.ref(pid), api.INFINITE):
            with self.lock:
                self.counts[message.value] = self.counts.get(message.value, 0) + 1
                self.messages.append((round(time.monotonic(), 3), message.value, pid.value or 0))
                del self.messages[:-DIAG_MESSAGES]

    def identify(self, handle, pid):
        """A handle's process, checked: a member of this job, its creation time and image, and its state as the zero-time
        wait answered (`prewait`, diag_wait): `running` on WAIT_TIMEOUT, `exited` on WAIT_OBJECT_0, else `unknown`. Only
        `exited` reads the exit code, and only a read that succeeds gives one; a failed read keeps its error and no code."""
        api = self.api
        inside = api.BOOL()
        if not api.IsProcessInJob(handle, self.job, api.ref(inside)) or not inside.value:
            return {'pid': pid, 'verified': False, 'why': 'not a member of this job'}
        created, ended, kernel, user = api.FILETIME(), api.FILETIME(), api.FILETIME(), api.FILETIME()
        if not api.GetProcessTimes(handle, api.ref(created), api.ref(ended), api.ref(kernel), api.ref(user)):
            return {'pid': pid, 'verified': False, 'why': 'GetProcessTimes: %d' % api.last()}
        name, size = api.buffer(32768), api.DWORD(32768)
        image = name.value if api.QueryFullProcessImageNameW(handle, 0, name, api.ref(size)) else None
        prewait = diag_wait(api, api.WaitForSingleObject(handle, 0))
        state = {'timeout': 'running', 'signalled': 'exited'}.get(prewait['wait'], 'unknown')
        code, code_error = None, None
        if state == 'exited':
            value = api.DWORD()
            if api.GetExitCodeProcess(handle, api.ref(value)):
                code = value.value
            else:
                code_error = api.last()
        basic = api.BasicInformation()
        status = api.NtQueryInformationProcess(handle, 0, api.ref(basic), api.size(basic), None)
        return {'pid': pid, 'verified': True, 'image': image, 'created': (created.dwHighDateTime << 32) | created.dwLowDateTime,
                'state': state, 'prewait': prewait, 'code': code, 'code_error': code_error,
                'parent_best_effort': basic.InheritedFromUniqueProcessId if status == 0 else None}

    def describe(self, pid):
        api = self.api
        handle = api.OpenProcess(api.PROCESS_QUERY_LIMITED_INFORMATION | api.SYNCHRONIZE, False, pid)
        if not handle:
            return {'pid': pid, 'verified': False, 'why': 'OpenProcess: %d' % api.last()}
        try:
            return self.identify(handle, pid)
        finally:
            api.CloseHandle(handle)

    def members(self):
        """The job's current members other than this script, each checked; whether the listing was whole."""
        api = self.api
        listing = api.Listing()
        error = None
        if not api.QueryInformationJobObject(self.job, 3, api.ref(listing), api.size(listing), None):
            error = api.last()
            if error != api.ERROR_MORE_DATA:
                return {'error': 'QueryInformationJobObject: %d' % error, 'members': []}
        accounting = api.Accounting()
        api.QueryInformationJobObject(self.job, 1, api.ref(accounting), api.size(accounting), None)
        with self.lock:
            counts, recent = dict(self.counts), list(self.messages[-16:])
        listed = min(listing.NumberOfProcessIdsInList, DIAG_LIST_CAPACITY)
        pids = [listing.ProcessIdList[index] for index in range(listed)]
        return {'assigned': listing.NumberOfAssignedProcesses, 'listed': listed,
                'truncated': error == api.ERROR_MORE_DATA or listed < listing.NumberOfAssignedProcesses,
                'total': accounting.TotalProcesses, 'active': accounting.ActiveProcesses,
                'terminated': accounting.TotalTerminatedProcesses, 'messages': counts, 'recent_messages': recent,
                'members': [self.describe(pid) for pid in pids if pid != self.own]}

    def terminate(self, member):
        """One action on one listed member: the handle opened by its pid is checked to be in this job and to be the process
        listed (its creation time); that same handle is terminated and waited on. Each step's own result is kept: the check;
        the zero-time wait before any call (`prewait`); the call and its error; and the wait after it (`wait`, with
        `wait_detail`), each answer as diag_wait gives it. Only a signalled zero-time wait makes the member
        `already_exited`; a failed or unexpected one is unknown, and no call follows it."""
        api = self.api
        out = {'checked': False, 'prewait': None, 'call': None, 'error': None, 'wait': None, 'already_exited': False}
        handle = api.OpenProcess(api.PROCESS_TERMINATE | api.PROCESS_QUERY_LIMITED_INFORMATION | api.SYNCHRONIZE, False,
                                 member['pid'])
        if not handle:
            out['error'] = 'OpenProcess: %d' % api.last()
            return out
        try:
            now = self.identify(handle, member['pid'])
            if not now['verified'] or now['created'] != member['created']:
                out['error'] = 'not the member listed'
                return out
            out['checked'], out['prewait'] = True, now['prewait']
            if now['state'] == 'exited':
                out['already_exited'], out['code'], out['code_error'] = True, now['code'], now['code_error']
                return out
            if now['state'] != 'running':
                out['error'] = 'the zero-time wait was %s: no exit observed, and no call made' % now['prewait']['wait']
                return out
            out['call'] = bool(api.TerminateProcess(handle, DIAG_TERMINATED))
            if not out['call']:
                out['error'] = 'TerminateProcess: %d' % api.last()
                return out
            out['wait_detail'] = diag_wait(api, api.WaitForSingleObject(handle, DIAG_TERMINATION_WAIT * 1000))
            out['wait'] = out['wait_detail']['wait']
            return out
        finally:
            api.CloseHandle(handle)


class DiagInert(object):
    """Diagnostics off, on every platform but Windows: each hook does nothing."""

    active = False

    def phase(self, name):
        pass

    def bind(self, record):
        pass

    def finished(self, status):
        pass


DIAG = DiagInert()
DIAG_GUARD = None
DIAG_ENTERED = False


class Diag(object):
    """One observer (Windows): of a leg (`final`: a stop it wins ends the script) or of one self-test control."""

    active = True

    def __init__(self, shard, sink, job, bounds, final, guard, clock=time.monotonic, sleep=time.sleep, hold=None):
        self.shard = shard
        self.sink = sink
        self.job = job
        self.bounds = bounds
        self.final = final
        self.guard = guard
        self.now = clock
        self.sleep = sleep
        self.own = os.getpid()
        self.lock = threading.Lock()
        self.terminal_lock = threading.Lock()
        self.terminal = None
        self.started = self.now()
        self.phase_name = 'begin'
        self.phase_since = self.started
        self.watch = None
        self.last_pid = None
        self.direct_verified = None
        self.sink_failures = 0
        self.done = threading.Event()
        self.stop_done = threading.Event()
        self.hold = hold if hold is not None else self.stop_done.wait
        self.result = None

    def emit(self, kind, quiet=False, **fields):
        fields.update(kind=kind, shard=self.shard, t=round(self.now() - self.started, 3))
        try:
            self.sink(fields, quiet)
        except Exception:
            self.sink_failures += 1

    def claim(self, kind):
        """The one terminal state: the first claim wins and every later one loses. No I/O is done under its lock."""
        with self.terminal_lock:
            if self.terminal is None:
                self.terminal = kind
                return True
            return False

    def stopping(self):
        with self.terminal_lock:
            return self.terminal == 'stopping'

    def phase(self, name):
        if self.stopping():
            self.hold()
        with self.lock:
            self.phase_name = name
            self.phase_since = self.now()
        self.emit('phase', phase=name)

    def bind(self, record):
        github = record['github']
        self.emit('bind', checkout=record['checkout'], sha=github.get('GITHUB_SHA'), run=github.get('GITHUB_RUN_ID'),
                  attempt=github.get('GITHUB_RUN_ATTEMPT'), job=github.get('GITHUB_JOB'), event=github.get('GITHUB_EVENT_NAME'))

    def finished(self, status):
        """Normal completion, after record.json: it claims the terminal state; if a stop has won, it waits for that stop,
        which ends the script (or releases a control once recorded), and never returns as a success."""
        if not self.claim('completed'):
            self.hold()
            return False
        self.done.set()
        self.emit('terminal', winner='completed', status=status)
        self.emit('completed', status=status, state=self.state(), members=self.members())
        return True

    def at_exit(self):
        """Every ordinary end of the script (a return, a refusal, an uncaught exception; diag_main calls it) claims the
        terminal state too: after a stop has won, the script does not end ordinarily; it waits, under the guard, for the stop
        to end it."""
        if not self.claim('exited') and self.stopping():
            self.hold()

    def members(self):
        return self.job.members() if self.job is not None else None

    def state(self):
        with self.lock:
            watch = self.watch
            fields = {'phase': self.phase_name, 'phase_for': round(self.now() - self.phase_since, 3)}
            if watch is not None:
                for name in ('spawned', 'first_byte', 'last_byte', 'exited', 'eof', 'waited'):
                    value = getattr(watch, name)
                    fields[name] = None if value is None else round(value - self.started, 3)
                fields.update(pid=watch.pid, bytes=watch.bytes, summaries=watch.summaries, forward=watch.forward,
                              code=watch.code)
        return fields

    def tee(self, argv, path, forward=True):
        """`tee`, observed: the same Popen, the same read loop and the same writes, in the same order."""
        sys.stdout.flush()
        out = sys.stdout.buffer
        with open(path, 'wb') as log:
            process = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            watch = self.observe(argv, process)
            while True:
                chunk = process.stdout.read1(1 << 16)
                if not chunk:
                    break
                self.received(watch, chunk)
                watch.forward = 'log.write'
                log.write(chunk)
                if forward:
                    watch.forward = 'stdout.write'
                    out.write(chunk)
                    watch.forward = 'stdout.flush'
                    out.flush()
                watch.forward = None
            self.mark(watch, 'eof')
            self.phase('%s: closing the pipe, then waiting' % watch.label)
            process.stdout.close()
            code = process.wait()
            self.mark(watch, 'waited', code)
            with self.lock:
                self.watch = None
            return code

    def observe(self, argv, process):
        with self.lock:
            watch = DiagWatch(self.phase_name, process, self.now())
            self.watch = watch
            self.last_pid = watch.pid
        member = self.job.describe(process.pid) if self.job is not None else None
        self.direct_verified = None if member is None else member.get('verified')
        self.emit('spawn', pid=process.pid, argv=argv, member=member)
        threading.Thread(target=self.reap, args=(watch,), daemon=True).start()
        return watch

    def reap(self, watch):
        """The direct child's exit, by its own handle, apart from the read loop: exit and EOF are separate facts."""
        code = watch.process.wait()
        self.mark(watch, 'exited', code)

    def mark(self, watch, what, code=None):
        with self.lock:
            setattr(watch, what, self.now())
            if what == 'exited':
                watch.code = code
        self.emit(what, pid=watch.pid, code=code, bytes=watch.bytes, summaries=watch.summaries)

    def received(self, watch, chunk):
        with self.lock:
            first = watch.first_byte is None
            watch.last_byte = self.now()
            if first:
                watch.first_byte = watch.last_byte
            watch.bytes += len(chunk)
            sighted = diag_sighted(watch, chunk)
            watch.summaries += sighted
        if first:
            self.emit('first-byte', pid=watch.pid)
        if sighted:
            self.emit('summary-seen', pid=watch.pid, summaries=watch.summaries, bytes=watch.bytes)

    def watchdog(self):
        taken = set()
        while not self.done.wait(1.0):
            if self.look(taken):
                return

    def look(self, taken):
        """One watchdog look; any exception in it ends the script through the guard, never silently."""
        try:
            return self.tick(taken)
        except BaseException as error:
            self.guard.end(DIAG_HARD_EXIT, 'a watcher failed: %s' % type(error).__name__)
            return True

    def tick(self, taken):
        """One look: a snapshot, a stop (True), or nothing."""
        with self.lock:
            decision = diag_decide(self.now(), self.started, self.watch, self.phase_since, taken, self.bounds)
        if decision is None:
            return False
        if decision[0] == 'snapshot':
            taken.add(decision[2])
            self.emit('snapshot', why=decision[1], state=self.state(), members=self.members())
            return False
        self.stop(decision[1])
        return True

    def await_eof(self, watch, seconds):
        end = self.now() + seconds
        while watch.eof is None and self.now() < end:
            self.sleep(0.05)
        return watch.eof is not None

    def act(self, member, index):
        began = self.now()
        result = self.job.terminate(member)
        action = {'index': index, 'pid': member['pid'], 'created': member['created'], 'image': member.get('image'),
                  'began': began, 'ended': self.now(), 'result': result}
        self.emit('action', index=index, pid=member['pid'], image=member.get('image'), result=result,
                  began=round(began - self.started, 3), ended=round(action['ended'] - self.started, 3))
        return action

    def stop(self, why):
        """A stop, only once it wins the terminal state (else it is stale and does nothing): it records the state and the
        first listing; acts on checked members, newest first, re-listing for new ones, at most DIAG_ACTIONS actions and within
        its budget in all; waits for EOF after each while none is seen; lists once more; records the collection and when
        EOF was seen; and, for a leg, ends the script through the guard's owned end (`end`, DIAG_STOP_EXIT). A control is
        released once recorded."""
        if not self.claim('stopping'):
            self.emit('stale-stop', why=why)
            return False
        with self.lock:
            watch, phase = self.watch, self.phase_name
            found = diag_classify(why, watch, self.phase_since, self.now())
        stop_at = self.now()
        end = stop_at + self.bounds[4]
        self.emit('terminal', winner='stopping', why=why)
        first = self.members()
        self.emit('stop', why=why, found=found, phase=phase, state=self.state(), members=first)
        acted, actions, skipped, listing = set(), [], [], first
        while not skipped and listing is not None:
            fresh = [m for m in diag_order(listing.get('members', []), self.own) if (m['pid'], m['created']) not in acted]
            if not fresh:
                break
            for member in fresh:
                if len(actions) >= DIAG_ACTIONS or self.now() >= end:
                    skipped.append(member['pid'])
                    continue
                acted.add((member['pid'], member['created']))
                actions.append(self.act(member, len(actions)))
                if watch is not None and watch.eof is None:
                    self.await_eof(watch, max(0.0, min(DIAG_TERMINATION_WAIT, end - self.now())))
            if not skipped:
                listing = self.members()
        if watch is not None and watch.eof is None:
            self.await_eof(watch, max(0.0, min(DIAG_FINAL_EOF_WAIT, end - self.now())))
        last = self.members()
        collection = diag_collection(first, actions, last, skipped)
        timing = diag_eof_timing(actions, None if watch is None else watch.eof, stop_at)
        self.result = {'class': found, 'why': why, 'phase': phase, 'pid': None if watch is None else watch.pid,
                       'timing': timing, 'collection': collection}
        self.emit('collection', last=last, skipped=skipped, **collection)
        self.emit('classification', sink_failures=self.sink_failures, **self.result)
        if self.final:
            self.guard.close(DIAG_STOP_EXIT, 'a stop: %s' % found)
        elif watch is not None and watch.eof is None:
            self.guard.close(DIAG_SELFTEST_EXIT, 'a control whose EOF never came')
        self.stop_done.set()
        return True


def diag_begin(shard, guard, make_job=None, make_sink=None, controls=DIAG_CONTROLS):
    """The leg's diagnostic directory, its event file, the job, and the controls, under `guard`. Without the job the leg is
    refused (exit DIAG_NO_JOB_EXIT) before any control or test runs; unless the controls come out as predicted it is refused
    (exit DIAG_SELFTEST_EXIT)."""
    try:
        os.mkdir(DIAG_DIRECTORY)
    except OSError as error:
        raise Refused('the diagnostic directory %r: %s' % (DIAG_DIRECTORY, error))
    sink = (make_sink or DiagSink)(os.path.join(DIAG_DIRECTORY, '%s.jsonl' % shard))
    guard.note = sink
    try:
        job = (make_job or DiagJob)()
    except OSError as error:
        diag_try(sink, {'kind': 'refused', 'shard': shard, 'why': 'no job ownership: %s' % error})
        guard.close(DIAG_NO_JOB_EXIT, 'no job ownership')
        return None, None
    guard.job_end = job.end
    results = {}
    for name, code in controls:
        control = Diag('%s control %s' % (shard, name), sink, job, DIAG_SELFTEST_BOUNDS, False, guard)
        watchdog = threading.Thread(target=control.watchdog, daemon=True)
        watchdog.start()
        control.phase('control %s' % name)
        status = control.tee([sys.executable, '-I', '-c', code], os.path.join(DIAG_DIRECTORY, '%s-%s.log' % (shard, name)), False)
        control.finished(status)
        watchdog.join(120)
        result = dict(control.result or {'class': 'completed', 'pid': control.last_pid, 'status': status})
        result['direct_verified'] = control.direct_verified
        results[name] = result
    accepted = diag_selftest_accept(results)
    diag_try(sink, {'kind': 'selftest', 'shard': shard, 'results': results, 'accepted': accepted,
                    'python': sys.version.split()[0], 'executable': sys.executable})
    if not accepted:
        guard.close(DIAG_SELFTEST_EXIT, 'the self-test was refused')
    return sink, job


def diag_leg(shard):
    """Begin a leg's diagnostics (Windows): the hard guard first; then the directory, the job and the controls; then the leg's
    observer and its watchdog. The leg's end is diag_main's: an ordinary end claims the terminal state there."""
    guard = DiagGuard.arm(shard, DIAG_HARD_DEADLINE)
    sink, job = diag_begin(shard, guard)
    diag = Diag(shard, sink, job, DIAG_RUN_BOUNDS, True, guard)
    diag.emit('begin', pid=os.getpid(), bounds=list(DIAG_RUN_BOUNDS), hard=DIAG_HARD_DEADLINE, python=sys.version.split()[0])
    threading.Thread(target=diag.watchdog, daemon=True).start()
    return diag


def diag_selftest_only():
    """`partition.py diag-selftest` (Windows): the controls alone, in the current directory, under their own hard guard."""
    guard = DiagGuard.arm('selftest', DIAG_SELFTEST_HARD_DEADLINE)
    diag_begin('selftest', guard)
    return 0


def diag_status(error):
    """The exit status Python itself gives `error` raised out of main, as a signed 32-bit value: SystemExit's code (None is
    0, an integer is itself, anything else is printed to stderr and is 1); KeyboardInterrupt is STATUS_CONTROL_C_EXIT; any
    other exception's traceback is printed as the interpreter prints it, and is 1."""
    if isinstance(error, SystemExit):
        if error.code is None:
            return 0
        if isinstance(error.code, int):
            return ((error.code + (1 << 31)) % (1 << 32)) - (1 << 31)
        try:
            print(error.code, file=sys.stderr)
        except Exception:
            pass
        return 1
    try:
        sys.excepthook(type(error), error, error.__traceback__)
    except Exception:
        pass
    return DIAG_CONTROL_C_EXIT if isinstance(error, KeyboardInterrupt) else 1


def diag_main(main, argv):
    """`main` on Windows. Once a hard guard is armed (a leg, or the self-test), the script never ends through the
    interpreter's exit, whose finalization (atexit handlers, thread shutdown, the final flushes) runs after the guard's
    thread has stopped. What `main` returns or raises becomes the status `sys.exit` would give it (diag_status); an ordinary
    end claims the terminal state (`at_exit`: after a stop has won it waits for that stop); stdout and stderr are flushed;
    and the guard's owned end records `end` and halts the script, all while the deadline is armed. If every halt returns,
    os._exit ends it with the same status. Without a guard (`verify`, the usage text, a refusal before one is armed) `main`'s
    return or exception goes on unchanged."""
    global DIAG_ENTERED
    DIAG_ENTERED = True
    try:
        value, raised = main(argv), None
    except BaseException as error:
        if DIAG_GUARD is None:
            raise
        value, raised = None, error
    if DIAG_GUARD is None:
        return value
    if raised is None:
        code, why = diag_status(SystemExit(value)), 'the script returned'
    else:
        code, why = diag_status(raised), 'the script raised %s' % type(raised).__name__
    try:
        if DIAG.active:
            DIAG.at_exit()
        for stream in (sys.stdout, sys.stderr):
            try:
                stream.flush()
            except Exception:
                pass
    finally:
        DIAG_GUARD.close(code, why)
        os._exit(code)


def run(group, shard, directory):
    global DIAG
    if os.name == 'nt':
        DIAG = diag_leg(shard)
    table = load_table()
    if table.shards.get(shard, (None,))[0] != group:
        raise Refused('%s has no shard %r in group %r' % (TABLE_PATH, shard, group))
    args = table.shards[shard][1]
    blobs = bound_blobs()
    manifest_shapes(MANIFEST_PATH)
    try:
        os.mkdir(directory)
    except OSError as error:
        raise Refused('the record directory %r: %s' % (directory, error))
    directory = os.path.abspath(directory)
    DIAG.phase('the record header: Git and the three compiler questions')
    record = {
        'format': FORMAT,
        'group': group,
        'shard': shard,
        'args': args,
        'table': blobs[TABLE_PATH],
        'script': blobs[SCRIPT_PATH],
        'manifest': blobs[MANIFEST_PATH],
        'checkout': git('rev-parse', '--verify', '--quiet', 'HEAD'),
        'cwd': os.getcwd(),
        'directory': directory,
        'sep': os.sep,
        'github': dict((key, os.environ.get(key, '')) for key in GITHUB_KEYS),
        'runner': dict((key, os.environ.get(key, '')) for key in RUNNER_KEYS),
        'compiler': capture(COMPILER),
        'path_rustc': capture(['rustc', '-vV']),
        'cargo': capture(['cargo', '-V']),
        'steps': [],
    }
    compiler_facts(record['compiler']['stdout'], shard)
    DIAG.bind(record)

    def finish(status):
        DIAG.phase('finish: record.json')
        with open(os.path.join(directory, RECORD), 'w') as handle:
            json.dump(record, handle, indent=1, sort_keys=True)
            handle.write('\n')
        DIAG.finished(status)
        return status

    def step(entry):
        record['steps'].append(entry)
        if entry['exit'] != 0:
            print('partition: %s %s exited %d' % (shard, entry['name'], entry['exit']), flush=True)
        return entry['exit']

    DIAG.phase('metadata')
    print('partition: %s metadata: %s' % (shard, ' '.join(METADATA)), flush=True)
    code = stream(METADATA, os.path.join(directory, 'metadata.json'), os.path.join(directory, 'metadata.log'), False)
    if step({'name': 'metadata', 'argv': METADATA, 'exit': code, 'out': 'metadata.json', 'err': 'metadata.log',
             'sha256': [digest(os.path.join(directory, 'metadata.json')), digest(os.path.join(directory, 'metadata.log'))]}):
        return finish(code)
    DIAG.phase('build')
    print('partition: %s build: %s' % (shard, ' '.join(BUILD)), flush=True)
    code = stream(BUILD, os.path.join(directory, 'build.json'), os.path.join(directory, 'build.log'), True)
    if step({'name': 'build', 'argv': BUILD, 'exit': code, 'out': 'build.json', 'err': 'build.log',
             'sha256': [digest(os.path.join(directory, 'build.json')), digest(os.path.join(directory, 'build.log'))]}):
        return finish(code)
    DIAG.phase('inventory')
    with open(os.path.join(directory, 'metadata.json'), 'rb') as handle:
        metadata_text = handle.read().decode('utf-8', 'replace')
    with open(os.path.join(directory, 'build.json'), 'rb') as handle:
        build_text = handle.read().decode('utf-8', 'replace')
    units, compile_only, _ = inventory(metadata_text, build_text, record['cwd'], table.groups[group]['os'], shard)
    record['units'] = units
    record['compile_only'] = compile_only
    for unit in units:
        unit['observed'] = [['build', identity(unit['executable'])]]
    for index, unit in enumerate(units):
        for name, argv in unit_commands(unit, index, args, directory, os.sep):
            log = os.path.join(directory, 'u%d-%s.log' % (index, name))
            DIAG.phase('u%d %s' % (index, name))
            print('partition: %s %s %s: %s' % (shard, unit['source'], name, ' '.join(argv)), flush=True)
            unit['observed'].append(['before %s' % name, identity(unit['executable'])])
            code = tee(argv, log) if name == 'run' else into(argv, log)
            DIAG.phase('u%d %s: the identity after' % (index, name))
            unit['observed'].append(['after %s' % name, identity(unit['executable'])])
            DIAG.phase('u%d %s: the log digest' % (index, name))
            entry = {'name': name, 'unit': index, 'argv': argv, 'exit': code, 'log': os.path.basename(log), 'sha256': digest(log)}
            if name == 'run':
                results = os.path.join(directory, 'u%d-results.log' % index)
                if not os.path.isfile(results):
                    with open(results, 'wb'):
                        pass
                DIAG.phase('u%d run: the results digest' % index)
                entry['results_sha256'] = digest(results)
            if code != 0 and name != 'run':
                with open(log, 'rb') as handle:
                    sys.stdout.flush()
                    sys.stdout.buffer.write(handle.read())
                    sys.stdout.flush()
            DIAG.phase('u%d %s: the step' % (index, name))
            if step(entry):
                return finish(code)
    return finish(0)


def text_lines(path):
    with open(path, 'rb') as handle:
        data = handle.read()
    return [line.rstrip('\r') for line in ANSI.sub(b'', data).decode('utf-8', 'replace').split('\n')]


def sections(path, where):
    found = []
    for line in text_lines(path):
        running = RUNNING.fullmatch(line)
        if running:
            found.append((running.group(1).replace('\\', '/'), running.group(2), []))
        elif found:
            found[-1][2].append(line)
    if len(found) != 1:
        raise Refused('%s: %d `Running` lines, not exactly one harness' % (where, len(found)))
    return found[0]


def listing(body, where):
    names = []
    counted = None
    for line in body:
        if not line.strip():
            continue
        if counted is not None:
            raise Refused('%s: a line after the count: %r' % (where, line))
        item = LISTED.fullmatch(line)
        if item:
            names.append(item.group(1))
            continue
        count = COUNTED.fullmatch(line)
        if count:
            counted = (int(count.group(1)), int(count.group(2)))
            continue
        raise Refused('%s: not a libtest listing line: %r' % (where, line))
    if counted is None:
        raise Refused('%s: no `N tests, M benchmarks` line, so the listing is cut short or is not libtest\'s' % where)
    if counted != (len(names), 0):
        raise Refused('%s: lists %d tests and counts %d tests, %d benchmarks' % (where, len(names), counted[0], counted[1]))
    if len(set(names)) != len(names):
        raise Refused('%s: lists a name twice' % where)
    return names


def results(path, where):
    """The harness's own logfile: one `<result> <name>` line per test it ran or ignored."""
    outcomes = {}
    for line in text_lines(path):
        if not line.strip():
            continue
        head, _, name = line.rpartition(' ')
        if not head or not name:
            raise Refused('%s: an unreadable logfile line: %r' % (where, line))
        if head == 'ok':
            outcome = 'ok'
        elif head == 'ignored' or head.startswith('ignored: '):
            outcome = 'ignored'
        else:
            raise Refused('%s: %s did not pass: %r' % (where, name, line))
        if name in outcomes:
            raise Refused('%s: the logfile reports %s twice' % (where, name))
        outcomes[name] = outcome
    return outcomes


def sample(names):
    names = sorted(names)
    return '%d, e.g. %s' % (len(names), ', '.join(names[:5]))


class Leg(object):
    pass


def require(where, label, got, want):
    if got != want:
        raise Refused('%s: %s is %r, not %r' % (where, label, got, want))


def read_leg(path, shard, attempt, table, env, head, blobs):
    where = '%s (attempt %d)' % (shard, attempt)
    group, args = table.shards[shard]
    group_os = table.groups[group]['os']
    paths = flavour(group_os)
    with open(os.path.join(path, RECORD), 'rb') as handle:
        try:
            record = json.loads(handle.read().decode('utf-8'))
        except ValueError as error:
            raise Refused('%s: %s is not JSON: %s' % (where, RECORD, error))
    if not isinstance(record, dict):
        raise Refused('%s: %s is not an object' % (where, RECORD))
    require(where, 'the record format', record.get('format'), FORMAT)
    require(where, 'the group', record.get('group'), group)
    require(where, 'the shard', record.get('shard'), shard)
    require(where, 'the shard arguments', record.get('args'), args)
    require(where, 'the table blob', record.get('table'), blobs[TABLE_PATH])
    require(where, 'the script blob', record.get('script'), blobs[SCRIPT_PATH])
    require(where, 'the manifest blob', record.get('manifest'), blobs[MANIFEST_PATH])
    require(where, 'the checkout', record.get('checkout'), head)
    github = record.get('github') if isinstance(record.get('github'), dict) else {}
    require(where, 'the run', github.get('GITHUB_RUN_ID'), env['GITHUB_RUN_ID'])
    require(where, 'the run attempt', github.get('GITHUB_RUN_ATTEMPT'), str(attempt))
    require(where, 'the commit', github.get('GITHUB_SHA'), env['GITHUB_SHA'])
    require(where, 'the event', github.get('GITHUB_EVENT_NAME'), env['GITHUB_EVENT_NAME'])
    require(where, 'the repository', github.get('GITHUB_REPOSITORY'), env['GITHUB_REPOSITORY'])
    runner = record.get('runner') if isinstance(record.get('runner'), dict) else {}
    require(where, 'the runner OS', runner.get('RUNNER_OS'), group_os)
    require(where, 'the runner environment', runner.get('RUNNER_ENVIRONMENT'), 'github-hosted')
    compiler = record.get('compiler') if isinstance(record.get('compiler'), dict) else {}
    require(where, 'the compiler query', compiler.get('argv'), COMPILER)
    require(where, 'the compiler query\'s exit', compiler.get('exit'), 0)
    verbose = str(compiler.get('stdout'))
    facts = compiler_facts(verbose, where)
    require(where, 'the compiler\'s host', facts.get('host'), table.groups[group]['triple'])
    if table.groups[group]['release'] != 'any':
        require(where, 'the compiler\'s release', facts.get('release'), table.groups[group]['release'])
    cwd = str(record.get('cwd'))
    directory = str(record.get('directory'))
    sep = record.get('sep')
    if sep not in ('/', '\\'):
        raise Refused('%s: the record names no path separator' % where)

    def file_text(name):
        with open(os.path.join(path, name), 'rb') as handle:
            return handle.read().decode('utf-8', 'replace')

    steps = record.get('steps') if isinstance(record.get('steps'), list) else []
    if len(steps) < 2 or not all(isinstance(entry, dict) for entry in steps):
        raise Refused('%s: the record holds no metadata and build steps' % where)
    for entry, (name, argv, out, err) in zip(steps[:2], (('metadata', METADATA, 'metadata.json', 'metadata.log'),
                                                         ('build', BUILD, 'build.json', 'build.log'))):
        require(where, 'step %s' % name, entry.get('name'), name)
        require(where, 'the %s argv' % name, entry.get('argv'), argv)
        require(where, 'the %s exit' % name, entry.get('exit'), 0)
        require(where, 'the %s digests' % name, [digest(os.path.join(path, out)), digest(os.path.join(path, err))], entry.get('sha256'))
    units, compile_only, root = inventory(file_text('metadata.json'), file_text('build.json'), cwd, group_os, where)
    require(where, 'the package root', root, normal(paths, cwd))
    recorded = record.get('units') if isinstance(record.get('units'), list) else []
    require(where, 'the test units', [dict((k, u.get(k)) for k in ('kind', 'name', 'source', 'executable')) for u in recorded if isinstance(u, dict)],
            [dict((k, u[k]) for k in ('kind', 'name', 'source', 'executable')) for u in units])
    require(where, 'the compile-only units', [list(pair) for pair in record.get('compile_only') or []], [list(pair) for pair in compile_only])
    expected_steps = []
    for index, unit in enumerate(units):
        for name, argv in unit_commands(unit, index, args, directory, sep):
            expected_steps.append((index, name, argv))
    require(where, 'the unit commands', [(e.get('unit'), e.get('name'), e.get('argv')) for e in steps[2:]],
            [(i, n, a) for i, n, a in expected_steps])
    present = sorted(os.listdir(path))
    require(where, 'the record\'s files', present, unit_files(len(units)))
    for entry in steps[2:]:
        at = '%s unit %s %s' % (where, entry.get('unit'), entry.get('name'))
        require(at, 'the exit', entry.get('exit'), 0)
        require(at, 'the log', entry.get('log'), 'u%d-%s.log' % (entry['unit'], entry['name']))
        require(at, 'the log\'s digest', digest(os.path.join(path, entry['log'])), entry.get('sha256'))
        if entry['name'] == 'run':
            require(at, 'the logfile\'s digest', digest(os.path.join(path, 'u%d-results.log' % entry['unit'])), entry.get('results_sha256'))
    positives, skips = predicate(args, where)

    leg = Leg()
    leg.shard = shard
    leg.attempt = attempt
    leg.group = group
    leg.units = [(u['kind'], u['name'], u['source']) for u in units]
    leg.compile_only = compile_only
    leg.compiler = verbose
    leg.universe = {}
    leg.ignored = {}
    leg.selected = {}
    leg.passed = 0
    leg.lines = []
    for index, unit in enumerate(units):
        at = '%s %s' % (where, unit['source'])
        observed = recorded[index].get('observed') if isinstance(recorded[index], dict) else None
        kinds = [label for label, _ in observed or []]
        require(at, 'the executable observations', kinds,
                ['build'] + [edge + ' ' + step for step in STEPS for edge in ('before', 'after')])
        identities = set(json.dumps(value) for _, value in observed)
        if len(identities) != 1 or json.loads(next(iter(identities)))[1] is None:
            raise Refused('%s: the executable %s changed or vanished between the build and the last command: %r'
                          % (at, unit['executable'], observed))
        bodies = {}
        for step in STEPS:
            source, binary, body = sections(os.path.join(path, 'u%d-%s.log' % (index, step)), '%s %s' % (at, step))
            if not same_source(paths, unit['source'], source):
                raise Refused('%s %s: the harness source is %r, not %r' % (at, step, source, unit['source']))
            require('%s %s' % (at, step), 'the harness executable', normal(paths, binary, cwd), normal(paths, unit['executable']))
            bodies[step] = body
        universe = listing(bodies['list'], at + ' list')
        hidden = set(listing(bodies['ignored'], at + ' ignored'))
        if not hidden <= set(universe):
            raise Refused('%s: ignores tests it does not list: %s' % (at, sample(hidden - set(universe))))
        wanted = set(name for name in universe if selects(positives, skips, name))
        harness = set(listing(bodies['selected'], at + ' selected'))
        if harness != wanted:
            raise Refused(
                '%s: the harness selected %d tests and the table selects %d: only the harness: %s; only the table: %s'
                % (at, len(harness), len(wanted), sample(harness - wanted), sample(wanted - harness))
            )
        lines = [line for line in bodies['run'] if line.strip()]
        if lines and lines[0].startswith(DEPRECATION):
            lines = ([lines[0][len(DEPRECATION):]] if lines[0][len(DEPRECATION):].strip() else []) + lines[1:]
        header = HEADER.fullmatch(lines[0]) if lines else None
        if not header or int(header.group(1)) != len(wanted):
            raise Refused('%s run: does not open with its own `running %d tests` line' % (at, len(wanted)))
        outcomes = results(os.path.join(path, 'u%d-results.log' % index), at + ' logfile')
        if set(outcomes) != wanted:
            raise Refused('%s: the logfile reports %d names and the selection holds %d: only the logfile: %s; never reported: %s'
                          % (at, len(outcomes), len(wanted), sample(set(outcomes) - wanted), sample(wanted - set(outcomes))))
        wrong = sorted(name for name, outcome in outcomes.items() if (outcome == 'ignored') != (name in hidden))
        if wrong:
            raise Refused('%s: the logfile reports these as %s against the ignore listing: %s'
                          % (at, ', '.join(outcomes[name] for name in wrong[:5]), sample(wrong)))
        passed = len(wanted - hidden)
        expected = (passed, 0, len(wanted & hidden), 0, len(universe) - len(wanted))
        summaries = [SUMMARY.fullmatch(line) for line in lines]
        if not any(match and match.group(1) == 'ok' and tuple(int(v) for v in match.groups()[1:]) == expected for match in summaries):
            raise Refused('%s run: no `test result: ok.` line reads %d passed, 0 failed, %d ignored, 0 measured, %d filtered out'
                          % (at, expected[0], expected[2], expected[4]))
        leg.universe[unit['source']] = universe
        leg.ignored[unit['source']] = hidden
        leg.selected[unit['source']] = wanted
        leg.passed += passed
        leg.lines.append('  %s %-22s %5d of %5d listed: %5d passed, %4d ignored; executable %s'
                         % (shard, unit['source'], len(wanted), len(universe), passed, len(wanted & hidden),
                            json.loads(next(iter(identities)))[0][:16]))
    return leg


def scan(records, problems):
    found = {}
    try:
        status = os.lstat(records)
    except OSError as error:
        problems.append('the records directory %r: %s' % (records, error))
        return found
    if not stat.S_ISDIR(status.st_mode):
        problems.append('the records directory %r is not a directory' % records)
        return found
    for entry in sorted(os.listdir(records)):
        path = os.path.join(records, entry)
        artifact = ARTIFACT.fullmatch(entry)
        if not artifact or not stat.S_ISDIR(os.lstat(path).st_mode):
            problems.append('an unexpected entry among the records: %r' % entry)
            continue
        inner = sorted(os.listdir(path))
        if not all(stat.S_ISREG(os.lstat(os.path.join(path, name)).st_mode) for name in inner):
            problems.append('%s holds something other than regular files' % entry)
            continue
        found.setdefault(artifact.group(1), {})[int(artifact.group(2))] = path
    return found


def verify(records):
    table = load_table()
    env = dict((key, os.environ.get(key, '')) for key in GITHUB_KEYS)
    if env['GITHUB_EVENT_NAME'] not in table.lanes:
        raise Refused('the event %r has no lane in %s' % (env['GITHUB_EVENT_NAME'], TABLE_PATH))
    if not env['GITHUB_RUN_ATTEMPT'].isdigit() or not env['GITHUB_RUN_ID'].isdigit():
        raise Refused('GITHUB_RUN_ID %r or GITHUB_RUN_ATTEMPT %r is not a number' % (env['GITHUB_RUN_ID'], env['GITHUB_RUN_ATTEMPT']))
    attempt = int(env['GITHUB_RUN_ATTEMPT'])
    head = git('rev-parse', '--verify', '--quiet', 'HEAD')
    blobs = bound_blobs()
    manifest_shapes(MANIFEST_PATH)
    problems = []
    if head != env['GITHUB_SHA']:
        problems.append('this job checked out %s, not the run\'s commit %s' % (head, env['GITHUB_SHA']))
    lane = table.lanes[env['GITHUB_EVENT_NAME']]
    expected = [shard for group in lane for shard in table.order[group]]
    found = scan(records, problems)
    for shard in sorted(found):
        if shard not in expected:
            problems.append('a record from %r, which is not a shard of the %s lane %r' % (shard, env['GITHUB_EVENT_NAME'], expected))
    legs = {}
    for shard in expected:
        attempts = found.get(shard, {})
        others = sorted(n for n in attempts if n != attempt)
        if others:
            problems.append('%s: records from attempts %r, not this run attempt %d; only this attempt\'s are read' % (shard, others, attempt))
            continue
        if attempt not in attempts:
            problems.append('%s: no record from attempt %d, so the leg did not finish and upload in this attempt' % (shard, attempt))
            continue
        try:
            legs[shard] = read_leg(attempts[attempt], shard, attempt, table, env, head, blobs)
        except Refused as error:
            problems.append(str(error))
        except (OSError, KeyError, TypeError, AttributeError, ValueError, IndexError) as error:
            problems.append('%s (attempt %d): an unreadable record: %r' % (shard, attempt, error))

    print('partition: %s at %s, attempt %d: verifying %s' % (env['GITHUB_EVENT_NAME'], head, attempt, ', '.join(lane)))
    for group in lane:
        members = [legs[shard] for shard in table.order[group] if shard in legs]
        if len(members) != len(table.order[group]):
            problems.append('%s: not every leg is readable, so its coverage is not decided' % group)
            continue
        first = members[0]
        consistent = True
        for leg in members[1:]:
            if leg.units != first.units or leg.compile_only != first.compile_only:
                problems.append('%s: %s declared and built %r with compile-only %r, and %s %r with %r'
                                % (group, first.shard, first.units, first.compile_only, leg.shard, leg.units, leg.compile_only))
                consistent = False
                continue
            if leg.compiler != first.compiler:
                problems.append('%s: %s and %s ran different compilers' % (group, first.shard, leg.shard))
                consistent = False
            for _, _, source in first.units:
                if leg.universe[source] != first.universe[source]:
                    problems.append(
                        '%s %s: %s and %s listed different tests: only %s: %s; only %s: %s'
                        % (group, source, first.shard, leg.shard, first.shard,
                           sample(set(first.universe[source]) - set(leg.universe[source])),
                           leg.shard, sample(set(leg.universe[source]) - set(first.universe[source])))
                    )
                    consistent = False
                if leg.ignored[source] != first.ignored[source]:
                    problems.append('%s %s: %s and %s ignore different tests' % (group, source, first.shard, leg.shard))
                    consistent = False
        if not consistent:
            continue
        inventory_digest = hashlib.sha256()
        for kind, name, source in first.units:
            inventory_digest.update(('unit\t%s\t%s\t%s\n' % (kind, name, source)).encode('utf-8'))
            for test in sorted(first.universe[source]):
                inventory_digest.update(('%s\t%s\t%s\n' % (source, test, 'ignored' if test in first.ignored[source] else 'test')).encode('utf-8'))
        for kind, name in first.compile_only:
            inventory_digest.update(('compile-only\t%s\t%s\n' % (kind, name)).encode('utf-8'))
        print('partition: %s: %d legs, %d test units, compile-only %r, compiler %s, inventory %s' % (
            group, len(members), len(first.units), first.compile_only,
            compiler_facts(first.compiler, group)['release'],
            inventory_digest.hexdigest()))
        for leg in members:
            for line in leg.lines:
                print(line)
            if not any(leg.selected[source] for _, _, source in leg.units):
                print('partition: %s selects no test; recorded, with every listed test held by another leg' % leg.shard)
        for _, _, source in first.units:
            owners = {}
            for leg in members:
                for name in leg.selected[source]:
                    owners.setdefault(name, []).append(leg.shard)
            twice = set(name for name, held in owners.items() if len(held) > 1)
            never = set(first.universe[source]) - set(owners)
            if twice:
                problems.append('%s %s: selected by more than one leg: %s' % (group, source, sample(twice)))
            if never:
                problems.append('%s %s: selected by no leg: %s' % (group, source, sample(never)))
            print('partition: %s %s: %d listed, %d selected exactly once' % (group, source, len(first.universe[source]), len(owners) - len(twice)))
        passed = sum(leg.passed for leg in members)
        if passed < table.groups[group]['floor']:
            problems.append('%s: its legs report %d passing tests, below the floor of %d' % (group, passed, table.groups[group]['floor']))

    for problem in problems:
        print('partition: refused: %s' % problem, file=sys.stderr)
    if problems:
        return 1
    print('partition: every partitioned group ran every listed test exactly once')
    return 0


def main(argv):
    if os.name == 'nt' and not DIAG_ENTERED:
        return diag_main(main, argv)
    try:
        if len(argv) == 4 and argv[0] == 'run':
            return run(argv[1], argv[2], argv[3])
        if len(argv) == 2 and argv[0] == 'verify':
            return verify(argv[1])
        if len(argv) == 1 and argv[0] == 'diag-selftest' and os.name == 'nt':
            return diag_selftest_only()
    except Refused as error:
        print('partition: refused: %s' % error, file=sys.stderr)
        return 1
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
