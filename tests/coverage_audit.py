"""Mutation tests for the production coverage gate's inventory and patch evidence."""
import contextlib
import io
import json
import runpy
import tempfile
import unittest
from pathlib import Path

verify = runpy.run_path(str(Path(__file__).resolve().parents[1] / "tools/check-coverage.py"))["verify"]
ENGINE = "third_party/hron/src/eval/mod.rs"
EXTENSION = "third_party/hron/src/eval/captured.rs"
MODULES = ("src/lib.rs", "src/recurrence.rs", "src/recurrence/abi.rs", EXTENSION)


class CoverageAudit(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.enterContext(contextlib.chdir(directory.name))
        self.out = Path("report")
        self.out.mkdir()
        for name in (*MODULES, ENGINE, "src/abi_tests.rs", "src/recurrence/tests.rs"):
            path = Path(name)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fn test() {}\n")
        Path("src/lib.rs").write_text("#[cfg(test)]\nmod abi_tests;\nmod recurrence;\n")
        Path("src/recurrence.rs").write_text("#[cfg(test)]\nmod tests;\nmod abi;\n")
        Path("third_party/hron-2.2.1.patch").write_text(
            f"--- a/{ENGINE}\n+++ b/{ENGINE}\n@@ -1,1 +1,3 @@\n fn check() {{\n"
            "+    if !work.charge(1) { return; }\n+    calculate();\n"
        )
        summary = {k: {"count": 2, "covered": 2} for k in ("lines", "branches")}
        self.files = [
            {"filename": name, "summary": summary, "segments": [], "branches": []}
            for name in MODULES
        ]
        self.engine = {
            "filename": ENGINE, "summary": summary,
            "segments": [[2, 5, 3, True, True, False], [3, 17, 0, False, False, False]],
            "branches": [[2, 8, 2, 23, 1, 2, 0, 0, 4]],
        }
        self.files.append(self.engine)
        self.lcov = [f"SF:{ENGINE}", "DA:2,3", "DA:3,3", "BRDA:2,0,0,1",
                     "BRDA:2,0,1,2", "end_of_record"]

    def failed(self):
        (self.out / "coverage.json").write_text(json.dumps({"data": [{"files": self.files}]}))
        (self.out / "coverage.lcov").write_text("\n".join(self.lcov)+"\n")
        with contextlib.redirect_stderr(io.StringIO()), contextlib.redirect_stdout(io.StringIO()):
            return verify(self.out)

    def test_complete_report(self):
        self.assertFalse(self.failed())

    def test_missing_provider_module(self):
        self.files.pop(2)
        self.assertTrue(self.failed())

    def test_missing_patch_branches(self):
        self.lcov = [row for row in self.lcov if not row.startswith("BRDA:")]
        self.assertTrue(self.failed())

    def test_missing_one_branch_arm(self):
        self.lcov.remove("BRDA:2,0,1,2")
        self.assertTrue(self.failed())

    def test_missing_json_branch_sites(self):
        self.engine["branches"] = []
        self.assertTrue(self.failed())

    def test_missing_every_branch_record(self):
        self.engine["branches"] = []
        self.lcov = [row for row in self.lcov if not row.startswith("BRDA:")]
        self.assertTrue(self.failed())

    def test_missing_wrapped_added_line(self):
        self.lcov.remove("DA:3,3")
        self.assertTrue(self.failed())

    def test_uncovered_added_line_and_branch(self):
        for original, replacement in (("DA:3,3", "DA:3,0"), ("BRDA:2,0,0,1", "BRDA:2,0,0,-")):
            with self.subTest(row=original):
                pos = self.lcov.index(original)
                self.lcov[pos] = replacement
                self.assertTrue(self.failed())
                self.lcov[pos] = original


if __name__ == "__main__":
    unittest.main()
