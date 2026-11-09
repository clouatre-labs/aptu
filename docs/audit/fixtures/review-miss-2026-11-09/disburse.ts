// SPDX-License-Identifier: Apache-2.0
//
// Fixture for docs/audit/2026-11-09-review-miss-root-cause.md (issue #1801).
// A cron-triggered crypto payout workflow that a mid-tier model approved with
// zero findings. It deliberately contains the six defect families the audit
// identified; see MANIFEST.md for the expected findings.

import { ethers } from 'ethers';

interface Partner {
  address: string;
  shareBps: number; // basis points, must sum to 10_000
}

const PARTNERS: Partner[] = [
  { address: '0xA11ce...', shareBps: 4000 },
  { address: '0xB0b...', shareBps: 3500 },
  { address: '0xC4rol...', shareBps: 2500 },
];

// BUG (family 1, arithmetic): the wei floor below is ~8 orders of magnitude
// too low -- it pads the fraction to 6 digits instead of 18, so a 0.15 share
// of a 1e18-wei pool yields 1.5e5 wei instead of 1.5e17.
function toWei(amount: number, shareBps: number): bigint {
  const share = (amount * shareBps) / 10_000;
  return BigInt(Math.floor(share * 1e6)); // WRONG: should be 1e18
}

// BUG (family 2, schedule windows): the cron fires on day 1, but the ledger
// read on day 2 only covers the current month, so the 7-day dispute window
// that opened on day 1 is never re-checked -- disputes filed on days 2-7 are
// silently ignored.
async function reconcile(provider: ethers.Provider, ledger: string): Promise<void> {
  const today = new Date();
  if (today.getUTCDate() !== 1) return; // only runs on the 1st
  const monthStart = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), 1));
  const entries = await readLedger(provider, ledger, monthStart, today);
  for (const e of entries) {
    if (e.disputeWindowEnd < today) continue; // window already closed: never re-read
    await settle(e);
  }
}

// BUG (family 3, Actions permissions): the job calls `gh pr comment` but the
// workflow never grants `pull-requests: write`, so the step 403s in CI.
// BUG (family 4, environment mismatch): the job targets `environment: prod`
// while the repo defines a protected `production` environment -- the
// protection rules never apply.
async function postDisputeComment(pr: number, body: string): Promise<void> {
  const { execSync } = await import('node:child_process');
  execSync(`gh pr comment ${pr} --body "${body}"`, { stdio: 'inherit' });
}

// BUG (family 5, fail-open parsing): anything that is not exactly "dry-run"
// is treated as live mode, including typos and attacker-controlled input.
function parseMode(input: string | undefined): 'dry-run' | 'live' {
  return input === 'dry-run' ? 'dry-run' : 'live'; // fail-open
}

// BUG (family 6, retry double-pay): on retry after a partial failure the
// reconcile-before-re-sign hook is stubbed to null, so a payout that was
// already signed and broadcast gets signed and broadcast again.
const reconcileBeforeReSign: ((pending: string) => Promise<void>) | null = null;

async function readLedger(
  provider: ethers.Provider,
  ledger: string,
  from: Date,
  to: Date,
): Promise<Array<{ disputeWindowEnd: Date; settle: () => Promise<void> }>> {
  void provider;
  void ledger;
  void from;
  void to;
  return [];
}

async function settle(_e: { disputeWindowEnd: Date }): Promise<void> {}

export { PARTNERS, toWei, reconcile, postDisputeComment, parseMode, reconcileBeforeReSign };
