#!/usr/bin/env python3
"""Run one shard of the test suite, or verify that a lane's shards ran all of it.

    python3 .github/scripts/partition.py run <group> <shard> <record-dir>
    python3 .github/scripts/partition.py verify <records-dir>

Both run from the repository root and read the predicate table beside this
file, `.github/scripts/partition.table`.

`run` executes on a partitioned test leg of .github/workflows/ci.yml. It runs
`cargo test --all-targets --all-features` five times -- build, list every test,
list the ignored ones, list what this shard's arguments select, and run that
selection -- and records each command's exact argv, exit status and output in
<record-dir>: stdout and stderr in one stream, written by the operating system
in the order the processes wrote them, so Cargo's `Running` line heads the
output of the harness it starts. The build and the run are also copied to this
script's own output as they arrive. Beside them it records the checkout, the
table and script it ran (as Git blobs), the run, the runner and the compiler.
Cargo still chooses the targets, their working directory and their
environment: the arguments after `--` are the only thing added, and nothing is
added to the environment this script inherited.

`verify` executes in the `test-coverage` job, on the records the legs
uploaded. It refuses -- exit 1, every reason printed -- unless, for every group
the event's lane partitions:

  * each of the group's shards has a record from this run, made at this
    checkout with this table and this script, on the group's platform, by
    exactly the commands above, every one of which exited 0, with every log the
    digest the leg recorded; an older attempt's record is superseded by a newer
    one, and a record from an attempt after this job's is refused;
  * no record is anything else: no other name, file, link or directory;
  * every leg built the same targets, from the same compiler, and listed the
    same tests and the same ignored tests in each, and its four Cargo commands
    ran the same harness binaries in the same order;
  * each leg's harness selected exactly what the table's predicate selects from
    the names it listed, and its run of each target opens with its own
    `running N tests` line, names every selected test in a `test <name> ... `
    line, and ends in its own `test result:` line accounting for exactly that
    selection: N run, the rest filtered out, the selected ignored tests
    ignored, none failed;
  * the group's selections are disjoint and together hold every listed test of
    every target, and the group's legs together report at least the table's
    floor of passing tests.
"""

import hashlib
import json
import os
import re
import stat
import subprocess
import sys

TABLE_PATH = '.github/scripts/partition.table'
SCRIPT_PATH = '.github/scripts/partition.py'
EVENTS = ('merge_group', 'pull_request', 'push')
NAME = re.compile(r'[a-z][a-z0-9]*')
FILTER = re.compile(r'[A-Za-z0-9_][A-Za-z0-9_:]*')
CARGO_TEST = ['cargo', 'test', '--all-targets', '--all-features']
STEPS = ('build', 'list', 'ignored', 'selected', 'run')
LIVE = ('build', 'run')
RECORD = 'record.json'
FILES = sorted([RECORD] + [step + '.log' for step in STEPS])
ARTIFACT = re.compile(r'test-partition-([a-z][a-z0-9]*)-attempt-([1-9][0-9]{0,5})')
FORMAT = 1
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
SUMMARY = re.compile(
    r'test result: (ok|FAILED)\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored; '
    r'([0-9]+) measured; ([0-9]+) filtered out; finished in [0-9.]+s'
)
RESULT = re.compile(r'test (\S+)(?: - [a-z ]+)? \.\.\. ')


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
    for path in (TABLE_PATH, SCRIPT_PATH):
        committed = git('rev-parse', '--verify', '--quiet', 'HEAD:' + path)
        working = git('hash-object', '--', path)
        if committed != working:
            raise Refused('%s is not the checkout\'s: blob %s in the tree, %s at HEAD' % (path, working, committed))
        blobs[path] = committed
    return blobs


def commands(args):
    return [
        ('build', CARGO_TEST + ['--no-run']),
        ('list', CARGO_TEST + ['--', '--list']),
        ('ignored', CARGO_TEST + ['--', '--list', '--ignored']),
        ('selected', CARGO_TEST + ['--', '--list'] + args),
        ('run', CARGO_TEST + ['--'] + args),
    ]


def digest(path):
    sha = hashlib.sha256()
    with open(path, 'rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            sha.update(block)
    return sha.hexdigest()


def capture(argv):
    result = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        raise Refused('%s exited %d: %s' % (' '.join(argv), result.returncode, result.stderr.decode('utf-8', 'replace').strip()))
    return {'argv': argv, 'exit': result.returncode, 'stdout': result.stdout.decode('utf-8', 'replace')}


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
    try:
        os.mkdir(directory)
    except OSError as error:
        raise Refused('the record directory %r: %s' % (directory, error))
    record = {
        'format': FORMAT,
        'group': group,
        'shard': shard,
        'args': args,
        'table': blobs[TABLE_PATH],
        'script': blobs[SCRIPT_PATH],
        'checkout': git('rev-parse', '--verify', '--quiet', 'HEAD'),
        'github': dict((key, os.environ.get(key, '')) for key in GITHUB_KEYS),
        'runner': dict((key, os.environ.get(key, '')) for key in RUNNER_KEYS),
        'compiler': capture(['cargo', 'rustc', '-q', '--lib', '--', '-vV']),
        'path_rustc': capture(['rustc', '-vV']),
        'cargo': capture(['cargo', '-V']),
        'commands': [],
    }
    status = 0
    for step, argv in commands(args):
        log = os.path.join(directory, step + '.log')
        print('partition: %s %s: %s' % (shard, step, ' '.join(argv)), flush=True)
        code = tee(argv, log) if step in LIVE else into(argv, log)
        record['commands'].append({'name': step, 'argv': argv, 'exit': code, 'log': step + '.log', 'sha256': digest(log)})
        if code != 0:
            if step not in LIVE:
                with open(log, 'rb') as handle:
                    sys.stdout.flush()
                    sys.stdout.buffer.write(handle.read())
                    sys.stdout.flush()
            print('partition: %s %s exited %d' % (shard, step, code), flush=True)
            status = code
            break
    with open(os.path.join(directory, RECORD), 'w') as handle:
        json.dump(record, handle, indent=1, sort_keys=True)
        handle.write('\n')
    return status


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
    if not found:
        raise Refused('%s: no `Running` line, so no harness ran' % where)
    return found


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


def outcome(body, where):
    lines = [line for line in body if line.strip()]
    header = HEADER.fullmatch(lines[0]) if lines else None
    if not header:
        raise Refused('%s: does not open with its own `running N tests` line' % where)
    summary = SUMMARY.fullmatch(lines[-1]) if len(lines) > 1 else None
    if not summary:
        raise Refused('%s: does not end in its own `test result:` line: cut short, or other output followed it' % where)
    named = set(match.group(1) for match in RESULT.finditer('\n'.join(lines[1:-1])))
    numbers = [int(value) for value in summary.groups()[1:]]
    return int(header.group(1)), summary.group(1), numbers, named


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
    require(where, 'the checkout', record.get('checkout'), head)
    github = record.get('github') if isinstance(record.get('github'), dict) else {}
    require(where, 'the run', github.get('GITHUB_RUN_ID'), env['GITHUB_RUN_ID'])
    require(where, 'the run attempt', github.get('GITHUB_RUN_ATTEMPT'), str(attempt))
    require(where, 'the commit', github.get('GITHUB_SHA'), env['GITHUB_SHA'])
    require(where, 'the event', github.get('GITHUB_EVENT_NAME'), env['GITHUB_EVENT_NAME'])
    require(where, 'the repository', github.get('GITHUB_REPOSITORY'), env['GITHUB_REPOSITORY'])
    runner = record.get('runner') if isinstance(record.get('runner'), dict) else {}
    require(where, 'the runner OS', runner.get('RUNNER_OS'), table.groups[group]['os'])
    require(where, 'the runner environment', runner.get('RUNNER_ENVIRONMENT'), 'github-hosted')
    compiler = record.get('compiler') if isinstance(record.get('compiler'), dict) else {}
    require(where, 'the compiler query', compiler.get('argv'), ['cargo', 'rustc', '-q', '--lib', '--', '-vV'])
    require(where, 'the compiler query\'s exit', compiler.get('exit'), 0)
    verbose = str(compiler.get('stdout'))
    facts = dict(line.split(': ', 1) for line in verbose.splitlines() if ': ' in line)
    require(where, 'the compiler\'s host', facts.get('host'), table.groups[group]['triple'])
    if table.groups[group]['release'] != 'any':
        require(where, 'the compiler\'s release', facts.get('release'), table.groups[group]['release'])
    ran = record.get('commands') if isinstance(record.get('commands'), list) else []
    require(where, 'the commands', [command.get('name') if isinstance(command, dict) else None for command in ran], list(STEPS))
    for (step, argv), command in zip(commands(args), ran):
        require(where, 'the %s argv' % step, command.get('argv'), argv)
        require(where, 'the %s exit' % step, command.get('exit'), 0)
        require(where, 'the %s log' % step, command.get('log'), step + '.log')
        require(where, 'the %s log\'s digest' % step, digest(os.path.join(path, step + '.log')), command.get('sha256'))

    parsed = {}
    for step in STEPS[1:]:
        parsed[step] = sections(os.path.join(path, step + '.log'), '%s %s' % (where, step))
    shape = [(source, binary) for source, binary, _ in parsed['list']]
    for step in STEPS[2:]:
        if [(source, binary) for source, binary, _ in parsed[step]] != shape:
            raise Refused(
                '%s: the %s command ran %r, not the listing\'s %r'
                % (where, step, [(source, binary) for source, binary, _ in parsed[step]], shape)
            )
    targets = [source for source, _ in shape]
    if len(set(targets)) != len(targets):
        raise Refused('%s: a target runs twice: %r' % (where, targets))
    listed = parsed['list']
    ignored = dict((source, body) for source, _, body in parsed['ignored'])
    chosen = dict((source, body) for source, _, body in parsed['selected'])
    executed = dict((source, body) for source, _, body in parsed['run'])
    positives, skips = predicate(args, where)

    leg = Leg()
    leg.shard = shard
    leg.attempt = attempt
    leg.group = group
    leg.targets = targets
    leg.compiler = verbose
    leg.universe = {}
    leg.ignored = {}
    leg.selected = {}
    leg.passed = 0
    leg.lines = []
    for source, _, body in listed:
        at = '%s %s' % (where, source)
        universe = listing(body, at + ' list')
        hidden = set(listing(ignored[source], at + ' ignored'))
        if not hidden <= set(universe):
            raise Refused('%s: ignores tests it does not list: %s' % (at, sample(hidden - set(universe))))
        wanted = set(name for name in universe if selects(positives, skips, name))
        harness = set(listing(chosen[source], at + ' selected'))
        if harness != wanted:
            raise Refused(
                '%s: the harness selected %d tests and the table selects %d: only the harness: %s; only the table: %s'
                % (at, len(harness), len(wanted), sample(harness - wanted), sample(wanted - harness))
            )
        total, verdict, numbers, named = outcome(executed[source], at + ' run')
        passed, failed, skipped, measured, filtered = numbers
        expected = [len(wanted), 'ok', len(wanted), 0, len(wanted & hidden), 0, len(universe) - len(wanted)]
        got = [total, verdict, passed + failed + skipped + measured, failed, skipped, measured, filtered]
        labels = ['tests run', 'result', 'tests accounted for', 'failed', 'ignored', 'measured', 'filtered out']
        for label, value, want in zip(labels, got, expected):
            require(at + ' run', label, value, want)
        if not wanted <= named:
            raise Refused('%s run: no `test <name> ... ` line for %s' % (at, sample(wanted - named)))
        leg.universe[source] = universe
        leg.ignored[source] = hidden
        leg.selected[source] = wanted
        leg.passed += passed
        leg.lines.append('  %s %-22s %5d of %5d listed: %5d passed, %4d ignored' % (shard, source, len(wanted), len(universe), passed, skipped))
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
        if inner != FILES:
            problems.append('%s holds %r, not exactly %r' % (entry, inner, FILES))
            continue
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
        if not attempts:
            problems.append('%s: no record, so the leg did not finish and upload in this run' % shard)
            continue
        newest = max(attempts)
        if newest > attempt:
            problems.append('%s: a record from attempt %d, after this job\'s attempt %d' % (shard, newest, attempt))
            continue
        for older in sorted(attempts):
            if older != newest:
                print('partition: %s: attempt %d\'s record is superseded by attempt %d\'s' % (shard, older, newest))
        try:
            legs[shard] = read_leg(attempts[newest], shard, newest, table, env, head, blobs)
        except Refused as error:
            problems.append(str(error))
        except (OSError, KeyError, TypeError, AttributeError, ValueError) as error:
            problems.append('%s (attempt %d): an unreadable record: %r' % (shard, newest, error))

    print('partition: %s at %s, attempt %d: verifying %s' % (env['GITHUB_EVENT_NAME'], head, attempt, ', '.join(lane)))
    for group in lane:
        members = [legs[shard] for shard in table.order[group] if shard in legs]
        if len(members) != len(table.order[group]):
            problems.append('%s: not every leg is readable, so its coverage is not decided' % group)
            continue
        first = members[0]
        consistent = True
        for leg in members[1:]:
            if leg.targets != first.targets:
                problems.append('%s: %s built %r and %s built %r' % (group, first.shard, first.targets, leg.shard, leg.targets))
                consistent = False
                continue
            if leg.compiler != first.compiler:
                problems.append('%s: %s and %s ran different compilers' % (group, first.shard, leg.shard))
                consistent = False
            for source in first.targets:
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
        inventory = hashlib.sha256()
        for source in first.targets:
            for name in sorted(first.universe[source]):
                inventory.update(('%s\t%s\t%s\n' % (source, name, 'ignored' if name in first.ignored[source] else 'test')).encode('utf-8'))
        print('partition: %s: %d legs, compiler %s, inventory %s' % (
            group, len(members), dict(line.split(': ', 1) for line in first.compiler.splitlines() if ': ' in line).get('release'), inventory.hexdigest()))
        for leg in members:
            for line in leg.lines:
                print(line)
            if not any(leg.selected[source] for source in leg.targets):
                print('partition: notice: %s selects no test at all; the table no longer describes the code' % leg.shard)
        for source in first.targets:
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
