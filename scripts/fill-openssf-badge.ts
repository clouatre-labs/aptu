// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 Aptu Contributors

import { chromium, type Page } from "@playwright/test";
import * as readline from "node:readline";

interface Answer {
  id: string;
  status: "Met" | "Unmet" | "N/A" | "?";
  justification: string;
}

// All criterion answers for aptu (project ID 11662).
// Criteria with status "?" are skipped (not set).
const ANSWERS: Answer[] = [
  // --- Passing level ---
  {
    id: "description_good",
    status: "Met",
    justification:
      "README opens with "Aptu AI-powered CLI for issue triage and PR review with gamification." The GitHub repository description matches. URL: https://github.com/clouatre-labs/aptu#readme",
  },
  {
    id: "interact",
    status: "Met",
    justification:
      "GitHub Issues are enabled for bug reports. README links CONTRIBUTING.md, SECURITY.md, and the GitHub Marketplace listing. URL: https://github.com/clouatre-labs/aptu#readme",
  },
  {
    id: "contribution",
    status: "Met",
    justification:
      "CONTRIBUTING.md at the repository root documents the fork/PR workflow, commit signing, and PR checklist. URL: https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md",
  },
  {
    id: "contribution_requirements",
    status: "Met",
    justification:
      "CONTRIBUTING.md specifies coding standards (Rust 2024, thiserror/anyhow), commit signing (GPG + DCO), conventional commits, and test requirements. URL: https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md",
  },
  {
    id: "floss_license",
    status: "Met",
    justification:
      "Apache-2.0 license; OSI-approved. LICENSE file present at repository root. URL: https://github.com/clouatre-labs/aptu/blob/main/LICENSE",
  },
  {
    id: "floss_license_osi",
    status: "Met",
    justification:
      "Apache-2.0 is on the OSI approved list. URL: https://opensource.org/license/apache-2-0",
  },
  {
    id: "license_location",
    status: "Met",
    justification:
      "LICENSE file at repository root. URL: https://github.com/clouatre-labs/aptu/blob/main/LICENSE",
  },
  {
    id: "documentation_basics",
    status: "Met",
    justification:
      "README documents installation, usage tiers, inputs, outputs, troubleshooting, and supported platforms. URL: https://github.com/clouatre-labs/aptu#readme",
  },
  {
    id: "documentation_interface",
    status: "Met",
    justification:
      "README documents all inputs (repo, state, since, dry-run, no-apply, no-comment, force) and outputs (JSON/text) with descriptions and examples. URL: https://github.com/clouatre-labs/aptu#readme",
  },
  {
    id: "sites_https",
    status: "Met",
    justification:
      "All project URLs use HTTPS: GitHub repository, GitHub Marketplace, SECURITY.md links. URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "discussion",
    status: "Met",
    justification:
      "GitHub Issues are searchable, URL-addressable, and publicly accessible without proprietary software. URL: https://github.com/clouatre-labs/aptu/issues",
  },
  {
    id: "english",
    status: "Met",
    justification:
      "All documentation, issue templates, and code comments are in English. URL: https://github.com/clouatre-labs/aptu#readme",
  },
  {
    id: "maintained",
    status: "Met",
    justification:
      "Active development: Renovate bot runs weekly; v0.1.0 released 2026-01-08; multiple CI and security improvements merged in March 2026. URL: https://github.com/clouatre-labs/aptu/releases",
  },
  {
    id: "repo_public",
    status: "Met",
    justification:
      "Public GitHub repository at https://github.com/clouatre-labs/aptu. URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "repo_track",
    status: "Met",
    justification:
      "Git version control with full commit history, branches, and tags. URL: https://github.com/clouatre-labs/aptu/commits/main",
  },
  {
    id: "repo_interim",
    status: "Met",
    justification:
      "Multiple commits and PRs between releases, all visible in the public commit history. URL: https://github.com/clouatre-labs/aptu/commits/main",
  },
  {
    id: "repo_distributed",
    status: "Met",
    justification:
      "Git is a distributed VCS; any clone is a full copy of the repository history. URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "version_unique",
    status: "Met",
    justification:
      "Releases are tagged with unique semver tags (v0.1.0, v0.1.1, v0.1.2, v0.1.3, v0.1.4). URL: https://github.com/clouatre-labs/aptu/tags",
  },
  {
    id: "version_semver",
    status: "Met",
    justification:
      "CONTRIBUTING.md explicitly states: \"We follow SemVer: MAJOR (breaking), MINOR (new features), PATCH (fixes).\" URL: https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md",
  },
  {
    id: "version_tags",
    status: "Met",
    justification:
      "Every release is tagged in Git (v0.1.0 through v0.1.4). Tags are annotated and GPG-signed. URL: https://github.com/clouatre-labs/aptu/tags",
  },
  {
    id: "release_notes",
    status: "Met",
    justification:
      "GitHub Releases page documents What's Changed for each release with feature, bug fix, and infrastructure categories. URL: https://github.com/clouatre-labs/aptu/releases",
  },
  {
    id: "release_notes_vulns",
    status: "Met",
    justification:
      "No CVE-assigned vulnerabilities have been fixed to date. When a vulnerability fix is released, SECURITY.md policy requires documenting it in the release notes. URL: https://github.com/clouatre-labs/aptu/releases",
  },
  {
    id: "report_url",
    status: "Met",
    justification:
      "SECURITY.md at the repository root documents the vulnerability reporting process. README has a Security Policy badge linking directly to SECURITY.md. URL: https://github.com/clouatre-labs/aptu/blob/main/SECURITY.md",
  },
  {
    id: "report_process",
    status: "Met",
    justification:
      "GitHub Issues are enabled. SECURITY.md documents the private reporting process via GitHub Security Advisories. README links SECURITY.md. URL: https://github.com/clouatre-labs/aptu/blob/main/SECURITY.md",
  },
  {
    id: "report_tracker",
    status: "Met",
    justification:
      "GitHub Issues used as the primary tracker for bugs, enhancements, and CI fixes. URL: https://github.com/clouatre-labs/aptu/issues",
  },
  {
    id: "report_responses",
    status: "Met",
    justification:
      "All filed issues have been addressed and closed with fixes (e.g., #93 cache flakiness, #59 version validation, #52 check-latest). URL: https://github.com/clouatre-labs/aptu/issues?q=is%3Aclosed",
  },
  {
    id: "enhancement_responses",
    status: "Met",
    justification:
      "Enhancement requests are tracked as GitHub Issues and addressed in releases (e.g., #52 check-latest, #63 cache-hit output, #62 cache restore-keys all implemented). URL: https://github.com/clouatre-labs/aptu/issues?q=label%3Aenhancement+is%3Aclosed",
  },
  {
    id: "report_archive",
    status: "Met",
    justification:
      "GitHub Issues are publicly readable and searchable indefinitely. URL: https://github.com/clouatre-labs/aptu/issues",
  },
  {
    id: "vulnerability_report_process",
    status: "Met",
    justification:
      "SECURITY.md at repository root documents the vulnerability reporting process. URL: https://github.com/clouatre-labs/aptu/blob/main/SECURITY.md",
  },
  {
    id: "vulnerability_report_private",
    status: "Met",
    justification:
      "SECURITY.md instructs reporters to use GitHub's private vulnerability reporting. Private vulnerability reporting is enabled on the repository (Security Advisories tab). URL: https://github.com/clouatre-labs/aptu/blob/main/SECURITY.md",
  },
  {
    id: "vulnerability_report_response",
    status: "Met",
    justification:
      "SECURITY.md defines response SLA: acknowledgement within 48 hours for critical/high. The project is actively maintained with same-day or next-day responses to issues. URL: https://github.com/clouatre-labs/aptu/blob/main/SECURITY.md",
  },
  {
    id: "build",
    status: "Met",
    justification:
      "cargo build --workspace --locked builds the entire workspace. URL: https://github.com/clouatre-labs/aptu/blob/main/Cargo.toml",
  },
  {
    id: "build_common_tools",
    status: "Met",
    justification:
      "Cargo (Rust standard build system) is used. URL: https://doc.rust-lang.org/cargo/",
  },
  {
    id: "build_floss_tools",
    status: "Met",
    justification:
      "All build tools are FLOSS (cargo, rustc, tokio, etc.). URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "test",
    status: "Met",
    justification:
      "cargo nextest run --workspace --locked and bats integration tests cover all main code paths. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "test_invocation",
    status: "Met",
    justification:
      "Tests are invoked via `cargo nextest run --workspace --locked` or `just test`. URL: https://github.com/clouatre-labs/aptu/blob/main/Justfile",
  },
  {
    id: "test_most",
    status: "Met",
    justification:
      "Unit tests + bats integration tests cover all main code paths. URL: https://github.com/clouatre-labs/aptu/blob/main/tests",
  },
  {
    id: "test_continuous_integration",
    status: "Met",
    justification:
      "CI runs on every PR via .github/workflows/ci.yml. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "test_policy",
    status: "Met",
    justification:
      "CONTRIBUTING.md documents the test requirements. The PR template includes a Test Plan section and CI verification checklist. URL: https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md",
  },
  {
    id: "tests_are_added",
    status: "Met",
    justification:
      "The PR template requires a Test Plan section. Recent PRs adding new inputs each included corresponding test jobs in ci.yml. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/PULL_REQUEST_TEMPLATE.md",
  },
  {
    id: "tests_documented_added",
    status: "Met",
    justification:
      "The PR template requires a Test Plan section documenting what was tested. CONTRIBUTING.md covers test requirements. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/PULL_REQUEST_TEMPLATE.md",
  },
  {
    id: "warnings",
    status: "Met",
    justification:
      "Zizmor runs on every PR and flags GitHub Actions security issues. actionlint validates workflow YAML syntax. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "warnings_fixed",
    status: "Met",
    justification:
      "Zizmor is a required CI check; a failing zizmor result blocks merge via the CI Result gate. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "warnings_strict",
    status: "Met",
    justification:
      "Zizmor is run with annotations=true and min-severity=high. All flagged issues must be resolved before merge. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "know_secure_design",
    status: "Met",
    justification:
      "Evidence: SHA-pinned GitHub Actions (zizmor enforcement), zizmor workflow security scanning on every PR, branch protection with signed commits, minimal permissions (contents: read), no pull_request_target triggers. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "know_common_errors",
    status: "Met",
    justification:
      "README Security Patterns section documents prompt injection risk with three defensive tiers. ASSURANCE.md covers MITM, cache poisoning, secret leakage, and dependency confusion. URL: https://github.com/clouatre-labs/aptu/blob/main/docs/ASSURANCE.md",
  },
  {
    id: "crypto_published",
    status: "N/A",
    justification:
      "Not applicable -- the project does not implement cryptography. Binary downloads use HTTPS (handled by curl/GitHub infrastructure).",
  },
  {
    id: "crypto_call",
    status: "N/A",
    justification:
      "Not applicable -- the project does not call cryptographic functions directly.",
  },
  {
    id: "crypto_floss",
    status: "N/A",
    justification:
      "Not applicable -- the project does not implement or bundle cryptographic code.",
  },
  {
    id: "crypto_keylength",
    status: "N/A",
    justification:
      "Not applicable -- the project does not manage cryptographic keys.",
  },
  {
    id: "crypto_working",
    status: "N/A",
    justification:
      "Not applicable -- the project does not use cryptography in its logic.",
  },
  {
    id: "crypto_weaknesses",
    status: "N/A",
    justification:
      "Not applicable -- the project does not implement cryptography.",
  },
  {
    id: "crypto_pfs",
    status: "N/A",
    justification:
      "Not applicable -- the project does not make TLS connections; curl is invoked by the runner; the project does not manage TLS sessions.",
  },
  {
    id: "crypto_password_storage",
    status: "N/A",
    justification:
      "Not applicable -- the project does not store or handle passwords or credentials.",
  },
  {
    id: "crypto_random",
    status: "N/A",
    justification:
      "Not applicable -- the project does not generate random numbers for security purposes.",
  },
  {
    id: "delivery_mitm",
    status: "Met",
    justification:
      "All binaries are downloaded exclusively via HTTPS from crates.io and GitHub releases. curl is called with -fsSL which fails on redirects to non-HTTPS URLs. URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "delivery_unsigned",
    status: "Met",
    justification:
      "All download URLs use HTTPS. The release page provides checksums. The build validates version format before downloading. URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "vulnerabilities_critical_fixed",
    status: "Met",
    justification:
      "No critical vulnerabilities have been reported or identified. The project actively monitors upstream dependencies via Renovate. URL: https://github.com/clouatre-labs/aptu/security",
  },
  {
    id: "vulnerabilities_critical_fixed_rapid",
    status: "Met",
    justification:
      "No critical vulnerabilities have been reported or identified. SECURITY.md defines a 14-day remediation SLA for critical/high findings, ensuring rapid response. URL: https://github.com/clouatre-labs/aptu/blob/main/SECURITY.md",
  },
  {
    id: "vulnerabilities_fixed_60_days",
    status: "Met",
    justification:
      "No open vulnerabilities. SECURITY.md defines a 14-day remediation SLA for critical/high severity findings. URL: https://github.com/clouatre-labs/aptu/blob/main/SECURITY.md",
  },
  {
    id: "no_leaked_credentials",
    status: "Met",
    justification:
      "No credentials, API keys, or secrets in the repository. Zizmor workflow security scanning flags secret injection patterns. The project does not handle API keys -- callers supply them via secrets. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "static_analysis",
    status: "Met",
    justification:
      "Zizmor runs on every PR and scans GitHub Actions workflows for security issues. actionlint validates workflow YAML structure. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "static_analysis_fixed",
    status: "Met",
    justification:
      "Zizmor is a required CI check; a failing zizmor result blocks merge via the CI Result gate. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "static_analysis_often",
    status: "Met",
    justification:
      "Zizmor runs on every pull request and every push to main. Renovate PRs also trigger the full CI suite weekly. URL: https://github.com/clouatre-labs/aptu/actions",
  },
  {
    id: "static_analysis_common_vulnerabilities",
    status: "Met",
    justification:
      "Zizmor specifically checks for GitHub Actions security vulnerabilities (template injection, dangerous permissions, unpinned actions). actionlint validates workflow structure. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "dynamic_analysis",
    status: "Met",
    justification:
      "The CI test suite exercises the CLI end-to-end on real GitHub Actions runners: it builds, runs unit tests, integration tests, and validates all outputs. This constitutes dynamic analysis of the project's behavior. URL: https://github.com/clouatre-labs/aptu/blob/main/.github/workflows/ci.yml",
  },
  {
    id: "dynamic_analysis_unsafe",
    status: "N/A",
    justification:
      "Not applicable -- the project is written in Rust, a memory-safe language, and does not use unsafe blocks.",
  },
  {
    id: "dynamic_analysis_enable_assertions",
    status: "Met",
    justification:
      "The test suite uses assert! macros and cargo test runs with debug assertions enabled. URL: https://github.com/clouatre-labs/aptu/blob/main/tests",
  },
  {
    id: "dynamic_analysis_fixed",
    status: "Met",
    justification:
      "Any findings from dynamic analysis are fixed before merge per CONTRIBUTING.md. URL: https://github.com/clouatre-labs/aptu/blob/main/CONTRIBUTING.md",
  },
  {
    id: "hardening",
    status: "Met",
    justification:
      "Rust memory safety; cargo deny; REUSE/SPDX; zizmor; gitleaks; SLSA Level 3; cosign; SHA-pinned Actions; branch protection. URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "crypto_used_network",
    status: "Met",
    justification:
      "All network connections (GitHub API, AI providers) use TLS 1.2+ via rustls. URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "crypto_tls12",
    status: "Met",
    justification:
      "rustls enforces TLS 1.2 minimum for all outbound connections. URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "crypto_certificate_verification",
    status: "Met",
    justification:
      "rustls verifies TLS certificates via webpki. URL: https://github.com/clouatre-labs/aptu",
  },
  {
    id: "crypto_verification_private",
    status: "N/A",
    justification:
      "Not applicable -- the project does not connect to private internal hosts.",
  },
];

// Criteria to skip (computed fields, or criteria not presented on the form).
const SKIP_IDS = new Set([
  "achieve_passing_status",
  "achieve_silver_status",
  "hardened_site",
]);

// Passing-ONLY criterion IDs (level 0 only -- NOT re-presented on the silver form).
// Derived directly from criteria/criteria.yml level '0', minus DUAL_IDS.
const PASSING_ONLY_IDS = new Set([
  // Basics
  "description_good", "interact", "contribution",
  "floss_license", "floss_license_osi", "license_location",
  "documentation_basics", "documentation_interface", "sites_https",
  "discussion", "english", "maintained",
  // Change Control
  "repo_public", "repo_track", "repo_interim", "repo_distributed",
  "version_unique", "version_semver", "version_tags",
  "release_notes", "release_notes_vulns",
  // Reporting
  "report_process", "report_responses",
  "enhancement_responses", "report_archive",
  "vulnerability_report_process", "vulnerability_report_private",
  "vulnerability_report_response",
  // Quality
  "build", "build_common_tools", "build_floss_tools",
  "test", "test_invocation", "test_most", "test_continuous_integration",
  "test_policy", "tests_are_added",
  "warnings", "warnings_fixed",
  "know_secure_design", "know_common_errors",
  // Security
  "crypto_published", "crypto_call", "crypto_floss", "crypto_keylength",
  "crypto_working", "crypto_pfs",
  "crypto_password_storage", "crypto_random",
  "delivery_mitm", "delivery_unsigned",
  "vulnerabilities_fixed_60_days", "vulnerabilities_critical_fixed",
  "no_leaked_credentials",
  // Analysis
  "static_analysis", "static_analysis_fixed", "static_analysis_often",
  "dynamic_analysis", "dynamic_analysis_enable_assertions", "dynamic_analysis_fixed",
]);

// Criteria that appear on BOTH passing and silver forms.
const DUAL_IDS = new Set([
  "contribution_requirements",
  "report_tracker",
  "tests_documented_added",
  "warnings_strict",
  "static_analysis_common_vulnerabilities",
  "dynamic_analysis_unsafe",
  "crypto_weaknesses",
]);

function passingAnswers(): Answer[] {
  return ANSWERS.filter(
    (a) => (PASSING_ONLY_IDS.has(a.id) || DUAL_IDS.has(a.id)) && !SKIP_IDS.has(a.id) && a.status !== "?"
  );
}

function silverAnswers(): Answer[] {
  return ANSWERS.filter(
    (a) => !PASSING_ONLY_IDS.has(a.id) && !SKIP_IDS.has(a.id) && a.status !== "?"
  );
}

// Passing form section structure (matches _form_0.html.erb accordion order, criteria from criteria.yml level 0).
// Each section has a Save-and-Continue button; must be submitted section-by-section.
const PASSING_SECTIONS: Array<{ name: string; continueValue: string; ids: string[] }> = [
  {
    name: "Basics",
    continueValue: "changecontrol",
    ids: [
      "description_good",
      "interact",
      "contribution",
      "contribution_requirements",
      "floss_license",
      "floss_license_osi",
      "license_location",
      "documentation_basics",
      "documentation_interface",
      "sites_https",
      "discussion",
      "english",
      "maintained",
    ],
  },
  {
    name: "Change Control",
    continueValue: "reporting",
    ids: [
      "repo_public",
      "repo_track",
      "repo_interim",
      "repo_distributed",
      "version_unique",
      "version_semver",
      "version_tags",
      "release_notes",
      "release_notes_vulns",
    ],
  },
  {
    name: "Reporting",
    continueValue: "quality",
    ids: [
      "report_process",
      "report_tracker",
      "report_responses",
      "enhancement_responses",
      "report_archive",
      "vulnerability_report_process",
      "vulnerability_report_private",
      "vulnerability_report_response",
    ],
  },
  {
    name: "Quality",
    continueValue: "security",
    ids: [
      "build",
      "build_common_tools",
      "build_floss_tools",
      "test",
      "test_invocation",
      "test_most",
      "test_continuous_integration",
      "test_policy",
      "tests_are_added",
      "tests_documented_added",
      "warnings",
      "warnings_fixed",
      "warnings_strict",
    ],
  },
  {
    name: "Security",
    continueValue: "analysis",
    ids: [
      "know_secure_design",
      "know_common_errors",
      "crypto_published",
      "crypto_call",
      "crypto_floss",
      "crypto_keylength",
      "crypto_working",
      "crypto_weaknesses",
      "crypto_pfs",
      "crypto_password_storage",
      "crypto_random",
      "delivery_mitm",
      "delivery_unsigned",
      "vulnerabilities_fixed_60_days",
      "vulnerabilities_critical_fixed",
      "no_leaked_credentials",
    ],
  },
  {
    name: "Analysis",
    continueValue: "Save",
    ids: [
      "static_analysis",
      "static_analysis_common_vulnerabilities",
      "static_analysis_fixed",
      "static_analysis_often",
      "dynamic_analysis",
      "dynamic_analysis_unsafe",
      "dynamic_analysis_enable_assertions",
      "dynamic_analysis_fixed",
    ],
  },
];

// Silver form section structure (matches _form_1.html.erb accordion order).
const SILVER_SECTIONS: Array<{ name: string; continueValue: string; ids: string[] }> = [
  {
    name: "Basics",
    continueValue: "changecontrol",
    ids: [
      "achieve_passing",
      "contribution_requirements",
      "dco",
      "governance",
      "code_of_conduct",
      "roles_responsibilities",
      "access_continuity",
      "bus_factor",
      "documentation_roadmap",
      "documentation_architecture",
      "documentation_security",
      "documentation_quick_start",
      "documentation_current",
      "documentation_achievements",
      "accessibility_best_practices",
      "internationalization",
      "sites_password_security",
    ],
  },
  {
    name: "Change Control",
    continueValue: "reporting",
    ids: ["maintenance_or_update"],
  },
  {
    name: "Reporting",
    continueValue: "quality",
    ids: ["report_tracker", "vulnerability_report_credit", "vulnerability_response_process"],
  },
  {
    name: "Quality",
    continueValue: "security",
    ids: [
      "coding_standards",
      "coding_standards_enforced",
      "build_standard_variables",
      "build_preserve_debug",
      "build_non_recursive",
      "build_repeatable",
      "installation_common",
      "installation_standard_variables",
      "installation_development_quick",
      "external_dependencies",
      "dependency_monitoring",
      "updateable_reused_components",
      "interfaces_current",
      "automated_integration_testing",
      "regression_tests_added50",
      "test_statement_coverage80",
      "test_policy_mandated",
      "tests_documented_added",
      "warnings_strict",
    ],
  },
  {
    name: "Security",
    continueValue: "analysis",
    ids: [
      "implement_secure_design",
      "crypto_weaknesses",
      "crypto_algorithm_agility",
      "crypto_credential_agility",
      "crypto_used_network",
      "crypto_tls12",
      "crypto_certificate_verification",
      "crypto_verification_private",
      "signed_releases",
      "version_tags_signed",
      "input_validation",
      "hardening",
      "assurance_case",
    ],
  },
  {
    name: "Analysis",
    continueValue: "future",
    ids: ["static_analysis_common_vulnerabilities", "dynamic_analysis_unsafe"],
  },
];

async function waitForEnter(prompt: string): Promise<void> {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });
  return new Promise((resolve) => {
    rl.question(prompt, () => {
      rl.close();
      resolve();
    });
  });
}

async function fillSection(page: Page, answers: Answer[]): Promise<void> {
  await page.waitForTimeout(500);

  for (const answer of answers) {
    const radioSelector = `input[name="project[${answer.id}_status]"][value="${answer.status}"]`;
    const radio = page.locator(radioSelector);
    const radioCount = await radio.count();
    if (radioCount === 0) {
      console.warn(`  WARN: radio not found for ${answer.id} (${answer.status}) -- skipping`);
      continue;
    }

    const isChecked = await radio.isChecked().catch(() => false);
    const isDisabled = await radio.isDisabled().catch(() => false);
    if (isChecked || isDisabled) {
      console.log(`  SKIP: ${answer.id} (${isDisabled ? "disabled" : "already checked"})`);
    } else {
      await radio.scrollIntoViewIfNeeded().catch(() => {});
      await radio.click({ force: true });
    }

    if (answer.justification) {
      const textareaSelector = `textarea[name="project[${answer.id}_justification]"]`;
      try {
        const textarea = page.locator(textareaSelector);
        const textareaCount = await textarea.count();
        if (textareaCount > 0) {
          await textarea.fill(answer.justification);
        }
      } catch {
        // suppressed
      }
    }

    await page.waitForTimeout(80);
  }
}

async function saveAndContinue(page: Page, continueValue: string): Promise<void> {
  const btn = page.locator(`button[name="continue"][value="${continueValue}"]`).first();
  const count = await btn.count();
  if (count === 0) {
    console.warn(`  WARN: Save and continue button not found for value="${continueValue}", falling back to first continue button`);
    await page.locator('button[name="continue"]').first().click();
  } else {
    await btn.click();
  }
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(600);
}

async function fillPassingBySection(page: Page, allAnswers: Answer[]): Promise<void> {
  const answerMap = new Map<string, Answer>(allAnswers.map((a) => [a.id, a]));

  for (let i = 0; i < PASSING_SECTIONS.length; i++) {
    const section = PASSING_SECTIONS[i];
    console.log(`\n  Section [${i + 1}/${PASSING_SECTIONS.length}]: ${section.name}`);

    const sectionAnswers = section.ids
      .filter((id) => !SKIP_IDS.has(id))
      .map((id) => answerMap.get(id))
      .filter((a): a is Answer => a !== undefined && a.status !== "?");

    console.log(`    ${sectionAnswers.length} answers to fill`);
    await fillSection(page, sectionAnswers);

    const isLast = i === PASSING_SECTIONS.length - 1;
    if (isLast) {
      console.log(`    Saving final section...`);
      await saveAndContinue(page, "Save");
    } else {
      console.log(`    Save and continue -> ${section.continueValue}...`);
      await saveAndContinue(page, section.continueValue);
    }
    console.log(`    URL after save: ${page.url()}`);
  }
}

async function fillSilverBySection(page: Page, allAnswers: Answer[]): Promise<void> {
  const answerMap = new Map<string, Answer>(allAnswers.map((a) => [a.id, a]));

  for (let i = 0; i < SILVER_SECTIONS.length; i++) {
    const section = SILVER_SECTIONS[i];
    console.log(`\n  Section [${i + 1}/${SILVER_SECTIONS.length}]: ${section.name}`);

    const sectionAnswers = section.ids
      .filter((id) => !SKIP_IDS.has(id))
      .map((id) => answerMap.get(id))
      .filter((a): a is Answer => a !== undefined && a.status !== "?");

    console.log(`    ${sectionAnswers.length} answers to fill`);
    await fillSection(page, sectionAnswers);

    const isLast = i === SILVER_SECTIONS.length - 1;
    if (isLast) {
      console.log(`    Saving final section...`);
      await saveAndContinue(page, "Save");
    } else {
      console.log(`    Save and continue -> ${section.continueValue}...`);
      await saveAndContinue(page, section.continueValue);
    }
    console.log(`    URL after save: ${page.url()}`);
  }
}

async function submitAndExit(page: Page): Promise<void> {
  const submitBtn = page.locator('input[type="submit"]:not([name]), button[type="submit"]:not([name])').first();
  const count = await submitBtn.count();
  if (count === 0) {
    await page.locator('input[type="submit"], button[type="submit"]').first().click();
  } else {
    await submitBtn.click();
  }
  await page.waitForLoadState("networkidle");
}

async function main(): Promise<void> {
  const PROJECT_ID = "11662";
  const PASSING_EDIT_URL = `https://www.bestpractices.dev/en/projects/${PROJECT_ID}/passing/edit`;
  const SILVER_EDIT_URL = `https://www.bestpractices.dev/en/projects/${PROJECT_ID}/silver/edit`;

  const args = process.argv.slice(2);
  const silverOnly = args.includes("--silver-only");
  const passingOnly = args.includes("--passing-only");
  if (silverOnly) console.log("Mode: silver section only (skipping passing)");
  if (passingOnly) console.log("Mode: passing section only (skipping silver)");

  const firstUrl = silverOnly ? SILVER_EDIT_URL : PASSING_EDIT_URL;

  console.log("Launching headed Chromium...");
  const browser = await chromium.launch({ headless: false });
  const context = await browser.newContext();
  const page = await context.newPage();

  console.log(`Navigating to ${firstUrl}`);
  await page.goto(firstUrl);
  await page.waitForLoadState("networkidle");

  const currentUrl = page.url();
  if (currentUrl.includes("/login") || currentUrl.includes("/en/login")) {
    console.log("\nThe page redirected to the login screen.");
    console.log("Please complete GitHub OAuth login in the browser, then press Enter to continue...");
    await waitForEnter("> ");

    console.log(`Re-navigating to ${firstUrl}`);
    await page.goto(firstUrl);
    await page.waitForLoadState("networkidle");

    const afterLoginUrl = page.url();
    if (afterLoginUrl.includes("/login")) {
      console.error("ERROR: Still on login page. Please ensure you completed the OAuth flow.");
      await browser.close();
      process.exit(1);
    }
  }

  // --- Fill passing section-by-section ---
  if (!silverOnly) {
    console.log("\nFilling passing-level criteria (section by section)...");
    const pAnswers = passingAnswers();
    console.log(`  ${pAnswers.length} criteria to fill`);
    await fillPassingBySection(page, pAnswers);
    console.log(`  After passing fill, URL: ${page.url()}`);

    if (passingOnly) {
      console.log(`\nDone (passing only). Check https://www.bestpractices.dev/projects/${PROJECT_ID}`);
      await browser.close();
      return;
    }
  }

  // --- Navigate to silver edit page ---
  console.log(`\nNavigating to ${SILVER_EDIT_URL}`);
  await page.goto(SILVER_EDIT_URL);
  await page.waitForLoadState("networkidle");

  const silverUrl = page.url();
  if (silverUrl.includes("/login")) {
    console.log("Redirected to login again. Please complete GitHub OAuth login, then press Enter...");
    await waitForEnter("> ");
    await page.goto(SILVER_EDIT_URL);
    await page.waitForLoadState("networkidle");
  }

  // --- Fill silver section-by-section ---
  console.log("Filling silver-level criteria (section by section)...");
  const sAnswers = silverAnswers();
  console.log(`  ${sAnswers.length} total silver criteria`);
  await fillSilverBySection(page, sAnswers);

  console.log("\nSubmitting silver form...");
  await submitAndExit(page);
  console.log(`  After submit, URL: ${page.url()}`);

  console.log(`\nDone. Check https://www.bestpractices.dev/projects/${PROJECT_ID}`);
  await browser.close();
}

main().catch((err: unknown) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
