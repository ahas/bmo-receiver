//! The repository carries no personal data (docs/PERSONAL-DATA.md): every tracked file is read once and held to
//! the five kinds the document names, with the file, the line and the kind reported and never the value.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;

/// One finding: where it stands and which kind it is, never the value that matched.
#[derive(Debug, PartialEq, Eq)]
struct Finding {
  file: String,
  line: usize,
  kind: &'static str,
}

impl std::fmt::Display for Finding {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(out, "{}:{}: {}", self.file, self.line, self.kind)
  }
}

// The five kinds of docs/PERSONAL-DATA.md and what each line is read for. Every pattern is written so that this
// file's own source is no sample of the thing it looks for; a sample fixture is assembled instead of written out.
const PATTERNS: &[(&str, &str)] = &[
  ("mail", r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}"),
  ("home-path", r"(?:^|[^A-Za-z0-9._-])/(?:home|root|Users)/[A-Za-z0-9._-]+"),
  ("home-path", r"[A-Za-z]:[\\/]Users[\\/][A-Za-z0-9._-]+"),
  ("telephone", r"\b01[016789][-\s]?\d{3,4}[-\s]?\d{4}\b"),
  ("telephone", r"\b0(?:2|[3-6][1-5])[-\s]?\d{3,4}[-\s]?\d{4}\b"),
  ("telephone", r"\+\d{1,3}[-\s]\d{2,4}[-\s]\d{3,4}[-\s]\d{4}\b"),
  ("telephone", r"\b\d{3}-\d{3}-\d{4}\b"),
  ("telephone", r"\(\d{3}\)\s*\d{3}-\d{4}"),
  ("token", r"\bgh[pousr]_[A-Za-z0-9]{20,}\b"),
  ("token", r"\bgithub_pat_[A-Za-z0-9_]{20,}\b"),
  ("token", r"\bglpat-[A-Za-z0-9_-]{20,}\b"),
  ("token", r"\bAKIA[0-9A-Z]{16}\b"),
  ("token", r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b"),
  ("token", r"\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{16,}\b"),
  ("token", r"\bsk-[A-Za-z0-9_-]{20,}\b"),
  ("token", r"\bAIza[0-9A-Za-z_-]{35}\b"),
  ("token", r"\bnpm_[A-Za-z0-9]{36}\b"),
  ("token", r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b"),
  ("token", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
  ("token", r"\bBearer\s+[A-Za-z0-9._~+/-]{20,}=*\b"),
  ("private-network", r"\b\d{1,3}(?:\.\d{1,3}){3}\b"),
  ("private-network", r"(?i)\b(?:fc|fd)[0-9a-f]{2}:[0-9a-f:]{2,}"),
  ("private-network", r"(?i)\bfe[89ab][0-9a-f]:[0-9a-f:]{2,}"),
];

// The domains RFC 2606 and RFC 6761 reserve for documentation: an address there names no person and no machine.
const DOCUMENTATION_DOMAINS: &[&str] = &["example.com", "example.net", "example.org"];
const DOCUMENTATION_SUFFIXES: &[&str] = &[".example", ".test", ".invalid", ".localhost"];

fn rules() -> &'static [(&'static str, Regex)] {
  static RULES: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();

  RULES.get_or_init(|| {
    PATTERNS
      .iter()
      .map(|(kind, pattern)| (*kind, Regex::new(pattern).expect("a personal-data pattern reads")))
      .collect()
  })
}

/// Whether a mail address names the documentation space rather than a person or a machine.
fn is_documentation_address(address: &str) -> bool {
  let Some((_, domain)) = address.rsplit_once('@') else { return false };
  let domain = domain.trim_end_matches('.').to_ascii_lowercase();
  let documented = DOCUMENTATION_DOMAINS
    .iter()
    .any(|base| domain == *base || domain.ends_with(&format!(".{base}")));

  documented
    || domain == "localhost"
    || DOCUMENTATION_SUFFIXES.iter().any(|suffix| domain.ends_with(suffix))
}

// A private address is the IPv4 space of RFC 1918, the shared space carrier-grade and overlay networks hand out,
// and the link-local block, or an IPv6 address of the unique local or link-local prefixes: the reads that reach
// this answer are already narrowed to those prefixes. The loopback address is left out, since it names the
// machine the program runs on and no host on a network.
fn is_private_address(address: &str) -> bool {
  if address.contains(':') {
    return true;
  }

  let octets: Vec<u8> = address.split('.').filter_map(|part| part.parse().ok()).collect();
  let [first, second, ..] = octets.as_slice() else { return false };

  octets.len() == 4
    && (*first == 10
      || (*first == 172 && (16..=31).contains(second))
      || (*first == 192 && *second == 168)
      || (*first == 169 && *second == 254)
      || (*first == 100 && (64..=127).contains(second)))
}

fn kept(kind: &str, value: &str) -> bool {
  match kind {
    "mail" => !is_documentation_address(value),
    "private-network" => is_private_address(value),
    _ => true,
  }
}

fn findings_in(file: &str, text: &str) -> Vec<Finding> {
  let mut findings = Vec::new();

  for (number, line) in text.lines().enumerate() {
    let mut kinds: Vec<&'static str> = Vec::new();

    for (kind, rule) in rules() {
      for found in rule.find_iter(line) {
        if kept(kind, found.as_str()) && !kinds.contains(kind) {
          kinds.push(kind);
        }
      }
    }

    findings.extend(kinds.into_iter().map(|kind| Finding {
      file: file.to_owned(),
      line: number + 1,
      kind,
    }));
  }

  findings
}

fn reported(findings: &[Finding]) -> String {
  findings.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
}

/// Every file git tracks or would track: the committed ones and the new ones it does not ignore.
fn tracked_files() -> Vec<String> {
  let root = Path::new(env!("CARGO_MANIFEST_DIR"));
  let output = Command::new("git")
    .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard"])
    .current_dir(root)
    .output()
    .expect("git ls-files runs");
  assert!(
    output.status.success(),
    "git ls-files failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );

  let mut files: Vec<String> = String::from_utf8(output.stdout)
    .expect("tracked paths are UTF-8")
    .split('\0')
    .filter(|path| !path.is_empty() && root.join(path).is_file())
    .map(str::to_owned)
    .collect();
  files.sort();
  files.dedup();
  files
}

#[test]
fn no_tracked_file_carries_personal_data() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut findings = Vec::new();

  for file in tracked_files() {
    let bytes = std::fs::read(root.join(&file)).expect("a tracked file is readable");
    findings.extend(findings_in(&file, &String::from_utf8_lossy(&bytes)));
  }

  assert!(findings.is_empty(), "personal data in tracked files:\n{}", reported(&findings));
}

#[test]
fn every_kind_is_caught_and_reported_without_its_value() {
  let text = concat!(
    "mail ",
    "qa",
    "@",
    "mail",
    ".",
    "dev",
    "\n",
    "home ",
    "/",
    "home",
    "/",
    "tester",
    "\n",
    "call ",
    "010",
    "-1234-5678",
    "\n",
    "token ",
    "ghp_",
    "abcdefghijklmnopqrstuvwxyz0123456789",
    "\n",
    "host ",
    "192",
    ".",
    "168",
    ".",
    "1",
    ".",
    "2",
    "\n",
  );
  let findings = findings_in("fixture.txt", text);

  assert_eq!(
    reported(&findings),
    [
      "fixture.txt:1: mail",
      "fixture.txt:2: home-path",
      "fixture.txt:3: telephone",
      "fixture.txt:4: token",
      "fixture.txt:5: private-network",
    ]
    .join("\n")
  );
  assert!(!reported(&findings).contains("1234"));
  assert!(!reported(&findings).contains("192"));
}

#[test]
fn a_documentation_address_and_the_loopback_are_left_alone() {
  assert!(findings_in("fixture", "Written by tester@example.com.\n").is_empty());

  let loopback = concat!("http://", "127", ".", "0", ".", "0", ".", "1", ":3296");

  assert!(findings_in("fixture", loopback).is_empty());
}

#[test]
fn a_windows_home_path_is_caught() {
  let text = concat!("C:\\", "Users\\", "tester\\notes.txt");

  assert_eq!(reported(&findings_in("fixture", text)), "fixture:1: home-path");
}

#[test]
fn versions_hashes_and_timestamps_are_no_finding() {
  let text = concat!(
    "vaul-vue/@vueuse/core@10.11.1\n",
    "base aec4a4019858b6e672825419987f96fab3f09f41\n",
    "changed 1792516919062\n",
    "sha512-guoy26JQktXPcz+0n3GukWIy/JDNkKti9v6VEMu6kV2sYBsWuGiTU8OWdg+ADfUbHg3/3DlqySDe7JmdHrktiww==\n",
  );

  assert!(findings_in("fixture", text).is_empty());
}
