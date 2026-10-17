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
    than guessed at, and every other unit the build compiled is accounted for
    as compile-only;
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

TABLE_PATH = '.github/scripts/partition.table'
SCRIPT_PATH = '.github/scripts/partition.py'
MANIFEST_PATH = 'Cargo.toml'
EVENTS = ('merge_group', 'pull_request', 'push')
NAME = re.compile(r'[a-z][a-z0-9]*')
FILTER = re.compile(r'[A-Za-z0-9_][A-Za-z0-9_:]*')
METADATA = ['cargo', 'metadata', '--format-version', '1', '--no-deps']
BUILD = ['cargo', 'test', '--all-targets', '--all-features', '--no-run', '--message-format=json-render-diagnostics']
UNIT_TEST = ['cargo', 'test', '--all-features']
COMPILER = ['cargo', 'rustc', '-q', '--lib', '--', '-vV']
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
    """The leg's test units and compile-only units, from Cargo's metadata and the build's JSON artifacts alone."""
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
    compile_only = []
    for target in packages[0].get('targets', []):
        kinds = target.get('kind')
        if kinds == ['custom-build']:
            compile_only.append(('custom-build', target.get('name')))
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
            compile_only.append((kind, target.get('name')))
    if finished is not True:
        raise Refused('%s: the build JSON has no successful `build-finished` message' % where)
    if sorted(units) != sorted(expected):
        raise Refused('%s: the build compiled test units %r for the declared targets %r'
                      % (where, sorted(units), sorted(expected)))
    for key, source in expected.items():
        if units[key]['source'] != source:
            raise Refused('%s: test unit %r built %r, not the declared %r' % (where, key, units[key]['source'], source))
    ordered = [units[key] for key in sorted(units)]
    return ordered, sorted(set((str(k), str(n)) for k, n in compile_only)), root


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


def run(group, shard, directory):
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

    def finish(status):
        with open(os.path.join(directory, RECORD), 'w') as handle:
            json.dump(record, handle, indent=1, sort_keys=True)
            handle.write('\n')
        return status

    def step(entry):
        record['steps'].append(entry)
        if entry['exit'] != 0:
            print('partition: %s %s exited %d' % (shard, entry['name'], entry['exit']), flush=True)
        return entry['exit']

    print('partition: %s metadata: %s' % (shard, ' '.join(METADATA)), flush=True)
    code = stream(METADATA, os.path.join(directory, 'metadata.json'), os.path.join(directory, 'metadata.log'), False)
    if step({'name': 'metadata', 'argv': METADATA, 'exit': code, 'out': 'metadata.json', 'err': 'metadata.log',
             'sha256': [digest(os.path.join(directory, 'metadata.json')), digest(os.path.join(directory, 'metadata.log'))]}):
        return finish(code)
    print('partition: %s build: %s' % (shard, ' '.join(BUILD)), flush=True)
    code = stream(BUILD, os.path.join(directory, 'build.json'), os.path.join(directory, 'build.log'), True)
    if step({'name': 'build', 'argv': BUILD, 'exit': code, 'out': 'build.json', 'err': 'build.log',
             'sha256': [digest(os.path.join(directory, 'build.json')), digest(os.path.join(directory, 'build.log'))]}):
        return finish(code)
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
            print('partition: %s %s %s: %s' % (shard, unit['source'], name, ' '.join(argv)), flush=True)
            unit['observed'].append(['before %s' % name, identity(unit['executable'])])
            code = tee(argv, log) if name == 'run' else into(argv, log)
            unit['observed'].append(['after %s' % name, identity(unit['executable'])])
            entry = {'name': name, 'unit': index, 'argv': argv, 'exit': code, 'log': os.path.basename(log), 'sha256': digest(log)}
            if name == 'run':
                results = os.path.join(directory, 'u%d-results.log' % index)
                if not os.path.isfile(results):
                    with open(results, 'wb'):
                        pass
                entry['results_sha256'] = digest(results)
            if code != 0 and name != 'run':
                with open(log, 'rb') as handle:
                    sys.stdout.flush()
                    sys.stdout.buffer.write(handle.read())
                    sys.stdout.flush()
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
    facts = dict(line.split(': ', 1) for line in verbose.splitlines() if ': ' in line)
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
            dict(line.split(': ', 1) for line in first.compiler.splitlines() if ': ' in line).get('release'),
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
    try:
        if len(argv) == 4 and argv[0] == 'run':
            return run(argv[1], argv[2], argv[3])
        if len(argv) == 2 and argv[0] == 'verify':
            return verify(argv[1])
    except Refused as error:
        print('partition: refused: %s' % error, file=sys.stderr)
        return 1
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
