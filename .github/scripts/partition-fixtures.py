#!/usr/bin/env python3
"""Fixtures for .github/scripts/partition.py, run by test-partition.sh.

Each case builds a throwaway Git repository holding the real partition.py and
a table -- the real partition.table with its floors lowered, or a labelled
variant of it -- runs `partition.py run` there for every leg of a lane against a
fake `cargo` and `rustc` on PATH, and then `partition.py verify` on the records,
mutated or not. The fake harness implements libtest's listing, filter, skip,
ignore and summary output over a synthetic suite whose names put the filter
substrings in unusual places, with a zero-test target, ignored tests and a
nested child's output in another leg's run. Every refusal is paired with the
legitimate shape nearest it, which must pass. No Cargo, compiler or network.
"""

import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(HERE, 'partition.py')
TABLE = os.path.join(HERE, 'partition.table')
PYTHON = sys.executable
RUN_ID = '4242'
REPOSITORY = 'sourcemaps/upstroke'

LIB = [
    'agent::proc::tests::one',
    'engine::tests::two',
    'engine::topology::recover::tests::three',
    'engine::topology::recover::tests::four',
    'engine::topology::recovery::tests::five',
    'engine::topology::tests::six',
    'my_engine::tests::seven',
    'x::engine::topology::recover::eight',
    'rundir::tests::nine',
    'tests::engine_free',
]
LIB_IGNORED = ['engine::topology::recover::tests::four', 'rundir::tests::nine']
CHILDREN = {'agent::proc::tests::one': ['engine::topology::recover::tests::four']}
PANICS = ['x::engine::topology::recover::eight']


def vv(release, host):
    return ('rustc %s (0123456789 2026-01-01)\nbinary: rustc\ncommit-hash: 0123456789abcdef\n'
            'commit-date: 2026-01-01\nhost: %s\nrelease: %s\nLLVM version: 21.1.0\n' % (release, host, release))


def suite(group, lib=None, ignored=None, children=None):
    lib = LIB if lib is None else lib
    if group == 'windows':
        sep, exe, main = '\\', '.exe', ['tests::a', 'tests::b', 'tests::c']
        compiler = vv('1.97.1', 'x86_64-pc-windows-msvc')
    else:
        sep, exe, main = '/', '', ['tests::a', 'tests::b']
        compiler = vv('1.99.0', 'aarch64-apple-darwin')
    path = lambda *parts: sep.join(parts)
    return {
        'vv': compiler,
        'targets': [
            {'source': path('src', 'lib.rs'), 'binary': path('target', 'debug', 'deps', 'upstroke-1a2b3c' + exe),
             'names': lib, 'ignored': LIB_IGNORED if ignored is None else ignored,
             'children': CHILDREN if children is None else children, 'panics': PANICS},
            {'source': path('src', 'main.rs'), 'binary': path('target', 'debug', 'deps', 'upstroke-4d5e6f' + exe),
             'names': main, 'ignored': [], 'children': {}, 'panics': []},
            {'source': path('examples', 'probe.rs'), 'binary': path('target', 'debug', 'examples', 'probe-7a8b9c' + exe),
             'names': [], 'ignored': [], 'children': {}, 'panics': []},
        ],
    }


FAKE = r'''
import json, os, sys
spec = json.load(open(os.environ['FAKE_SUITE']))
args = sys.argv[1:]
with open(os.environ['FAKE_LOG'], 'a') as log:
    log.write(json.dumps(args) + '\n')
def out(fd, text):
    os.write(fd, text.encode('utf-8'))
def plural(count, word):
    return '%d %s%s' % (count, word, '' if count == 1 else 's')
if args == ['-V']:
    out(1, 'cargo 1.97.1 (fake)\n'); sys.exit(0)
if args in (['rustc', '-q', '--lib', '--', '-vV'], ['-vV']):
    out(1, spec['vv']); sys.exit(0)
if args[:3] != ['test', '--all-targets', '--all-features']:
    out(2, 'fake cargo: unexpected %r\n' % args); sys.exit(2)
rest = args[3:]
fail = os.environ.get('FAKE_FAIL', '')
out(2, '\x1b[1m\x1b[92m   Compiling\x1b[0m upstroke v0.1.0 (/fake)\n')
out(2, '\x1b[1m\x1b[92m    Finished\x1b[0m `test` profile [unoptimized + debuginfo] target(s) in 0.01s\n')
if rest == ['--no-run']:
    for target in spec['targets']:
        out(2, '  \x1b[1m\x1b[92mExecutable\x1b[0m unittests %s (%s)\n' % (target['source'], target['binary']))
    sys.exit(101 if fail == 'build' else 0)
words = rest[1:]
listing = '--list' in words
only_ignored = '--ignored' in words
words = [word for word in words if word not in ('--list', '--ignored')]
positives, skips, index = [], [], 0
while index < len(words):
    if words[index] == '--skip':
        skips.append(words[index + 1]); index += 2
    else:
        positives.append(words[index]); index += 1
for target in spec['targets']:
    out(2, '\x1b[1m\x1b[92m     Running\x1b[0m unittests %s (%s)\n' % (target['source'], target['binary']))
    names = sorted(target['names'])
    ignored = set(target['ignored'])
    chosen = [n for n in names if (not positives or any(p in n for p in positives)) and not any(s in n for s in skips)]
    if listing:
        if only_ignored:
            chosen = [n for n in chosen if n in ignored]
        body = ''.join('%s: test\n' % n for n in chosen)
        out(1, body + ('\n' if chosen else '') + '%s, 0 benchmarks\n' % plural(len(chosen), 'test'))
        continue
    out(1, '\nrunning %s\n' % plural(len(chosen), 'test'))
    for name in chosen:
        mode = ' - should panic' if name in target['panics'] and name not in ignored else ''
        out(1, 'test %s%s ... %s\n' % (name, mode, 'ignored' if name in ignored else 'ok'))
        for child in target['children'].get(name, []):
            out(1, '\nrunning 1 test\ntest %s ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; '
                   '0 measured; %d filtered out; finished in 0.00s\n\n' % (child, len(names) - 1))
    passed = len([n for n in chosen if n not in ignored])
    out(1, '\ntest result: ok. %d passed; 0 failed; %d ignored; 0 measured; %d filtered out; '
           'finished in 0.01s\n\n' % (passed, len(chosen) - passed, len(names) - len(chosen)))
step = 'list' if listing else 'run'
sys.exit(101 if fail == step else 0)
'''


class Fixture(object):
    def __init__(self, root):
        self.root = root
        self.count = 0
        self.failures = []
        self.bin = os.path.join(root, 'bin')
        os.mkdir(self.bin)
        fake = os.path.join(root, 'fakecargo.py')
        with open(fake, 'w') as handle:
            handle.write(FAKE)
        for tool in ('cargo', 'rustc'):
            launcher = os.path.join(self.bin, tool)
            with open(launcher, 'w') as handle:
                handle.write('#!/bin/sh\nexec "%s" -I -B "%s" "$@"\n' % (PYTHON, fake))
            os.chmod(launcher, 0o755)

    def fresh(self, name):
        self.count += 1
        path = os.path.join(self.root, '%03d-%s' % (self.count, name))
        os.mkdir(path)
        return path

    def environment(self, extra):
        env = {
            'PATH': self.bin + os.pathsep + os.environ.get('PATH', '/usr/bin:/bin'),
            'HOME': self.root,
            'LANG': 'C.UTF-8',
            'GIT_CONFIG_NOSYSTEM': '1',
            'GIT_CONFIG_GLOBAL': os.devnull,
        }
        env.update(extra)
        return env


def run(argv, cwd, env):
    result = subprocess.run(argv, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return result.returncode, result.stdout.decode('utf-8', 'replace'), result.stderr.decode('utf-8', 'replace')


def git(cwd, env, *args):
    code, out, err = run(['git', '-c', 'user.name=fixture', '-c', 'user.email=fixture@upstroke.local'] + list(args), cwd, env)
    if code != 0:
        raise RuntimeError('git %s: %s' % (' '.join(args), err))
    return out.strip()


def real_table(floor='1'):
    with open(TABLE) as handle:
        text = handle.read()
    return re.sub(r'^(group \S+ \S+ \S+ \S+) [0-9]+$', r'\1 ' + floor, text, flags=re.M)


def with_shard(text, shard, args):
    replaced, count = re.subn(r'^(shard \S+ %s) .*$' % shard, r'\1 ' + args, text, flags=re.M)
    assert count == 1, shard
    return replaced


class Repo(object):
    """A committed checkout of the real partition.py beside a given table."""

    def __init__(self, fixture, name, table):
        self.fixture = fixture
        self.path = fixture.fresh(name)
        self.env = fixture.environment({})
        scripts = os.path.join(self.path, '.github', 'scripts')
        os.makedirs(scripts)
        shutil.copyfile(SCRIPT, os.path.join(scripts, 'partition.py'))
        with open(os.path.join(scripts, 'partition.table'), 'w') as handle:
            handle.write(table)
        git(self.path, self.env, 'init', '-q')
        git(self.path, self.env, 'add', '.github')
        git(self.path, self.env, 'commit', '-q', '-m', 'fixture')
        self.head = git(self.path, self.env, 'rev-parse', 'HEAD')

    def github(self, event, attempt, group=None):
        env = {
            'GITHUB_EVENT_NAME': event,
            'GITHUB_JOB': 'test',
            'GITHUB_REF': 'refs/heads/fixture',
            'GITHUB_REPOSITORY': REPOSITORY,
            'GITHUB_RUN_ATTEMPT': str(attempt),
            'GITHUB_RUN_ID': RUN_ID,
            'GITHUB_SHA': self.head,
            'GITHUB_WORKFLOW': 'CI',
        }
        if group is not None:
            env.update({
                'RUNNER_OS': {'windows': 'Windows', 'macos': 'macOS'}[group],
                'RUNNER_ARCH': {'windows': 'X64', 'macos': 'ARM64'}[group],
                'RUNNER_ENVIRONMENT': 'github-hosted',
                'RUNNER_NAME': 'GitHub Actions 1',
                'ImageOS': {'windows': 'win25', 'macos': 'macos15'}[group],
                'ImageVersion': '20261013.1',
            })
        return env

    def leg(self, event, group, shard, attempt, spec, records, fail=''):
        spec_path = os.path.join(self.path, 'suite-%s.json' % group)
        with open(spec_path, 'w') as handle:
            json.dump(spec, handle)
        log = os.path.join(self.path, 'cargo-argv-%s-%d.jsonl' % (shard, attempt))
        env = self.fixture.environment(self.github(event, attempt, group))
        env.update({'FAKE_SUITE': spec_path, 'FAKE_LOG': log, 'FAKE_FAIL': fail})
        code, out, err = run([PYTHON, '-I', '-B', '.github/scripts/partition.py', 'run', group, shard, 'test-partition'],
                             self.path, env)
        target = os.path.join(records, 'test-partition-%s-attempt-%d' % (shard, attempt))
        if os.path.isdir(os.path.join(self.path, 'test-partition')):
            shutil.move(os.path.join(self.path, 'test-partition'), target)
        return code, out, err, target, log

    def lane(self, event, records, specs, attempt=1, table=None):
        os.makedirs(records, exist_ok=True)
        groups, shards = lanes_of(table if table is not None else self.table_text())
        for group in groups[event]:
            for shard in shards[group]:
                code, out, err, _, _ = self.leg(event, group, shard, attempt, specs[group], records)
                if code != 0:
                    raise RuntimeError('the runner refused a legitimate leg %s: %s %s' % (shard, out, err))

    def table_text(self):
        with open(os.path.join(self.path, '.github', 'scripts', 'partition.table')) as handle:
            return handle.read()

    def verify(self, records, event, attempt=1, extra=None):
        env = self.fixture.environment(self.github(event, attempt))
        env.update(extra or {})
        return run([PYTHON, '-I', '-B', '.github/scripts/partition.py', 'verify', records], self.path, env)


def lanes_of(table):
    lanes, shards = {}, {}
    for line in table.splitlines():
        words = line.split()
        if words[:1] == ['lane']:
            lanes[words[1]] = words[2:]
        if words[:1] == ['shard']:
            shards.setdefault(words[1], []).append(words[2])
    return lanes, shards


def leg_dir(records, shard, attempt=1):
    return os.path.join(records, 'test-partition-%s-attempt-%d' % (shard, attempt))


def edit_record(records, shard, change, attempt=1):
    path = os.path.join(leg_dir(records, shard, attempt), 'record.json')
    with open(path) as handle:
        record = json.load(handle)
    change(record)
    with open(path, 'w') as handle:
        json.dump(record, handle)


def redigest(records, shard, step, attempt=1):
    import hashlib
    log = os.path.join(leg_dir(records, shard, attempt), step + '.log')
    with open(log, 'rb') as handle:
        value = hashlib.sha256(handle.read()).hexdigest()

    def change(record):
        for command in record['commands']:
            if command['name'] == step:
                command['sha256'] = value
    edit_record(records, shard, change, attempt)


def edit_log(records, shard, step, change, attempt=1, digest=True):
    log = os.path.join(leg_dir(records, shard, attempt), step + '.log')
    with open(log, 'rb') as handle:
        text = handle.read().decode('utf-8')
    with open(log, 'wb') as handle:
        handle.write(change(text).encode('utf-8'))
    if digest:
        redigest(records, shard, step, attempt)


def is_running(line):
    return 'Running' in line and '(' in line


def cut_section(text, fragment):
    lines = text.split('\n')
    start = [i for i, line in enumerate(lines) if is_running(line) and fragment in line]
    assert len(start) == 1, fragment
    end = [i for i, line in enumerate(lines) if i > start[0] and is_running(line)]
    stop = end[0] if end else len(lines)
    return '\n'.join(lines[:start[0]] + lines[stop:])


def section_lines(text, fragment):
    lines = text.split('\n')
    start = [i for i, line in enumerate(lines) if is_running(line) and fragment in line][0]
    end = [i for i, line in enumerate(lines) if i > start and is_running(line)]
    return lines, start, (end[0] if end else len(lines))


def replace_once(old, new):
    def change(text):
        assert text.count(old) == 1, old
        return text.replace(old, new)
    return change


def check(fixture, label, result, code, needle=None, stream='err'):
    got, out, err = result
    text = err if stream == 'err' else out
    ok = got == code and (needle is None or needle in text)
    print('%s %s' % ('ok    ' if ok else 'FAILED', label))
    if not ok:
        fixture.failures.append(label)
        print('  exit %d (wanted %d); wanted %r in std%s\n  stdout: %s\n  stderr: %s' % (got, code, needle, stream, out[-2000:], err[-2000:]))


def main():
    root = tempfile.mkdtemp(prefix='partition-fixtures-')
    try:
        fixture = Fixture(root)
        cases(fixture)
        if fixture.failures:
            print('partition fixtures: %d FAILED: %s' % (len(fixture.failures), ', '.join(fixture.failures)))
            return 1
        print('partition fixtures: all cases passed')
        return 0
    finally:
        shutil.rmtree(root, ignore_errors=True)


def cases(fixture):
    specs = {'windows': suite('windows'), 'macos': suite('macos')}
    table = real_table()
    repo = Repo(fixture, 'real-table', table)
    base = os.path.join(repo.path, 'base-queue')
    repo.lane('merge_group', base, specs)
    pr = os.path.join(repo.path, 'base-pr')
    repo.lane('pull_request', pr, specs)

    def copy(name):
        path = os.path.join(fixture.fresh(name), 'records')
        shutil.copytree(base, path, symlinks=True)
        return path

    # -- the legitimate shapes ------------------------------------------------
    check(fixture, 'P-QUEUE: the merge-queue lane, four Windows and three macOS legs', repo.verify(base, 'merge_group'),
          0, 'every partitioned group ran every listed test exactly once', 'out')
    check(fixture, 'P-PR: the pull-request lane partitions macOS alone', repo.verify(pr, 'pull_request'),
          0, 'every partitioned group ran every listed test exactly once', 'out')
    recorded = json.load(open(os.path.join(leg_dir(base, 'w2'), 'record.json')))
    with open(os.path.join(repo.path, 'cargo-argv-w2-1.jsonl')) as handle:
        received = [['cargo'] + json.loads(line) for line in handle]
    check(fixture, 'P-ARGV: the argv each command recorded is the argv the fake cargo received',
          (0 if [c['argv'] for c in recorded['commands']] == [r for r in received if r[1] == 'test'] else 1, '', ''), 0)
    with open(os.path.join(leg_dir(base, 'w2'), 'selected.log')) as handle:
        w2 = handle.read()
    check(fixture, 'P-UNUSUAL: `recovery::` is not `recover::`, so w2 holds it; `my_engine::` is w3\'s',
          (0 if 'engine::topology::recovery::tests::five: test' in w2
           and 'my_engine::tests::seven: test' in open(os.path.join(leg_dir(base, 'w3'), 'selected.log')).read() else 1, '', ''), 0)
    with open(os.path.join(leg_dir(base, 'w4'), 'run.log')) as handle:
        w4 = handle.read()
    check(fixture, 'P-CHILD: a nested child\'s run of an ignored helper prints inside another leg\'s section',
          (0 if 'test engine::topology::recover::tests::four ... ok' in w4 else 1, '', ''), 0)
    with open(os.path.join(leg_dir(base, 'w1'), 'run.log')) as handle:
        w1 = handle.read()
    check(fixture, 'P-SHOULD-PANIC: a `should_panic` test reports as `test <name> - should panic ... ok`',
          (0 if 'test x::engine::topology::recover::eight - should panic ... ok' in w1 else 1, '', ''), 0)

    path = copy('crlf')
    edit_log(path, 'w1', 'run', lambda text: text.replace('\n', '\r\n'))
    check(fixture, 'P-CRLF: a run log with CRLF line endings', repo.verify(path, 'merge_group'), 0)

    path = copy('superseded')
    code, out, err, _, _ = repo.leg('merge_group', 'windows', 'w2', 2, specs['windows'], path)
    check(fixture, 'P-SUPERSEDED: attempt 2 of w2 supersedes attempt 1 in an attempt-2 job',
          repo.verify(path, 'merge_group', attempt=2), 0, 'superseded by attempt 2', 'out')
    check(fixture, 'N-FUTURE-ATTEMPT: the same records read by an attempt-1 job',
          repo.verify(path, 'merge_group', attempt=1), 1, 'after this job\'s attempt 1')

    empty_lib = [name for name in LIB if 'engine::topology::recover::' not in name]
    empty = {'windows': suite('windows', lib=empty_lib, ignored=['rundir::tests::nine'], children={}),
             'macos': suite('macos', lib=empty_lib, ignored=['rundir::tests::nine'], children={})}
    path = os.path.join(fixture.fresh('empty-shard'), 'records')
    repo.lane('merge_group', path, empty)
    check(fixture, 'P-EMPTY-SHARD: w1 selects no test; coverage still exact, with a notice',
          repo.verify(path, 'merge_group'), 0, 'w1 selects no test at all', 'out')

    # -- the table does not partition ----------------------------------------
    omits = Repo(fixture, 'omission-table', with_shard(table, 'w4', '--skip engine:: --skip tests::'))
    path = os.path.join(omits.path, 'records')
    omits.lane('merge_group', path, specs)
    check(fixture, 'N-OMISSION: w4 also skips `tests::`, so no leg selects them',
          omits.verify(path, 'merge_group'), 1, 'selected by no leg')

    balanced = with_shard(with_shard(table, 'w1', 'engine::topology::'), 'w4',
                          '--skip engine:: --skip rundir:: --skip agent::proc::')
    dup = Repo(fixture, 'duplicate-and-omission-table', balanced)
    path = os.path.join(dup.path, 'records')
    dup.lane('merge_group', path, specs)
    sizes = sum(len(open(os.path.join(leg_dir(path, s), 'selected.log')).read().split(': test')) - 1
                for s in ('w1', 'w2', 'w3', 'w4'))
    listed = len(open(os.path.join(leg_dir(path, 'w1'), 'list.log')).read().split(': test')) - 1
    check(fixture, 'N-DUP-OMIT (control): the duplicate and the omission balance, so every count still matches',
          (0 if sizes == listed else 1, '', ''), 0)
    check(fixture, 'N-DUP-OMIT: w1 widened to all of topology and w4 drops two, the same number',
          dup.verify(path, 'merge_group'), 1, 'selected by more than one leg')
    check(fixture, 'N-DUP-OMIT: the omission half is named too',
          dup.verify(path, 'merge_group'), 1, 'selected by no leg')

    floor = Repo(fixture, 'floor-table', real_table('1000'))
    path = os.path.join(floor.path, 'records')
    floor.lane('merge_group', path, specs)
    check(fixture, 'N-FLOOR: the group passes fewer tests than its floor', floor.verify(path, 'merge_group'), 1, 'below the floor')

    bad = Repo(fixture, 'unreadable-table', table.replace('shard windows w4 --skip engine::', 'shard windows w4 --exact engine::'))
    check(fixture, 'N-TABLE-FLAG: a table argument that is not a filter or `--skip <filter>`',
          bad.verify(os.path.join(bad.path, 'none'), 'merge_group'), 1, 'is neither a name filter')

    # -- the selection is not the table's -------------------------------------
    path = copy('argv')
    edit_record(path, 'w2', lambda r: r['commands'][4]['argv'].__setitem__(slice(None), r['commands'][4]['argv'][:-2]))
    check(fixture, 'N-ARGV: w2\'s run argv lost its `--skip`', repo.verify(path, 'merge_group'), 1, 'the run argv')

    path = copy('harness-selection')
    edit_log(path, 'w2', 'selected', replace_once('engine::topology::tests::six: test\n\n2 tests',
                                                  'engine::topology::tests::six: test\nmy_engine::tests::seven: test\n\n3 tests'))
    check(fixture, 'N-HARNESS: w2\'s harness listed a test its predicate does not select',
          repo.verify(path, 'merge_group'), 1, 'the harness selected 3 tests and the table selects 2')

    # -- discovery differs ------------------------------------------------------
    path = copy('same-count')
    edit_log(path, 'w3', 'list', replace_once('agent::proc::tests::one: test', 'agent::proc::tests::uno: test'))
    check(fixture, 'N-SAME-COUNT: w3 listed the same number of tests under another name',
          repo.verify(path, 'merge_group'), 1, 'listed different tests')

    path = copy('conditional')
    edit_log(path, 'w3', 'list', replace_once('agent::proc::tests::one: test\n', ''))
    edit_log(path, 'w3', 'list', replace_once('\n10 tests, 0 benchmarks', '\n9 tests, 0 benchmarks'))
    edit_log(path, 'w3', 'run', replace_once('0 measured; 8 filtered out', '0 measured; 7 filtered out'))
    check(fixture, 'N-CFG: w3 compiled one test fewer, and its own run agrees', repo.verify(path, 'merge_group'), 1,
          'listed different tests')

    path = copy('ignored-state')
    edit_log(path, 'w3', 'ignored', replace_once('engine::topology::recover::tests::four: test\n',
                                                 'agent::proc::tests::one: test\nengine::topology::recover::tests::four: test\n'))
    edit_log(path, 'w3', 'ignored', replace_once('\n2 tests, 0 benchmarks', '\n3 tests, 0 benchmarks'))
    check(fixture, 'N-IGNORED: w3 ignores a test the other legs run', repo.verify(path, 'merge_group'), 1, 'ignore different tests')

    path = copy('ignored-count')
    edit_log(path, 'w1', 'run', replace_once('test engine::topology::recover::tests::three ... ok',
                                             'test engine::topology::recover::tests::three ... ignored'))
    edit_log(path, 'w1', 'run', replace_once('test result: ok. 2 passed; 0 failed; 1 ignored',
                                             'test result: ok. 1 passed; 0 failed; 2 ignored'))
    check(fixture, 'N-IGNORED-RUN: w1 reports a listed test as ignored', repo.verify(path, 'merge_group'), 1, 'ignored is 2, not 1')

    # -- a target, a summary or a run is missing ---------------------------------
    path = copy('missing-probe')
    edit_log(path, 'w2', 'run', lambda text: cut_section(text, 'probe.rs'))
    check(fixture, 'N-MISSING-PROBE: w2\'s run omitted the probe target', repo.verify(path, 'merge_group'), 1, 'the run command ran')

    path = copy('missing-main')
    edit_log(path, 'w4', 'run', lambda text: cut_section(text, 'main.rs'))
    check(fixture, 'N-MISSING-MAIN: w4\'s run omitted the binary target', repo.verify(path, 'merge_group'), 1, 'the run command ran')

    path = copy('missing-target-everywhere')
    for step in ('list', 'ignored', 'selected', 'run'):
        edit_log(path, 'm2', step, lambda text: cut_section(text, 'probe.rs'))
    check(fixture, 'N-TARGET-SET: m2 built and ran no probe target at all', repo.verify(path, 'merge_group'), 1, 'm2 built')

    def child_only(text):
        lines, start, stop = section_lines(text, 'lib.rs')
        child = [i for i in range(start, stop) if lines[i].startswith('test result: ok. 1 passed')]
        assert len(child) == 1
        return '\n'.join(lines[:child[0] + 1] + lines[stop:])
    path = copy('child-only-summary')
    edit_log(path, 'w4', 'run', child_only)
    check(fixture, 'N-CHILD-ONLY: w4\'s library section ends at a child\'s summary', repo.verify(path, 'merge_group'), 1,
          'tests accounted for is 1, not 3')

    def truncate(text):
        lines, start, stop = section_lines(text, 'lib.rs')
        return '\n'.join(lines[:start + 4])
    path = copy('truncated')
    edit_log(path, 'w1', 'run', truncate)
    check(fixture, 'N-TRUNCATED: w1\'s run log stops inside the library section', repo.verify(path, 'merge_group'), 1,
          'the run command ran')

    path = copy('incomplete-upload')
    edit_log(path, 'w1', 'run', truncate, digest=False)
    check(fixture, 'N-INCOMPLETE: w1\'s run log is short of the digest its leg recorded', repo.verify(path, 'merge_group'), 1,
          'the run log\'s digest')

    path = copy('failed-run')
    edit_log(path, 'w2', 'run', replace_once('test engine::topology::tests::six ... ok', 'test engine::topology::tests::six ... FAILED'))
    edit_log(path, 'w2', 'run', replace_once('test result: ok. 2 passed; 0 failed', 'test result: FAILED. 1 passed; 1 failed'))
    check(fixture, 'N-FAILED: w2 reports a failure', repo.verify(path, 'merge_group'), 1, 'result is \'FAILED\'')

    path = copy('failed-exit')
    edit_record(path, 'w2', lambda r: r['commands'][4].__setitem__('exit', 101))
    check(fixture, 'N-EXIT: w2\'s run exited 101', repo.verify(path, 'merge_group'), 1, 'the run exit is 101')

    path = copy('missing-leg')
    shutil.rmtree(leg_dir(path, 'w3'))
    check(fixture, 'N-MISSING-LEG: w3 uploaded nothing (failed, cancelled or skipped)', repo.verify(path, 'merge_group'), 1,
          'w3: no record')

    path = copy('not-libtest')
    edit_log(path, 'w1', 'list', replace_once('tests::engine_free: test\n', 'tests::engine_free: test\nhello from a custom harness\n'))
    check(fixture, 'N-FORMAT: a listing line that is not libtest\'s', repo.verify(path, 'merge_group'), 1, 'not a libtest listing line')

    # -- provenance --------------------------------------------------------------
    path = copy('checkout')
    edit_record(path, 'w1', lambda r: r.__setitem__('checkout', '0' * 40))
    check(fixture, 'N-CHECKOUT: w1 recorded another checkout', repo.verify(path, 'merge_group'), 1, 'the checkout')

    path = copy('run-id')
    edit_record(path, 'm1', lambda r: r['github'].__setitem__('GITHUB_RUN_ID', '4243'))
    check(fixture, 'N-RUN: m1 recorded another run', repo.verify(path, 'merge_group'), 1, 'the run is')

    path = copy('attempt')
    edit_record(path, 'm1', lambda r: r['github'].__setitem__('GITHUB_RUN_ATTEMPT', '2'))
    check(fixture, 'N-ATTEMPT: m1\'s record and its artifact name disagree on the attempt',
          repo.verify(path, 'merge_group', attempt=2), 1, 'the run attempt')

    path = copy('event')
    edit_record(path, 'w4', lambda r: r['github'].__setitem__('GITHUB_EVENT_NAME', 'pull_request'))
    check(fixture, 'N-EVENT: w4 recorded another event', repo.verify(path, 'merge_group'), 1, 'the event')

    path = copy('platform')
    edit_record(path, 'w3', lambda r: r['runner'].__setitem__('RUNNER_OS', 'Linux'))
    check(fixture, 'N-PLATFORM: w3 ran on Linux', repo.verify(path, 'merge_group'), 1, 'the runner OS')

    path = copy('self-hosted')
    edit_record(path, 'w3', lambda r: r['runner'].__setitem__('RUNNER_ENVIRONMENT', 'self-hosted'))
    check(fixture, 'N-SELF-HOSTED: w3 ran on a self-hosted runner', repo.verify(path, 'merge_group'), 1, 'the runner environment')

    path = copy('host')
    edit_record(path, 'm3', lambda r: r['compiler'].__setitem__('stdout', vv('1.99.0', 'x86_64-apple-darwin')))
    check(fixture, 'N-HOST: m3\'s compiler targets another host', repo.verify(path, 'merge_group'), 1, 'the compiler\'s host')

    path = copy('release')
    edit_record(path, 'w1', lambda r: r['compiler'].__setitem__('stdout', vv('1.98.0', 'x86_64-pc-windows-msvc')))
    check(fixture, 'N-RELEASE: w1 ran on a compiler other than the image\'s', repo.verify(path, 'merge_group'), 1,
          'the compiler\'s release')

    path = copy('compilers')
    edit_record(path, 'm2', lambda r: r['compiler'].__setitem__('stdout', r['compiler']['stdout'].replace('0123456789abcdef', 'fedcba9876543210')))
    check(fixture, 'N-COMPILERS: m1 and m2 ran different builds of one release', repo.verify(path, 'merge_group'), 1,
          'ran different compilers')

    path = copy('table-blob')
    edit_record(path, 'w2', lambda r: r.__setitem__('table', '1' * 40))
    check(fixture, 'N-TABLE: w2 ran another table', repo.verify(path, 'merge_group'), 1, 'the table blob')

    check(fixture, 'N-HEAD: the run\'s commit is not this checkout',
          repo.verify(base, 'merge_group', extra={'GITHUB_SHA': '2' * 40}), 1, 'not the run\'s commit')
    check(fixture, 'N-LANE: an event with no lane', repo.verify(base, 'workflow_dispatch'), 1, 'has no lane')

    # -- the artifacts themselves -----------------------------------------------
    path = copy('extra-leg')
    shutil.copytree(leg_dir(path, 'w4'), leg_dir(path, 'w5'))
    check(fixture, 'N-EXTRA: a record from a shard the lane does not run', repo.verify(path, 'merge_group'), 1, 'not a shard')

    check(fixture, 'N-WRONG-LANE: the queue\'s Windows records read on the pull-request lane',
          repo.verify(base, 'pull_request'), 1, 'not a shard of the pull_request lane')

    path = copy('stray-file')
    with open(os.path.join(path, 'stray.txt'), 'w') as handle:
        handle.write('x')
    check(fixture, 'N-STRAY: a file among the records', repo.verify(path, 'merge_group'), 1, 'an unexpected entry')

    path = copy('extra-file')
    with open(os.path.join(leg_dir(path, 'w1'), 'extra.log'), 'w') as handle:
        handle.write('x')
    check(fixture, 'N-EXTRA-FILE: a sixth log in w1\'s record', repo.verify(path, 'merge_group'), 1, 'not exactly')

    path = copy('symlink')
    target = os.path.join(leg_dir(path, 'w1'), 'run.log')
    os.remove(target)
    os.symlink(os.path.join(leg_dir(path, 'w2'), 'run.log'), target)
    check(fixture, 'N-LINK: w1\'s run log is a link', repo.verify(path, 'merge_group'), 1, 'other than regular files')

    path = copy('linked-leg')
    shutil.rmtree(leg_dir(path, 'w3'))
    os.symlink(leg_dir(base, 'w3'), leg_dir(path, 'w3'))
    check(fixture, 'N-LINKED-DIR: w3\'s record is a link to a directory elsewhere', repo.verify(path, 'merge_group'), 1,
          'an unexpected entry')

    empty_dir = os.path.join(fixture.fresh('no-records'), 'records')
    os.mkdir(empty_dir)
    check(fixture, 'N-EMPTY: no record at all', repo.verify(empty_dir, 'merge_group'), 1, 'no record')
    check(fixture, 'N-NO-DIRECTORY: the records directory is missing', repo.verify(empty_dir + '-absent', 'merge_group'), 1,
          'the records directory')

    # -- the runner ---------------------------------------------------------------
    scratch = os.path.join(fixture.fresh('runner'), 'records')
    os.mkdir(scratch)
    code, out, err, _, _ = repo.leg('merge_group', 'windows', 'm1', 1, specs['windows'], scratch)
    check(fixture, 'R-SHARD: the runner refuses a shard of another group', (code, out, err), 1, 'has no shard \'m1\'')
    os.mkdir(os.path.join(repo.path, 'test-partition'))
    code, out, err, target, _ = repo.leg('merge_group', 'windows', 'w1', 1, specs['windows'], scratch)
    check(fixture, 'R-EXISTS: the runner refuses an existing record directory', (code, out, err), 1, 'the record directory')
    os.rmdir(target)
    code, out, err, target, _ = repo.leg('merge_group', 'windows', 'w1', 1, specs['windows'], scratch, fail='run')
    failed = json.load(open(os.path.join(target, 'record.json')))
    check(fixture, 'R-FAIL: a failing run stops the runner with its exit status, recorded',
          (code if failed['commands'][-1]['exit'] == 101 else -1, out, err), 101)
    shutil.rmtree(target)
    with open(os.path.join(repo.path, '.github', 'scripts', 'partition.table'), 'a') as handle:
        handle.write('# a local edit\n')
    code, out, err, _, _ = repo.leg('merge_group', 'windows', 'w1', 1, specs['windows'], scratch)
    check(fixture, 'R-DIRTY: the runner refuses a table that is not the checkout\'s', (code, out, err), 1,
          'is not the checkout\'s')
    git(repo.path, repo.env, 'checkout', '--', '.github/scripts/partition.table')


if __name__ == '__main__':
    sys.exit(main())
