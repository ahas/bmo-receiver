//! A test that sets what stands for the whole process — the environment, a declaration read once —
//! run alone in a process of its own: this binary started again on that one test, while every other
//! test of the binary shares the process the harness started.

use std::path::PathBuf;
use std::process::Command;

// The variable naming, in the process started for one test, the file its run marks once it passed.
const MARK: &str = "BMO_TEST_ALONE_MARK";

/// Kept by a test to its end in its own process: marks the run done as it goes, unless the test
/// panicked, so the process that started it knows the test ran rather than matched nothing.
pub struct AloneRun(PathBuf);

impl Drop for AloneRun {
  fn drop(&mut self) {
    if !std::thread::panicking() {
      std::fs::write(&self.0, "").unwrap();
    }
  }
}

/// The test named `test` in `module` (its `module_path!()`), alone in a process of its own. In that
/// process this hands back the guard the test keeps to its end; in the one the harness started it
/// runs that process, asserts the test ran and passed there, and hands back nothing, so the test
/// returns.
pub fn alone(module: &str, test: &str) -> Option<AloneRun> {
  if let Some(mark) = std::env::var_os(MARK) {
    return Some(AloneRun(mark.into()));
  }

  // The harness names a test by its path under the crate, which `module_path!()` begins with.
  let under = module.split_once("::").map_or(module, |(_, rest)| rest);
  let name = format!("{under}::{test}");
  let mark = std::env::temp_dir().join(format!("bmo-test-alone-{}-{test}", std::process::id()));

  let _ = std::fs::remove_file(&mark);

  // Its output is kept rather than passed through, so the harness's lines of that run are never read
  // as this binary's own.
  let ran = Command::new(std::env::current_exe().unwrap())
    .args([name.as_str(), "--exact", "--nocapture"])
    .env(MARK, &mark)
    .output()
    .unwrap();
  let said = format!("{}{}", String::from_utf8_lossy(&ran.stdout), String::from_utf8_lossy(&ran.stderr));

  assert!(ran.status.success(), "{name} failed in a process of its own:\n{said}");
  assert!(
    std::fs::remove_file(&mark).is_ok(),
    "{name} did not run in a process of its own:\n{said}"
  );

  None
}
