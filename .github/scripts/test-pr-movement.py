#!/usr/bin/env python3
"""Offline movement contracts: ordinary temporary history, no remote or enqueue calls."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("review_parser", ROOT / "scripts/pr-review-parse.py")
parser = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(parser)


class BaseContract(unittest.TestCase):
    def test_restored_files_match_reference_blobs(self):
        # Blob identities from 3b04c1d7. Compare bytes, without requiring history in CI's
        # shallow checkout or fetching anything from a remote.
        references = {
            ".github/scripts/validate-pr-body.sh": "b34ee6f5a09b8e7954b6df68c23250a6651676fd",
            ".github/scripts/validate-pr-ledger-evidence.sh": "c0a9827d11dd45c27fdc9c8ba4f9bacb83af1612",
            ".github/scripts/test-pr-policy.sh": "fb88e42d3de910e3ede74d7efa2fef6d689dd889",
            ".github/scripts/test-pr-ledger-evidence.sh": "831ee6335bbb4f6d60c49760d9e0d4d89bc5d6cc",
            "scripts/lane.sh": "4c516af4309565f30350b636f1b5e37066bafd03",
        }
        for path, reference in references.items():
            with self.subTest(path=path):
                data = (ROOT / path).read_bytes()
                if path == "scripts/lane.sh":
                    comment = (
                        b"# The exception is a `findings/` pull request that records a red: each pass also checks every fact\n"
                        b"# the record gives against the logs and the source (MAINTAINING.md; `findings/PROCESS.md`\n"
                        b"# section 10).\n"
                    )
                    self.assertEqual(data.count(comment), 1)
                    data = data.replace(comment, b"")
                header = ("blob %d\0" % len(data)).encode("ascii")
                self.assertEqual(hashlib.sha1(header + data).hexdigest(), reference)


class MovementContracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        self.old_cwd = Path.cwd()
        os.chdir(self.repo)
        self.addCleanup(os.chdir, self.old_cwd)
        self.environment = mock.patch.dict(os.environ, {
            "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "Movement Test", "GIT_AUTHOR_EMAIL": "test@example.invalid",
            "GIT_COMMITTER_NAME": "Movement Test", "GIT_COMMITTER_EMAIL": "test@example.invalid",
            "GIT_AUTHOR_DATE": "2001-01-01T00:00:00Z", "GIT_COMMITTER_DATE": "2001-01-01T00:00:00Z",
        })
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.git("init", "-q")
        self.write("source.txt", "source\n")
        self.base = self.commit("base")
        self.reviewed = self.base

    def git(self, *args):
        return subprocess.run(["git", *args], check=True, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE).stdout

    def write(self, path, text):
        target = self.repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")

    def commit(self, message):
        self.git("add", "--all")
        self.git("-c", "commit.gpgsign=false", "commit", "-qm", message)
        return self.git("rev-parse", "HEAD").decode("ascii").strip()

    def movement(self, head, reader=None):
        if reader is None:
            result = subprocess.run(
                [sys.executable, str(ROOT / "scripts/pr-review-parse.py"), "movement",
                 self.reviewed, head, self.base],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
            return json.loads(result.stdout)["moved"]
        with mock.patch.object(parser, "GitEvidence", return_value=reader):
            return parser.movement_result([self.reviewed, head, self.base])["moved"]

    def changes(self, head):
        reader = parser.GitEvidence()
        return [change for oid, commit in reader.branch_commits(self.reviewed, head, self.base)
                for change in reader.changes(oid, commit)]

    def test_add_then_edit_keeps_both_commit_records(self):
        self.write("findings/one.md", "first\n")
        addition = self.commit("add")
        self.assertEqual(self.movement(addition), "ledger-only")
        self.write("findings/one.md", "second\n")
        edit = self.commit("edit")
        self.assertEqual([(one.commit, one.status, one.path) for one in self.changes(edit)], [
            (edit, "M", b"findings/one.md"), (addition, "A", b"findings/one.md")])
        self.assertEqual(self.movement(edit), "ledger-rewrite")
        self.write("findings/one.md", "first\n")
        reverted = self.commit("restore addition's contents")
        self.assertEqual(self.movement(reverted), "ledger-rewrite")
        self.assertEqual(len(self.changes(reverted)), 3)

    def test_move_keeps_the_source_and_destination(self):
        self.write("findings/old.md", "finding\n")
        self.reviewed = self.commit("reviewed finding")
        (self.repo / "findings/old.md").rename(self.repo / "findings/new.md")
        head = self.commit("move")
        self.assertEqual({(one.status, one.path) for one in self.changes(head)}, {
            ("D", b"findings/old.md"), ("A", b"findings/new.md")})
        self.assertEqual(self.movement(head), "ledger-rewrite")
        (self.repo / "findings/new.md").rename(self.repo / "findings/old.md")
        back = self.commit("move back")
        self.assertEqual(len(self.changes(back)), 4)
        self.assertEqual(self.movement(back), "ledger-rewrite")

    def test_mixed_push_and_neighboring_additions(self):
        self.write("findings/one.md", "one\n")
        first = self.commit("first addition")
        self.write("findings/two.md", "two\n")
        second = self.commit("second addition")
        self.assertEqual(self.movement(first), "ledger-only")
        self.assertEqual(self.movement(second), "ledger-only")
        self.write("source.txt", "repair\n")
        self.assertEqual(self.movement(self.commit("repair")), "repairs")

    def test_archive_present_absent_and_wrong_type(self):
        reader = parser.GitEvidence()
        base_tree = reader.commit(self.base).tree
        self.assertEqual(reader.archive(base_tree, b"reviews/FINDINGS.md", parser.REGULAR_MODES), "absent")
        self.write("reviews/FINDINGS.md", "closed\n")
        self.write("reviews/findings/old.md", "old\n")
        self.reviewed = self.commit("historical archive")
        tree = reader.commit(self.reviewed).tree
        self.assertEqual(reader.archive(tree, b"reviews/FINDINGS.md", parser.REGULAR_MODES), "present")
        self.assertEqual(reader.archive(tree, b"reviews/findings", {b"40000"}), "present")
        self.assertEqual(reader.archive(tree, b"reviews/FINDINGS.md", {b"40000"}), "wrong-type")
        self.assertEqual(reader.archive(tree, b"reviews/findings", parser.REGULAR_MODES), "wrong-type")
        self.write("reviews/FINDINGS.md", "closed and edited\n")
        self.write("reviews/findings/new.md", "new\n")
        head = self.commit("historical edits")
        self.assertEqual(self.movement(head), "ledger-rewrite")
        self.reviewed = self.base
        self.assertEqual(self.movement(head), "ledger-rewrite")

    def test_archive_symlink_is_not_an_existing_file(self):
        (self.repo / "reviews").mkdir()
        (self.repo / "reviews/FINDINGS.md").symlink_to("../source.txt")
        self.reviewed = self.commit("archive is a link")
        reader = parser.GitEvidence()
        tree = reader.commit(self.reviewed).tree
        self.assertEqual(reader.archive(tree, b"reviews/FINDINGS.md", parser.REGULAR_MODES), "wrong-type")
        (self.repo / "reviews/FINDINGS.md").unlink()
        (self.repo / "reviews/FINDINGS.md").symlink_to("../elsewhere")
        self.assertEqual(self.movement(self.commit("edit link")), "ledger-rewrite")

    def test_archive_read_errors_are_not_absence(self):
        self.write("reviews/FINDINGS.md", "archive\n")
        self.reviewed = self.commit("archive")
        reader = parser.GitEvidence()
        tree = reader.commit(self.reviewed).tree
        archive_tree = reader.tree(tree)[b"reviews"].oid
        real_command = reader.command

        def fail(*args):
            if args == ("cat-file", "tree", archive_tree):
                raise OSError("injected EIO")
            return real_command(*args)

        with mock.patch.object(reader, "command", side_effect=fail):
            with self.assertRaises(OSError):
                reader.archive(tree, b"reviews/FINDINGS.md", parser.REGULAR_MODES)
        # A later successful read is positive evidence, not a cached failed absence.
        self.assertEqual(reader.archive(tree, b"reviews/FINDINGS.md", parser.REGULAR_MODES), "present")
        blob = reader.tree(archive_tree)[b"FINDINGS.md"].oid
        for failed_bytes in (b"", b"archive"):
            with mock.patch.object(reader, "command", side_effect=lambda *args:
                                   failed_bytes if args == ("cat-file", "blob", blob) else real_command(*args)):
                with self.assertRaises(parser.Unparsed):
                    reader.archive(tree, b"reviews/FINDINGS.md", parser.REGULAR_MODES)
        self.write("source.txt", "positive non-exempt repair\n")
        head = self.commit("repair alongside archive")
        with mock.patch.object(reader, "archive", side_effect=OSError("injected archive EIO")):
            with self.assertRaises(OSError):
                self.movement(head, reader)

    def test_successful_short_tree_even_at_record_boundary_is_indeterminate(self):
        self.write("findings/one.md", "one\n")
        self.write("z-repair.txt", "repair\n")
        head = self.commit("mixed")
        tree = self.git("rev-parse", head + "^{tree}").decode().strip()
        full = self.git("cat-file", "tree", tree)
        first_end = full.index(b"\0") + 21
        for partial in (b"", full[:first_end], full[:-1]):
            with self.subTest(partial=partial):
                reader = parser.GitEvidence()
                original = reader.command

                def short(*args):
                    return partial if args == ("cat-file", "tree", tree) else original(*args)

                with mock.patch.object(reader, "command", side_effect=short):
                    with self.assertRaises(parser.Unparsed):
                        self.movement(head, reader)
        self.assertEqual(self.movement(head), "repairs")

    def test_short_commit_cannot_omit_a_parent_or_later_edit(self):
        self.write("findings/one.md", "one\n")
        first = self.commit("add")
        self.write("findings/one.md", "two\n")
        head = self.commit("edit")
        full = self.git("cat-file", "commit", head)
        reader = parser.GitEvidence()
        original = reader.command
        # Well-formed but missing the parent: syntax alone would make this a root addition.
        missing_parent = full.replace(("parent " + first + "\n").encode(), b"")
        with mock.patch.object(reader, "command", side_effect=lambda *args:
                               missing_parent if args == ("cat-file", "commit", head) else original(*args)):
            with self.assertRaises(parser.Unparsed):
                self.movement(head, reader)
        self.assertEqual(self.movement(head), "ledger-rewrite")

    def test_every_record_and_type_is_validated(self):
        oid = b"\x01" * 20
        valid = b"100644 one\0" + oid
        self.assertEqual(parser.parse_tree(valid, 20), {b"one": parser.GitEntry(b"100644", "01" * 20)})
        for broken in (b"100644 one", valid[:-1], b"100644 \0" + oid,
                       b"100644 ../one\0" + oid, b"100644 .\0" + oid,
                       b"100644 ..\0" + oid, b"100644 /one\0" + oid,
                       b"100644 one\0" + b"\0" * 20, valid + valid,
                       valid + b"100644 unfinished", b"100644 one\0" + oid + b"\0"):
            with self.subTest(broken=broken), self.assertRaises(parser.Unparsed):
                parser.parse_tree(broken, 20)
        # Invalid status tokens have no input channel; arbitrary kinds/modes are refused instead.
        for mode in (b"A", b"AA", b"M100", b"R100", b"Z", b"100600", b"040000", b"1006440"):
            with self.subTest(mode=mode), self.assertRaises(parser.Unparsed):
                parser.parse_tree(mode + b" one\0" + oid, 20)
        for mode in parser.GIT_MODES:
            self.assertEqual(parser.parse_tree(mode + b" one\0" + oid, 20)[b"one"].mode, mode)

    def test_paths_are_bytes_and_moves_do_not_depend_on_quoting(self):
        for name in (b"tab\tname", b"line\nname", b'quote"name', b"nonutf8\xff"):
            raw = b"100644 " + name + b"\0" + b"\x01" * 20
            self.assertIn(name, parser.parse_tree(raw, 20))
        self.write("findings/tab\tname.md", "one\n")
        head = self.commit("quoted path")
        self.assertEqual(self.changes(head)[0].path, b"findings/tab\tname.md")
        self.assertEqual(self.movement(head), "ledger-rewrite")

    def test_resolution_commit_framing_and_process_failure(self):
        oid = self.base.encode("ascii")
        for data in (oid, oid + b"\0", oid + b"\n\n", oid + b"\nextra\n", b"\n"):
            reader = parser.GitEvidence()
            with mock.patch.object(reader, "command", return_value=data), self.assertRaises(parser.Unparsed):
                reader.resolve("HEAD")
        for data in (b"", b"tree " + oid + b"\n", b"tree " + oid + b"\nparent bad\n\n",
                     b"tree " + oid + b"\ntree " + oid + b"\n\n"):
            with self.subTest(data=data), self.assertRaises(parser.Unparsed):
                parser.parse_commit(data, 40)
        for status in (1, 128):
            response = subprocess.CompletedProcess([], status, b"partial", b"read failed")
            with mock.patch.object(parser.subprocess, "run", return_value=response):
                with self.assertRaises(parser.Unparsed):
                    parser.GitEvidence().command("cat-file", "tree", self.base)
        response = subprocess.CompletedProcess([], 128, b"", b"EIO")
        with mock.patch.object(parser.subprocess, "run", return_value=response):
            with self.assertRaises(parser.Unparsed):
                parser.GitEvidence().ancestor(self.base, self.base)

    def test_base_commits_and_merge_commits_stay_excluded(self):
        self.write("findings/one.md", "one\n")
        self.reviewed = self.commit("reviewed")
        self.git("checkout", "-qb", "base-side", self.base)
        self.write("base-only.txt", "base change\n")
        base_tip = self.commit("base moves")
        self.git("checkout", "-qb", "topic", self.reviewed)
        self.git("-c", "commit.gpgsign=false", "merge", "--no-ff", "-qm", "clean merge", base_tip)
        head = self.git("rev-parse", "HEAD").decode().strip()
        self.base = base_tip
        self.assertEqual(self.movement(head), "merges-only")
        self.write("findings/two.md", "two\n")
        self.assertEqual(self.movement(self.commit("addition after merge")), "ledger-only")

    def test_shell_reads_only_a_complete_typed_result(self):
        self.write("findings/one.md", "one\n")
        head = self.commit("addition")
        script = ('PR_READY_AUDIT_LIBRARY=1 source "$1"; moved=indeterminate; '
                  'read_movement "$2" "$3" "$4"; printf "%s" "$moved"')
        result = subprocess.run(["bash", "-c", script, "movement-test",
                                 str(ROOT / "scripts/pr-ready-audit.sh"), self.reviewed, head, self.base],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
        self.assertEqual(result.stdout, b"ledger-only")
        # Exercise the actual shell reader with successful but malformed helper output.
        helper = self.repo / "helper.py"
        for payload in (b"", b"movement\0ledger-only\0", b"movement\0ledger-only\x000",
                        b"movement\0unknown\x000\0", b"movement\0repairs\x000\0extra\0",
                        b"movement\0ledger-only\x001\0", b"movement\0ledger-only\x000\0unfinished"):
            helper.write_text("import pathlib, sys\npathlib.Path(sys.argv[4]).write_bytes(%r)\n" % payload)
            refusal = ('PR_READY_AUDIT_LIBRARY=1 source "$1"; review_parser="$5"; '
                       'moved=indeterminate; if read_movement "$2" "$3" "$4"; then exit 99; fi; '
                       'printf "%s" "$moved"')
            result = subprocess.run(["bash", "-c", refusal, "movement-test",
                                     str(ROOT / "scripts/pr-ready-audit.sh"), self.reviewed, head,
                                     self.base, str(helper)], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, b"indeterminate")
        helper.write_text("import pathlib, sys\npathlib.Path(sys.argv[4]).write_bytes("
                          "b'movement\\0ledger-only\\x000\\0')\nsys.exit(1)\n")
        result = subprocess.run(["bash", "-c", refusal, "movement-test",
                                 str(ROOT / "scripts/pr-ready-audit.sh"), self.reviewed, head,
                                 self.base, str(helper)], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, b"indeterminate")


if __name__ == "__main__":
    unittest.main()
