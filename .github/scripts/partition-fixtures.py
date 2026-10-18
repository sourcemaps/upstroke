#!/usr/bin/env python3
"""Fixtures for .github/scripts/partition.py, run by test-partition.sh.

Each case builds a throwaway Git repository holding the real partition.py, a
small Cargo.toml and a table -- the real partition.table with its floors
lowered, or a labelled variant of it -- runs `partition.py run` there for
every leg of a lane against a fake `cargo` and `rustc` on PATH, and then
`partition.py verify` on the records, mutated or not. The fake Cargo answers
`metadata`, builds fake executables and reports them as JSON artifacts, and
runs one target per `cargo test <selector>` with libtest's listing, filter,
skip, ignore, summary and `--logfile` formats (its deprecation warning included) over a synthetic suite whose
names put the filter substrings in unusual places, with a zero-test target,
ignored tests (one with a reason), a `should_panic` test and a nested child's
output in another leg's run. Its hooks omit a target, skip a declared unit in
the build, replace or re-touch an executable mid-leg, fail a step, or make the
compiler query fail, answer nothing or answer without a release. Every
refusal is paired with the legitimate shape nearest it, which must pass. The
child environments are built from scratch here (PATH, HOME, LANG and named
fixture values); a canary variable is set and must appear in no record. No
Cargo, compiler or network.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(HERE, 'partition.py')
TABLE = os.path.join(HERE, 'partition.table')
PYTHON = sys.executable
RUN_ID = '4242'
REPOSITORY = 'sourcemaps/upstroke'
CANARY = 'canary-value-0f3a9c-must-never-be-recorded'
MANIFEST = '[package]\nname = "upstroke"\nversion = "0.1.0"\nedition = "2024"\n'
CUSTOM = MANIFEST + '\n[[test]]\nname = "custom"\npath = "tests/custom.rs"\nharness = false\n'

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
REASONS = {'engine::topology::recover::tests::four': 'run only as a nested child'}
CHILDREN = {'agent::proc::tests::one': ['engine::topology::recover::tests::four']}
PANICS = ['x::engine::topology::recover::eight']


def vv(release, host):
    return ('rustc %s (0123456789 2026-01-01)\nbinary: rustc\ncommit-hash: 0123456789abcdef\n'
            'commit-date: 2026-01-01\nhost: %s\nrelease: %s\nLLVM version: 21.1.0\n' % (release, host, release))


def suite(group, lib=None, ignored=None, children=None, extra=False, bench=False, build_script=False):
    lib = LIB if lib is None else lib
    windows = group == 'windows'
    exe = '.exe' if windows else ''
    main = ['tests::a', 'tests::b', 'tests::c'] if windows else ['tests::a', 'tests::b']
    targets = [
        {'kind': 'lib', 'name': 'upstroke', 'source': 'src/lib.rs', 'exe': 'target/debug/deps/upstroke-1a2b3c' + exe,
         'names': lib, 'ignored': LIB_IGNORED if ignored is None else ignored,
         'children': CHILDREN if children is None else children, 'panics': PANICS},
        {'kind': 'bin', 'name': 'upstroke', 'source': 'src/main.rs', 'exe': 'target/debug/deps/upstroke-4d5e6f' + exe,
         'names': main, 'ignored': [], 'children': {}, 'panics': []},
        {'kind': 'example', 'name': 'probe', 'source': 'examples/probe.rs', 'exe': 'target/debug/examples/probe-7a8b9c' + exe,
         'names': [], 'ignored': [], 'children': {}, 'panics': []},
    ]
    if extra:
        targets.append({'kind': 'test', 'name': 'fresh', 'source': 'tests/fresh.rs', 'exe': 'target/debug/deps/fresh-0d1e2f' + exe,
                        'names': ['fresh_engine_test', 'engine::fresh'], 'ignored': [], 'children': {}, 'panics': []})
    if bench:
        targets.append({'kind': 'bench', 'name': 'speed', 'source': 'benches/speed.rs', 'exe': 'target/debug/deps/speed-9f8e7d' + exe,
                        'names': ['speed::case'], 'ignored': [], 'children': {}, 'panics': []})
    return {'vv': vv('1.97.1', 'x86_64-pc-windows-msvc') if windows else vv('1.99.0', 'aarch64-apple-darwin'),
            'windows': windows, 'targets': targets, 'build_script': build_script}


FAKE = r'''
import json, os, sys, time
spec = json.load(open(os.environ['FAKE_SUITE']))
root = os.getcwd()
args = sys.argv[1:]
with open(os.environ['FAKE_LOG'], 'a') as log:
    log.write(json.dumps(args) + '\n')
def out(fd, text):
    os.write(fd, text.encode('utf-8'))
def plural(count, word):
    return '%d %s%s' % (count, word, '' if count == 1 else 's')
def shown(path):
    return path.replace('/', '\\') if spec['windows'] else path
fail = os.environ.get('FAKE_FAIL', '')
silent = os.environ.get('FAKE_SILENT', '')
if args == ['-V']:
    out(1, 'cargo 1.97.1 (fake)\n'); sys.exit(0)
query = os.environ.get('FAKE_QUERY', '')
if args == ['rustc', '-q', '--lib', '--', '--version', '--verbose']:
    if query == 'fail':
        out(2, 'error: could not compile `upstroke` (lib)\n'); sys.exit(101)
    if query == 'empty':
        sys.exit(0)
    if query == 'no-release':
        out(1, ''.join(line + '\n' for line in spec['vv'].splitlines() if not line.startswith('release: '))); sys.exit(0)
    out(1, spec['vv']); sys.exit(0)
if args == ['-vV']:
    out(1, spec['vv']); sys.exit(0)
if args == ['metadata', '--format-version', '1', '--no-deps']:
    targets = [{'kind': [t['kind']], 'crate_types': ['lib' if t['kind'] == 'lib' else 'bin'], 'name': t['name'],
                'src_path': os.path.join(root, t['source']), 'test': t['kind'] != 'example'} for t in spec['targets']]
    if spec.get('build_script'):
        targets.append({'kind': ['custom-build'], 'crate_types': ['bin'], 'name': 'build-script-build',
                        'src_path': os.path.join(root, 'build.rs'), 'test': False})
    out(1, json.dumps({'packages': [{'name': 'upstroke', 'manifest_path': os.path.join(root, 'Cargo.toml'), 'targets': targets}],
                       'version': 1}) + '\n')
    sys.exit(0)
out(2, '\x1b[1m\x1b[92m   Compiling\x1b[0m upstroke v0.1.0 (/fake)\n')
out(2, '\x1b[1m\x1b[92m    Finished\x1b[0m `test` profile [unoptimized + debuginfo] target(s) in 0.01s\n')
if args == ['test', '--all-targets', '--all-features', '--no-run', '--message-format=json-render-diagnostics']:
    manifest = os.path.join(root, 'Cargo.toml')
    out(1, json.dumps({'reason': 'compiler-artifact', 'manifest_path': manifest, 'target': {'kind': ['lib'], 'name': 'upstroke',
        'src_path': os.path.join(root, 'src/lib.rs')}, 'profile': {'test': False}, 'executable': None, 'fresh': False}) + '\n')
    if spec.get('build_script') and os.environ.get('FAKE_BUILD_SKIP') != 'build-script-build':
        out(1, json.dumps({'reason': 'compiler-artifact', 'manifest_path': manifest, 'target': {'kind': ['custom-build'],
            'name': 'build-script-build', 'src_path': os.path.join(root, 'build.rs')}, 'profile': {'test': False},
            'executable': None, 'fresh': False}) + '\n')
    for t in spec['targets']:
        if t['name'] == os.environ.get('FAKE_BUILD_SKIP'):
            continue
        path = os.path.join(root, t['exe'])
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, 'w') as handle:
            handle.write('fake executable %s %s\n' % (t['kind'], t['name']))
        out(1, json.dumps({'reason': 'compiler-artifact', 'manifest_path': manifest, 'target': {'kind': [t['kind']], 'name': t['name'],
            'src_path': os.path.join(root, t['source'])}, 'profile': {'test': True}, 'executable': path, 'fresh': False}) + '\n')
        out(2, '  \x1b[1m\x1b[92mExecutable\x1b[0m unittests %s (%s)\n' % (shown(t['source']), shown(t['exe'])))
    out(1, json.dumps({'reason': 'build-finished', 'success': fail != 'build'}) + '\n')
    sys.exit(101 if fail == 'build' else 0)
if args[:2] != ['test', '--all-features']:
    out(2, 'fake cargo: unexpected %r\n' % args); sys.exit(2)
at = args.index('--')
selector, words = args[2:at], args[at + 1:]
kind = 'lib' if selector == ['--lib'] else selector[0][2:]
name = None if kind == 'lib' else selector[1]
target = [t for t in spec['targets'] if t['kind'] == kind and (name is None or t['name'] == name)][0]
listing = '--list' in words
only_ignored = '--ignored' in words
logfile = None
if '--logfile' in words:
    logfile = words[words.index('--logfile') + 1]
    words = words[:words.index('--logfile')] + words[words.index('--logfile') + 2:]
words = [w for w in words if w not in ('--list', '--ignored')]
positives, skips, index = [], [], 0
while index < len(words):
    if words[index] == '--skip':
        skips.append(words[index + 1]); index += 2
    else:
        positives.append(words[index]); index += 1
step = 'list' if listing else 'run'
exe_path = os.path.join(root, target['exe'])
if not listing and os.environ.get('FAKE_REPLACE') == target['name']:
    with open(exe_path, 'w') as handle:
        handle.write('a different executable at the same path\n')
if not listing and os.environ.get('FAKE_RECOMPILE') == target['name']:
    os.utime(exe_path, ns=(time.time_ns(), time.time_ns() + 1000000000))
if target['name'] == silent:
    sys.exit(0)
prefix = '' if target['kind'] == 'test' else 'unittests '
out(2, '\x1b[1m\x1b[92m     Running\x1b[0m %s%s (%s)\n' % (prefix, shown(target['source']), shown(target['exe'])))
names = sorted(target['names'])
ignored = set(target['ignored'])
chosen = [n for n in names if (not positives or any(p in n for p in positives)) and not any(s in n for s in skips)]
if listing:
    if only_ignored:
        chosen = [n for n in chosen if n in ignored]
    body = ''.join('%s: test\n' % n for n in chosen)
    out(1, body + ('\n' if chosen else '') + '%s, 0 benchmarks\n' % plural(len(chosen), 'test'))
    sys.exit(101 if fail == step else 0)
results = []
if logfile:
    out(2, 'warning: `--logfile` is deprecated')
out(1, '\nrunning %s\n' % plural(len(chosen), 'test'))
for test in chosen:
    mode = ' - should panic' if test in target['panics'] and test not in ignored else ''
    out(1, 'test %s%s ... %s\n' % (test, mode, 'ignored' if test in ignored else 'ok'))
    if test in ignored:
        reason = spec.get('reasons', {}).get(test)
        results.append(('ignored: %s' % reason if reason else 'ignored') + ' ' + test)
    else:
        results.append('ok ' + test)
    for child in target['children'].get(test, []):
        out(1, '\nrunning 1 test\ntest %s ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; '
               '0 measured; %d filtered out; finished in 0.00s\n\n' % (child, len(names) - 1))
passed = len([n for n in chosen if n not in ignored])
out(1, '\ntest result: ok. %d passed; 0 failed; %d ignored; 0 measured; %d filtered out; '
       'finished in 0.01s\n\n' % (passed, len(chosen) - passed, len(names) - len(chosen)))
if logfile:
    with open(logfile, 'w') as handle:
        handle.write(''.join(line + '\n' for line in results))
sys.exit(101 if fail == step else 0)
'''


class Fixture(object):
    def __init__(self, root):
        self.root = root
        self.count = 0
        self.failures = []
        self.outputs = []
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
            'PARTITION_FIXTURE_CANARY_SECRET': CANARY,
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
    """A committed checkout of the real partition.py beside a given table and manifest."""

    def __init__(self, fixture, name, table, manifest=MANIFEST):
        self.fixture = fixture
        self.path = fixture.fresh(name)
        self.env = fixture.environment({})
        scripts = os.path.join(self.path, '.github', 'scripts')
        os.makedirs(scripts)
        shutil.copyfile(SCRIPT, os.path.join(scripts, 'partition.py'))
        with open(os.path.join(scripts, 'partition.table'), 'w') as handle:
            handle.write(table)
        with open(os.path.join(self.path, 'Cargo.toml'), 'w') as handle:
            handle.write(manifest)
        with open(os.path.join(self.path, '.gitignore'), 'w') as handle:
            handle.write('/target\n/test-partition\n/records*\n/suite-*.json\n/cargo-argv-*\n')
        git(self.path, self.env, 'init', '-q')
        git(self.path, self.env, 'add', '.github', 'Cargo.toml', '.gitignore')
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

    def leg(self, event, group, shard, attempt, spec, records, hooks=None):
        spec = dict(spec, reasons=REASONS)
        spec_path = os.path.join(self.path, 'suite-%s.json' % group)
        with open(spec_path, 'w') as handle:
            json.dump(spec, handle)
        log = os.path.join(self.path, 'cargo-argv-%s-%d.jsonl' % (shard, attempt))
        env = self.fixture.environment(self.github(event, attempt, group))
        env.update({'FAKE_SUITE': spec_path, 'FAKE_LOG': log})
        env.update(hooks or {})
        code, out, err = run([PYTHON, '-I', '-B', '.github/scripts/partition.py', 'run', group, shard, 'test-partition'],
                             self.path, env)
        target = os.path.join(records, 'test-partition-%s-attempt-%d' % (shard, attempt))
        if os.path.isdir(os.path.join(self.path, 'test-partition')):
            shutil.move(os.path.join(self.path, 'test-partition'), target)
        self.fixture.outputs.append(out + err)
        return code, out, err, target, log

    def lane(self, event, records, specs, attempt=1, hooks=None, legs=None):
        os.makedirs(records, exist_ok=True)
        groups, shards = lanes_of(self.table_text())
        for group in groups[event]:
            for shard in shards[group]:
                if legs is not None and shard not in legs:
                    continue
                code, out, err, _, _ = self.leg(event, group, shard, attempt, specs[group], records,
                                                (hooks or {}).get(shard, (hooks or {}).get('*')))
                if code != 0:
                    raise RuntimeError('the runner refused a leg %s: %s %s' % (shard, out, err))

    def table_text(self):
        with open(os.path.join(self.path, '.github', 'scripts', 'partition.table')) as handle:
            return handle.read()

    def verify(self, records, event, attempt=1, extra=None):
        env = self.fixture.environment(self.github(event, attempt))
        env.update(extra or {})
        result = run([PYTHON, '-I', '-B', '.github/scripts/partition.py', 'verify', records], self.path, env)
        self.fixture.outputs.append(result[1] + result[2])
        return result


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


def unit_index(records, shard, source, attempt=1):
    record = json.load(open(os.path.join(leg_dir(records, shard, attempt), 'record.json')))
    return [u['source'] for u in record['units']].index(source)


def edit_record(records, shard, change, attempt=1):
    path = os.path.join(leg_dir(records, shard, attempt), 'record.json')
    with open(path) as handle:
        record = json.load(handle)
    change(record)
    with open(path, 'w') as handle:
        json.dump(record, handle)


def redigest(records, shard, name, attempt=1):
    import hashlib
    with open(os.path.join(leg_dir(records, shard, attempt), name), 'rb') as handle:
        value = hashlib.sha256(handle.read()).hexdigest()

    def change(record):
        for entry in record['steps']:
            if entry.get('log') == name:
                entry['sha256'] = value
            if entry.get('name') == 'run' and name == 'u%d-results.log' % entry['unit']:
                entry['results_sha256'] = value
    edit_record(records, shard, change, attempt)


def edit_file(records, shard, name, change, attempt=1, digest=True):
    path = os.path.join(leg_dir(records, shard, attempt), name)
    with open(path, 'rb') as handle:
        text = handle.read().decode('utf-8')
    with open(path, 'wb') as handle:
        handle.write(change(text).encode('utf-8'))
    if digest:
        redigest(records, shard, name, attempt)


def replace_once(old, new):
    def change(text):
        assert text.count(old) == 1, (old, text[:300])
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


def truth(ok):
    return (0 if ok else 1, '', '')


def main():
    root = tempfile.mkdtemp(prefix='partition-fixtures-')
    try:
        fixture = Fixture(root)
        cases(fixture)
        canary_absent(fixture, root)
        if fixture.failures:
            print('partition fixtures: %d FAILED: %s' % (len(fixture.failures), ', '.join(fixture.failures)))
            return 1
        print('partition fixtures: all cases passed')
        return 0
    finally:
        shutil.rmtree(root, ignore_errors=True)


def canary_absent(fixture, root):
    """The canary variable sat in every child's environment; no record, log or output may hold its value."""
    found = []
    for directory, _, files in os.walk(root):
        for name in files:
            path = os.path.join(directory, name)
            if os.path.islink(path) or name == 'fakecargo.py':
                continue
            with open(path, 'rb') as handle:
                if CANARY.encode() in handle.read():
                    found.append(path)
    found += ['an output' for text in fixture.outputs if CANARY in text]
    check(fixture, 'C-CANARY: an unrelated secret-like variable is neither collected nor printed', truth(not found), 0)


def cases(fixture):
    specs = {'windows': suite('windows'), 'macos': suite('macos')}
    table = real_table()
    repo = Repo(fixture, 'real-table', table)
    base = os.path.join(repo.path, 'records-queue')
    repo.lane('merge_group', base, specs)
    pr = os.path.join(repo.path, 'records-pr')
    repo.lane('pull_request', pr, specs)

    def copy(name, source=None):
        path = os.path.join(fixture.fresh(name), 'records')
        shutil.copytree(source or base, path, symlinks=True)
        return path

    lib_w = unit_index(base, 'w1', 'src/lib.rs')
    probe_w = unit_index(base, 'w1', 'examples/probe.rs')

    # -- the legitimate shapes ------------------------------------------------
    check(fixture, 'P-QUEUE: the merge-queue lane, four Windows and three macOS legs', repo.verify(base, 'merge_group'),
          0, 'every partitioned group ran every listed test exactly once', 'out')
    check(fixture, 'P-PR: the pull-request lane partitions macOS alone', repo.verify(pr, 'pull_request'),
          0, 'every partitioned group ran every listed test exactly once', 'out')
    recorded = json.load(open(os.path.join(leg_dir(base, 'w2'), 'record.json')))
    with open(os.path.join(repo.path, 'cargo-argv-w2-1.jsonl')) as handle:
        received = [['cargo'] + json.loads(line) for line in handle]
    check(fixture, 'P-ARGV: every recorded argv is an argv the fake cargo received, in order',
          truth([s['argv'] for s in recorded['steps']] == [r for r in received if r[1] in ('metadata', 'test')]), 0)
    query = [r for r in received if r[1] == 'rustc']
    check(fixture, 'P-QUERY: the runner asked Cargo `rustc -q --lib -- --version --verbose` once, and recorded its answer',
          truth(query == [['cargo', 'rustc', '-q', '--lib', '--', '--version', '--verbose']]
                and recorded['compiler']['argv'] == query[0] and recorded['compiler']['stdout'] == specs['windows']['vv']), 0)
    w2 = open(os.path.join(leg_dir(base, 'w2'), 'u%d-selected.log' % lib_w)).read()
    w3 = open(os.path.join(leg_dir(base, 'w3'), 'u%d-selected.log' % lib_w)).read()
    check(fixture, 'P-UNUSUAL: `recovery::` is not `recover::`, so w2 holds it; `my_engine::` is w3\'s',
          truth('engine::topology::recovery::tests::five: test' in w2 and 'my_engine::tests::seven: test' in w3), 0)
    w4 = open(os.path.join(leg_dir(base, 'w4'), 'u%d-run.log' % lib_w)).read()
    check(fixture, 'P-CHILD: a nested child\'s run of an ignored helper prints inside another leg\'s section, and passes',
          truth('test engine::topology::recover::tests::four ... ok' in w4), 0)
    w1 = open(os.path.join(leg_dir(base, 'w1'), 'u%d-run.log' % lib_w)).read()
    w1_results = open(os.path.join(leg_dir(base, 'w1'), 'u%d-results.log' % lib_w)).read()
    check(fixture, 'P-SHOULD-PANIC: `test <name> - should panic ... ok` on stdout, `ok <name>` in the logfile',
          truth('test x::engine::topology::recover::eight - should panic ... ok' in w1
                and 'ok x::engine::topology::recover::eight\n' in w1_results), 0)
    check(fixture, 'P-IGNORED-REASON: `ignored: <reason> <name>` in the logfile is read as ignored',
          truth('ignored: run only as a nested child engine::topology::recover::tests::four\n' in w1_results), 0)
    check(fixture, 'P-LOGFILE-WARNING: libtest\'s unterminated `--logfile` deprecation warning precedes the run header',
          truth('warning: `--logfile` is deprecated\nrunning 3 tests\n' in w1), 0)
    w1_units = json.load(open(os.path.join(leg_dir(base, 'w1'), 'record.json')))['units']
    check(fixture, 'P-INVENTORY: the units are the declared lib, bin and example, each with 9 equal executable observations',
          truth([(u['kind'], u['name']) for u in w1_units] == [('bin', 'upstroke'), ('example', 'probe'), ('lib', 'upstroke')]
                and all(len(u['observed']) == 9 and len(set(json.dumps(v) for _, v in u['observed'])) == 1 for u in w1_units)), 0)

    path = copy('interleaved')
    edit_file(path, 'w4', 'u%d-run.log' % lib_w,
              replace_once('test tests::engine_free ... ok', 'test tests::engine_fr[child output mid-line]\nee ... ok'))
    check(fixture, 'P-INTERLEAVED: a stdout result line split by other output; the logfile carries the outcome',
          repo.verify(path, 'merge_group'), 0)

    path = copy('crlf')
    edit_file(path, 'w1', 'u%d-run.log' % lib_w, lambda text: text.replace('\n', '\r\n'))
    edit_file(path, 'w1', 'u%d-results.log' % lib_w, lambda text: text.replace('\n', '\r\n'))
    check(fixture, 'P-CRLF: a run log and a logfile with CRLF line endings', repo.verify(path, 'merge_group'), 0)

    second = os.path.join(fixture.fresh('attempt-2'), 'records')
    repo.lane('merge_group', second, specs, attempt=2)
    check(fixture, 'P-ATTEMPT-2: every leg re-run at attempt 2, verified at attempt 2', repo.verify(second, 'merge_group', attempt=2), 0)
    path = copy('older-attempt', second)
    shutil.copytree(leg_dir(base, 'w2'), leg_dir(path, 'w2', 1))
    check(fixture, 'N-OLDER-ATTEMPT: an attempt-1 record beside attempt 2\'s is refused, not superseded',
          repo.verify(path, 'merge_group', attempt=2), 1, 'records from attempts [1], not this run attempt 2')
    path = copy('future-attempt')
    shutil.copytree(leg_dir(second, 'w2', 2), leg_dir(path, 'w2', 2))
    check(fixture, 'N-FUTURE-ATTEMPT: an attempt-2 record read by an attempt-1 job',
          repo.verify(path, 'merge_group', attempt=1), 1, 'records from attempts [2], not this run attempt 1')
    path = copy('partial-rerun', second)
    shutil.rmtree(leg_dir(path, 'w3', 2))
    check(fixture, 'N-PARTIAL-RERUN: attempt 2 re-ran three Windows legs; w3 has no attempt-2 record',
          repo.verify(path, 'merge_group', attempt=2), 1, 'w3: no record from attempt 2')

    empty_lib = [name for name in LIB if 'engine::topology::recover::' not in name]
    empty = {'windows': suite('windows', lib=empty_lib, ignored=['rundir::tests::nine'], children={}),
             'macos': suite('macos', lib=empty_lib, ignored=['rundir::tests::nine'], children={})}
    path = os.path.join(fixture.fresh('empty-shard'), 'records')
    repo.lane('merge_group', path, empty)
    check(fixture, 'P-EMPTY-SHARD: w1 selects no test; the units and coverage are still exact, and it is recorded',
          repo.verify(path, 'merge_group'), 0, 'w1 selects no test; recorded', 'out')

    future = {'windows': suite('windows', extra=True), 'macos': suite('macos', extra=True)}
    path = os.path.join(fixture.fresh('future-target'), 'records')
    repo.lane('merge_group', path, future)
    check(fixture, 'P-FUTURE-TARGET: a new integration test target is declared, built, run and covered',
          repo.verify(path, 'merge_group'), 0, '4 test units', 'out')
    path = os.path.join(fixture.fresh('future-target-unrun'), 'records')
    repo.lane('merge_group', path, future, hooks={'*': {'FAKE_SILENT': 'fresh'}})
    check(fixture, 'N-FUTURE-TARGET-UNRUN: the new target is built but no command ran its harness, on any leg',
          repo.verify(path, 'merge_group'), 1, '0 `Running` lines, not exactly one harness')

    # -- the target universe: omitted everywhere, or never built -----------------
    path = os.path.join(fixture.fresh('omit-everywhere'), 'records')
    repo.lane('merge_group', path, specs, hooks={'*': {'FAKE_SILENT': 'probe'}})
    check(fixture, 'N-OMIT-EVERYWHERE: the zero-test probe target missing from every command of every leg',
          repo.verify(path, 'merge_group'), 1, 'examples/probe.rs list: 0 `Running` lines')
    code, out, err, _, _ = repo.leg('merge_group', 'windows', 'w1', 1, specs['windows'], scratch(fixture, 'build-skip'),
                                    {'FAKE_BUILD_SKIP': 'probe'})
    check(fixture, 'N-BUILD-OMITS-DECLARED: the build reports no test unit for a declared target; the runner refuses',
          (code, out, err), 1, 'the build compiled test units')
    path = copy('build-json-tampered')
    edit_file(path, 'w3', 'build.json', lambda text: '\n'.join(l for l in text.split('\n') if 'probe' not in l), digest=False)
    redigest_build(path, 'w3')
    check(fixture, 'N-BUILD-JSON: a leg\'s build JSON lost the probe unit after recording, re-digested',
          repo.verify(path, 'merge_group'), 1, 'the build compiled test units')
    built = {'windows': suite('windows', build_script=True), 'macos': suite('macos', build_script=True)}
    path = os.path.join(fixture.fresh('build-script'), 'records')
    repo.lane('merge_group', path, built)
    w1_record = json.load(open(os.path.join(leg_dir(path, 'w1'), 'record.json')))
    check(fixture, 'P-BUILD-SCRIPT: a declared build script the build JSON reports is a compile-only unit, observed',
          truth(['custom-build', 'build-script-build'] in w1_record['compile_only']
                and repo.verify(path, 'merge_group')[0] == 0), 0)
    code, out, err, _, _ = repo.leg('merge_group', 'windows', 'w1', 1, built['windows'], scratch(fixture, 'build-script-unbuilt'),
                                    {'FAKE_BUILD_SKIP': 'build-script-build'})
    check(fixture, 'N-BUILD-SCRIPT-UNBUILT: the metadata declares a build script the build JSON never reports; the runner refuses',
          (code, out, err), 1, 'that the build JSON never reports')
    bench = {'windows': suite('windows', bench=True), 'macos': suite('macos', bench=True)}
    code, out, err, _, _ = repo.leg('merge_group', 'windows', 'w1', 1, bench['windows'], scratch(fixture, 'bench'))
    check(fixture, 'N-BENCH: a bench target is declared; the runner refuses a kind it does not model',
          (code, out, err), 1, "is of kind ['bench']")
    custom = Repo(fixture, 'custom-harness', table, CUSTOM)
    code, out, err, _, _ = custom.leg('merge_group', 'windows', 'w1', 1, specs['windows'], scratch(fixture, 'custom-run'))
    check(fixture, 'N-CUSTOM-RUN: a manifest `harness = false` target; the runner refuses', (code, out, err), 1, 'harness = false')
    check(fixture, 'N-CUSTOM-VERIFY: the verifier reads the checkout\'s manifest and refuses it too',
          custom.verify(base, 'merge_group'), 1, 'harness = false')

    # -- the table does not partition ----------------------------------------
    omits = Repo(fixture, 'omission-table', with_shard(table, 'w4', '--skip engine:: --skip tests::'))
    path = os.path.join(omits.path, 'records')
    omits.lane('merge_group', path, specs)
    check(fixture, 'N-OMISSION: w4 also skips `tests::`, so no leg selects them', omits.verify(path, 'merge_group'), 1, 'selected by no leg')

    balanced = with_shard(with_shard(table, 'w1', 'engine::topology::'), 'w4',
                          '--skip engine:: --skip rundir:: --skip agent::proc::')
    dup = Repo(fixture, 'duplicate-and-omission-table', balanced)
    path = os.path.join(dup.path, 'records')
    dup.lane('merge_group', path, specs)
    lib_d = unit_index(path, 'w1', 'src/lib.rs')
    sizes = sum(len(open(os.path.join(leg_dir(path, s), 'u%d-selected.log' % lib_d)).read().split(': test')) - 1
                for s in ('w1', 'w2', 'w3', 'w4'))
    listed = len(open(os.path.join(leg_dir(path, 'w1'), 'u%d-list.log' % lib_d)).read().split(': test')) - 1
    check(fixture, 'N-DUP-OMIT (control): the duplicate and the omission balance, so the counts still match', truth(sizes == listed), 0)
    check(fixture, 'N-DUP-OMIT: the duplicate half is named', dup.verify(path, 'merge_group'), 1, 'selected by more than one leg')
    check(fixture, 'N-DUP-OMIT: the omission half is named', dup.verify(path, 'merge_group'), 1, 'selected by no leg')

    floor = Repo(fixture, 'floor-table', real_table('1000'))
    path = os.path.join(floor.path, 'records')
    floor.lane('merge_group', path, specs)
    check(fixture, 'N-FLOOR: the group passes fewer tests than its floor', floor.verify(path, 'merge_group'), 1, 'below the floor')
    bad = Repo(fixture, 'unreadable-table', table.replace('shard windows w4 --skip engine::', 'shard windows w4 --exact engine::'))
    check(fixture, 'N-TABLE-FLAG: a table argument that is not a filter or `--skip <filter>`',
          bad.verify(os.path.join(bad.path, 'none'), 'merge_group'), 1, 'is neither a name filter')

    # -- the selection is not the table's -------------------------------------
    path = copy('argv')

    def drop_skip(record):
        for entry in record['steps']:
            if entry.get('name') == 'run' and entry.get('unit') == lib_w:
                entry['argv'] = [w for w in entry['argv'] if w not in ('--skip', 'engine::topology::recover::')]
    edit_record(path, 'w2', drop_skip)
    check(fixture, 'N-ARGV: w2\'s recorded library run argv lost its `--skip`', repo.verify(path, 'merge_group'), 1, 'the unit commands')
    path = copy('harness-selection')
    edit_file(path, 'w2', 'u%d-selected.log' % lib_w, replace_once('engine::topology::tests::six: test\n\n2 tests',
              'engine::topology::tests::six: test\nmy_engine::tests::seven: test\n\n3 tests'))
    check(fixture, 'N-HARNESS: w2\'s harness listed a test its predicate does not select', repo.verify(path, 'merge_group'), 1,
          'the harness selected 3 tests and the table selects 2')

    # -- discovery differs ------------------------------------------------------
    path = copy('same-count')
    edit_file(path, 'w3', 'u%d-list.log' % lib_w, replace_once('agent::proc::tests::one: test', 'agent::proc::tests::uno: test'))
    check(fixture, 'N-SAME-COUNT: w3 listed the same number of tests under another name', repo.verify(path, 'merge_group'), 1,
          'listed different tests')
    path = copy('conditional')
    edit_file(path, 'w3', 'u%d-list.log' % lib_w, replace_once('agent::proc::tests::one: test\n', ''))
    edit_file(path, 'w3', 'u%d-list.log' % lib_w, replace_once('\n10 tests, 0 benchmarks', '\n9 tests, 0 benchmarks'))
    edit_file(path, 'w3', 'u%d-run.log' % lib_w, replace_once('0 measured; 8 filtered out', '0 measured; 7 filtered out'))
    check(fixture, 'N-CFG: w3 compiled one test fewer, and its own run agrees', repo.verify(path, 'merge_group'), 1, 'listed different tests')
    path = copy('ignored-state')
    edit_file(path, 'w3', 'u%d-ignored.log' % lib_w, replace_once('engine::topology::recover::tests::four: test\n',
              'agent::proc::tests::one: test\nengine::topology::recover::tests::four: test\n'))
    edit_file(path, 'w3', 'u%d-ignored.log' % lib_w, replace_once('\n2 tests, 0 benchmarks', '\n3 tests, 0 benchmarks'))
    check(fixture, 'N-IGNORED: w3 ignores a test the other legs run', repo.verify(path, 'merge_group'), 1, 'ignore different tests')

    # -- outcomes: the logfile, not stdout, says what ran ----------------------------
    path = copy('ignored-as-ok')
    edit_file(path, 'w1', 'u%d-results.log' % lib_w, replace_once('ignored: run only as a nested child engine::topology::recover::tests::four',
              'ok engine::topology::recover::tests::four'))
    check(fixture, 'N-IGNORED-AS-OK: the logfile reports an ignored test as run', repo.verify(path, 'merge_group'), 1, 'against the ignore listing')
    path = copy('child-supplies-name')
    edit_file(path, 'w1', 'u%d-results.log' % lib_w, replace_once('ok engine::topology::recover::tests::three\n', ''))
    edit_file(path, 'w1', 'u%d-run.log' % lib_w, replace_once('test engine::topology::recover::tests::three ... ok\n',
              '\nrunning 1 test\ntest engine::topology::recover::tests::three ... ok\n\ntest result: ok. 1 passed; 0 failed; '
              '0 ignored; 0 measured; 9 filtered out; finished in 0.00s\n\n'))
    check(fixture, 'N-CHILD-SUPPLIES-NAME: the top level never reported a name a nested child printed; counts unchanged',
          repo.verify(path, 'merge_group'), 1, 'never reported: 1, e.g. engine::topology::recover::tests::three')
    path = copy('duplicate-outcome')
    edit_file(path, 'w1', 'u%d-results.log' % lib_w, replace_once('ok engine::topology::recover::tests::three\n',
              'ok x::engine::topology::recover::eight\n'))
    check(fixture, 'N-DUP-OUTCOME-OMIT: one name reported twice and another never, the line count unchanged',
          repo.verify(path, 'merge_group'), 1, 'reports x::engine::topology::recover::eight twice')
    path = copy('prefix')
    edit_file(path, 'w1', 'u%d-results.log' % lib_w, replace_once('ok engine::topology::recover::tests::three\n',
              'ok engine::topology::recover::tests::thre\n'))
    check(fixture, 'N-PREFIX: the logfile names an incomplete prefix of a selected name', repo.verify(path, 'merge_group'), 1,
          'only the logfile: 1, e.g. engine::topology::recover::tests::thre')
    path = copy('child-summary-same-counts')
    edit_file(path, 'w4', 'u%d-results.log' % lib_w, replace_once('ok tests::engine_free\n', ''))

    def child_summary_last(text):
        lines = text.rstrip('\n').split('\n')
        summary = [i for i, line in enumerate(lines) if line.startswith('test result: ok. 2 passed')]
        assert len(summary) == 1
        parent = lines[summary[0]]
        del lines[summary[0]]
        return '\n'.join(lines) + '\n\nrunning 1 test\ntest tests::engine_free ... ok\n\n' + parent + '\n'
    edit_file(path, 'w4', 'u%d-run.log' % lib_w, child_summary_last)
    check(fixture, 'N-CHILD-SUMMARY-SAME-COUNTS: the parent never reported a name; a child\'s summary with the parent\'s counts ends stdout',
          repo.verify(path, 'merge_group'), 1, 'never reported: 1, e.g. tests::engine_free')
    path = copy('failed-outcome')
    edit_file(path, 'w2', 'u%d-results.log' % lib_w, replace_once('ok engine::topology::tests::six\n', 'failed engine::topology::tests::six\n'))
    check(fixture, 'N-FAILED-OUTCOME: the logfile reports a failure', repo.verify(path, 'merge_group'), 1, 'did not pass')
    path = copy('no-summary')
    edit_file(path, 'w2', 'u%d-run.log' % lib_w, lambda text: '\n'.join(l for l in text.split('\n') if not l.startswith('test result:')))
    check(fixture, 'N-NO-SUMMARY: stdout has no top-level summary line at all', repo.verify(path, 'merge_group'), 1, 'no `test result: ok.` line')
    path = copy('warning-glued')
    edit_file(path, 'w2', 'u%d-run.log' % lib_w, replace_once('deprecated\nrunning 2 tests', 'deprecatedrunning 2 tests'))
    check(fixture, 'P-WARNING-GLUED: the warning and the header on one line', repo.verify(path, 'merge_group'), 0)
    path = copy('no-header')
    edit_file(path, 'w2', 'u%d-run.log' % lib_w, replace_once('running 2 tests\n', ''))
    check(fixture, 'N-HEADER: the run does not open with its own `running N tests`', repo.verify(path, 'merge_group'), 1,
          'does not open with its own `running 2 tests`')
    path = copy('logfile-missing')
    os.remove(os.path.join(leg_dir(path, 'w2'), 'u%d-results.log' % lib_w))
    check(fixture, 'N-LOGFILE-MISSING: a unit\'s logfile is absent from the record', repo.verify(path, 'merge_group'), 1, "the record's files")

    # -- a target, a harness or a run is missing or not the unit's ------------------
    path = copy('missing-probe')
    edit_file(path, 'w2', 'u%d-run.log' % probe_w, lambda text: '\n'.join(l for l in text.split('\n') if 'Running' not in l))
    check(fixture, 'N-MISSING-PROBE: w2\'s probe run shows no harness', repo.verify(path, 'merge_group'), 1, '0 `Running` lines')
    path = copy('two-harnesses')
    edit_file(path, 'w2', 'u%d-run.log' % probe_w,
              lambda text: text + '     Running unittests src/main.rs (target\\debug\\deps\\upstroke-4d5e6f.exe)\n')
    check(fixture, 'N-TWO-HARNESSES: a unit\'s command ran a second harness', repo.verify(path, 'merge_group'), 1, '2 `Running` lines')
    path = copy('wrong-executable')
    edit_file(path, 'w3', 'u%d-list.log' % lib_w, replace_once('upstroke-1a2b3c.exe', 'upstroke-ffffff.exe'))
    check(fixture, 'N-WRONG-EXE: a command ran another executable than the unit\'s', repo.verify(path, 'merge_group'), 1,
          'the harness executable')
    path = copy('units-tampered')
    edit_record(path, 'm2', lambda r: r['units'][0].__setitem__('executable', r['units'][0]['executable'] + '.old'))
    check(fixture, 'N-UNITS: a record\'s units are not the build\'s', repo.verify(path, 'merge_group'), 1, 'the test units')
    replaced = os.path.join(fixture.fresh('replaced-executable'), 'records')
    repo.lane('merge_group', replaced, specs, hooks={'w1': {'FAKE_REPLACE': 'upstroke'}})
    check(fixture, 'N-REPLACED: the same path holds other bytes when the run starts', repo.verify(replaced, 'merge_group'), 1,
          'changed or vanished between the build and the last command')
    retouched = os.path.join(fixture.fresh('recompiled-executable'), 'records')
    repo.lane('merge_group', retouched, specs, hooks={'m2': {'FAKE_RECOMPILE': 'upstroke'}})
    check(fixture, 'N-RECOMPILED: the same bytes rewritten (a new modification time) before the run', repo.verify(retouched, 'merge_group'), 1,
          'changed or vanished between the build and the last command')
    path = copy('identity-tampered')
    edit_record(path, 'w4', lambda r: r['units'][0]['observed'][-1][1].__setitem__(0, '0' * 64))
    check(fixture, 'N-IDENTITY-RECORD: a recorded observation disagrees with the others', repo.verify(path, 'merge_group'), 1,
          'changed or vanished')

    def truncate(text):
        lines = text.split('\n')
        return '\n'.join(lines[:6])
    path = copy('truncated')
    edit_file(path, 'w1', 'u%d-run.log' % lib_w, truncate)
    check(fixture, 'N-TRUNCATED: w1\'s library run log stops after its first results, re-digested', repo.verify(path, 'merge_group'), 1,
          'no `test result: ok.` line')
    path = copy('incomplete-upload')
    edit_file(path, 'w1', 'u%d-run.log' % lib_w, truncate, digest=False)
    check(fixture, 'N-INCOMPLETE: the same cut, not re-digested', repo.verify(path, 'merge_group'), 1, "the log's digest")
    path = copy('failed-run')
    edit_file(path, 'w2', 'u%d-run.log' % lib_w, replace_once('test result: ok. 2 passed; 0 failed', 'test result: FAILED. 1 passed; 1 failed'))
    check(fixture, 'N-FAILED: w2 reports a failure on stdout', repo.verify(path, 'merge_group'), 1, 'no `test result: ok.` line')

    def exit_101(record):
        for entry in record['steps']:
            if entry.get('name') == 'run' and entry.get('unit') == lib_w:
                entry['exit'] = 101
    path = copy('failed-exit')
    edit_record(path, 'w2', exit_101)
    check(fixture, 'N-EXIT: w2\'s library run exited 101', repo.verify(path, 'merge_group'), 1, 'the exit is 101')
    path = copy('missing-leg')
    shutil.rmtree(leg_dir(path, 'w3'))
    check(fixture, 'N-MISSING-LEG: w3 uploaded nothing (failed, cancelled or skipped)', repo.verify(path, 'merge_group'), 1,
          'w3: no record from attempt 1')
    path = copy('not-libtest')
    edit_file(path, 'w1', 'u%d-list.log' % lib_w, replace_once('tests::engine_free: test\n', 'tests::engine_free: test\nhello from a custom harness\n'))
    check(fixture, 'N-FORMAT: a listing line that is not libtest\'s', repo.verify(path, 'merge_group'), 1, 'not a libtest listing line')

    # -- provenance --------------------------------------------------------------
    for label, shard, change, needle in [
        ('N-CHECKOUT: w1 recorded another checkout', 'w1', lambda r: r.__setitem__('checkout', '0' * 40), 'the checkout'),
        ('N-RUN: m1 recorded another run', 'm1', lambda r: r['github'].__setitem__('GITHUB_RUN_ID', '4243'), 'the run is'),
        ('N-ATTEMPT: m1\'s record says attempt 2 under an attempt-1 artifact name', 'm1',
         lambda r: r['github'].__setitem__('GITHUB_RUN_ATTEMPT', '2'), 'the run attempt'),
        ('N-EVENT: w4 recorded another event', 'w4', lambda r: r['github'].__setitem__('GITHUB_EVENT_NAME', 'pull_request'), 'the event'),
        ('N-PLATFORM: w3 ran on Linux', 'w3', lambda r: r['runner'].__setitem__('RUNNER_OS', 'Linux'), 'the runner OS'),
        ('N-SELF-HOSTED: w3 on a self-hosted runner', 'w3', lambda r: r['runner'].__setitem__('RUNNER_ENVIRONMENT', 'self-hosted'),
         'the runner environment'),
        ('N-HOST: m3\'s compiler targets another host', 'm3', lambda r: r['compiler'].__setitem__('stdout', vv('1.99.0', 'x86_64-apple-darwin')),
         "the compiler's host"),
        ('N-RELEASE: w1\'s compiler is not 1.97.1', 'w1', lambda r: r['compiler'].__setitem__('stdout', vv('1.98.0', 'x86_64-pc-windows-msvc')),
         "the compiler's release"),
        ('N-OLD-QUERY: w1 recorded the earlier `-- -vV` compiler query', 'w1',
         lambda r: r['compiler'].__setitem__('argv', ['cargo', 'rustc', '-q', '--lib', '--', '-vV']), 'the compiler query is'),
        ('N-NO-RELEASE: m2\'s compiler answer names no release, on a group of any release', 'm2',
         lambda r: r['compiler'].__setitem__('stdout', ''.join(line + '\n' for line in r['compiler']['stdout'].splitlines()
                                                              if not line.startswith('release: '))),
         'printed no rustc version, host and release'),
        ('N-TABLE: w2 ran another table', 'w2', lambda r: r.__setitem__('table', '1' * 40), 'the table blob'),
        ('N-MANIFEST: w2 ran another manifest', 'w2', lambda r: r.__setitem__('manifest', '2' * 40), 'the manifest blob'),
        ('N-SEPARATOR: w2\'s record names no path separator', 'w2', lambda r: r.__setitem__('sep', ':'), 'no path separator'),
    ]:
        path = copy(label.split(':')[0].lower())
        edit_record(path, shard, change)
        check(fixture, label, repo.verify(path, 'merge_group'), 1, needle)
    path = copy('compilers')
    edit_record(path, 'm2', lambda r: r['compiler'].__setitem__('stdout', r['compiler']['stdout'].replace('0123456789abcdef', 'fedcba9876543210')))
    check(fixture, 'N-COMPILERS: m1 and m2 ran different builds of one release', repo.verify(path, 'merge_group'), 1, 'ran different compilers')
    check(fixture, 'N-HEAD: the run\'s commit is not this checkout', repo.verify(base, 'merge_group', extra={'GITHUB_SHA': '2' * 40}), 1,
          "not the run's commit")
    check(fixture, 'N-LANE: an event with no lane', repo.verify(base, 'workflow_dispatch'), 1, 'has no lane')

    # -- the artifacts themselves -----------------------------------------------
    path = copy('extra-leg')
    shutil.copytree(leg_dir(path, 'w4'), leg_dir(path, 'w5'))
    check(fixture, 'N-EXTRA: a record from a shard the lane does not run', repo.verify(path, 'merge_group'), 1, 'not a shard')
    check(fixture, 'N-WRONG-LANE: the queue\'s Windows records read on the pull-request lane', repo.verify(base, 'pull_request'), 1,
          'not a shard of the pull_request lane')
    path = copy('stray-file')
    with open(os.path.join(path, 'stray.txt'), 'w') as handle:
        handle.write('x')
    check(fixture, 'N-STRAY: a file among the records', repo.verify(path, 'merge_group'), 1, 'an unexpected entry')
    path = copy('extra-file')
    with open(os.path.join(leg_dir(path, 'w1'), 'extra.log'), 'w') as handle:
        handle.write('x')
    check(fixture, 'N-EXTRA-FILE: an extra file in w1\'s record', repo.verify(path, 'merge_group'), 1, "the record's files")
    path = copy('symlink')
    target = os.path.join(leg_dir(path, 'w1'), 'u%d-run.log' % lib_w)
    os.remove(target)
    os.symlink(os.path.join(leg_dir(path, 'w2'), 'u%d-run.log' % lib_w), target)
    check(fixture, 'N-LINK: w1\'s run log is a link', repo.verify(path, 'merge_group'), 1, 'other than regular files')
    path = copy('linked-leg')
    shutil.rmtree(leg_dir(path, 'w3'))
    os.symlink(leg_dir(base, 'w3'), leg_dir(path, 'w3'))
    check(fixture, 'N-LINKED-DIR: w3\'s record is a link to a directory elsewhere', repo.verify(path, 'merge_group'), 1, 'an unexpected entry')
    empty_dir = os.path.join(fixture.fresh('no-records'), 'records')
    os.mkdir(empty_dir)
    check(fixture, 'N-EMPTY: no record at all', repo.verify(empty_dir, 'merge_group'), 1, 'no record')
    check(fixture, 'N-NO-DIRECTORY: the records directory is missing', repo.verify(empty_dir + '-absent', 'merge_group'), 1,
          'the records directory')

    # -- the runner ---------------------------------------------------------------
    code, out, err, _, _ = repo.leg('merge_group', 'windows', 'm1', 1, specs['windows'], scratch(fixture, 'other-group'))
    check(fixture, 'R-SHARD: the runner refuses a shard of another group', (code, out, err), 1, "has no shard 'm1'")
    os.mkdir(os.path.join(repo.path, 'test-partition'))
    code, out, err, target, _ = repo.leg('merge_group', 'windows', 'w1', 1, specs['windows'], scratch(fixture, 'exists'))
    check(fixture, 'R-EXISTS: the runner refuses an existing record directory', (code, out, err), 1, 'the record directory')
    os.rmdir(target)
    code, out, err, target, _ = repo.leg('merge_group', 'windows', 'w1', 1, specs['windows'], scratch(fixture, 'fail'),
                                         {'FAKE_FAIL': 'run'})
    failed = json.load(open(os.path.join(target, 'record.json')))
    check(fixture, 'R-FAIL: a failing run stops the runner with its exit status, recorded',
          (code if failed['steps'][-1]['exit'] == 101 else -1, out, err), 101)
    shutil.rmtree(target)
    for hook, label, needle in (
            ('fail', 'R-QUERY-FAILS: the runner refuses a compiler query that exits non-zero', 'exited 101'),
            ('empty', 'R-QUERY-EMPTY: the runner refuses an empty compiler answer', 'printed no rustc version, host and release'),
            ('no-release', 'R-QUERY-MALFORMED: the runner refuses a compiler answer that names no release',
             'printed no rustc version, host and release')):
        code, out, err, target, _ = repo.leg('merge_group', 'windows', 'w1', 1, specs['windows'], scratch(fixture, 'query-' + hook),
                                             {'FAKE_QUERY': hook})
        check(fixture, label, (code, out, err), 1, needle)
        if os.path.isdir(target):
            shutil.rmtree(target)
    for name in ('.github/scripts/partition.table', 'Cargo.toml'):
        with open(os.path.join(repo.path, name), 'a') as handle:
            handle.write('# a local edit\n')
        code, out, err, _, _ = repo.leg('merge_group', 'windows', 'w1', 1, specs['windows'], scratch(fixture, 'dirty'))
        check(fixture, 'R-DIRTY: the runner refuses a %s that is not the checkout\'s' % name, (code, out, err), 1,
              "is not the checkout's")
        git(repo.path, repo.env, 'checkout', '--', name)


def scratch(fixture, name):
    path = os.path.join(fixture.fresh(name), 'records')
    os.mkdir(path)
    return path


def redigest_build(records, shard):
    import hashlib
    leg = leg_dir(records, shard)

    def change(record):
        for entry in record['steps']:
            if entry.get('name') == 'build':
                entry['sha256'] = [hashlib.sha256(open(os.path.join(leg, n), 'rb').read()).hexdigest() for n in ('build.json', 'build.log')]
    edit_record(records, shard, change)


if __name__ == '__main__':
    sys.exit(main())
