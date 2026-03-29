# Governance

## Overview

Aptu is a solo-maintained open source project. This document describes the decision-making structure, roles, and continuity guarantees.

## Roles

### Owner

**@clouatre** holds the Owner role with the following exclusive authorities:

- Merge authority: only the owner merges pull requests to `main`
- Release authority: only the owner tags releases and publishes to crates.io
- Direction veto: the owner has final say on feature scope, API contracts, and breaking changes
- Repository administration: branch protection rules, CI configuration, secrets management

The owner participates in all architectural discussions and reviews every pull request before merge.

### Contributor

Any person who submits a pull request is a Contributor. Contributors have no merge or release authority but are credited in release notes for accepted contributions.

External contributors may:

- Open issues and feature requests
- Submit pull requests against `main`
- Comment on and discuss any open issue or pull request

## Decision Process

1. Community feedback arrives via GitHub Issues or pull request discussions.
2. The owner reviews the proposal and responds with acceptance, requested changes, or rejection.
3. Accepted pull requests are squash-merged by the owner after passing CI.
4. Direction changes (API breaks, scope expansions, license changes) require explicit owner approval and are announced via a tagged release with a migration guide.

There is no voting, no steering committee, and no committee approval process. The owner's decision is final.

## Bus Factor

**Bus factor = 1.** The project depends on a single maintainer for merge, release, and direction decisions. This is acknowledged explicitly.

Mitigations:

- **Apache-2.0 license**: Any contributor or user may fork the repository and continue the project under a new name without restriction. The Apache-2.0 license is an unconditional fork guarantee; no permission from the owner is required.
- **Public issue tracker**: All open issues, feature discussions, and bug reports are publicly visible on GitHub and preserved in any fork.
- **Public CI**: The full CI pipeline (`.github/workflows/`) is committed to the repository and reproducible without owner involvement.
- **Published crates**: `aptu-cli` and `aptu-core` are published to crates.io and remain available even if the GitHub repository becomes unavailable.

## Succession

There is no designated successor. In the event that the owner becomes permanently unavailable:

1. The Apache-2.0 license guarantees any contributor can fork and resume development.
2. The most active contributors at that time are encouraged to coordinate a community fork.
3. No other action is required; the license is the succession mechanism.

## Communication

- **Bug reports and feature requests**: GitHub Issues at `https://github.com/clouatre-labs/aptu/issues`
- **Pull requests**: `https://github.com/clouatre-labs/aptu/pulls`
- **Security issues**: See [SECURITY.md](SECURITY.md) for the private disclosure process

## Amendment

This document may be updated by the owner at any time via a pull request to `main`. Material changes (role additions, license changes) will be announced in the corresponding release notes.
