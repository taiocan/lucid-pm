# Nucleus Platform v2 — Risk and Issue Review
Date: 2026-05-30

## Risks identified

**Third-party auth provider stability**
Our current auth provider (Authify) has had three outages in the last two months.
If they go down during our beta launch window, new user registrations will be
blocked. This is a high-priority risk — we need a mitigation plan before launch.

**Database migration window too short**
The planned 4-hour maintenance window for the database migration may not be enough
given the volume of data. A failed mid-migration could corrupt the production record.
High priority — needs a longer window or a phased approach.

## Issues open

**Login fails intermittently on iOS Safari**
Several beta users reported that the login flow breaks on iOS Safari when using
private browsing mode. Reproducible about 30% of the time. Currently in progress —
Marco's team is investigating the session cookie behaviour.

**Report export crashes for large datasets**
The report export feature crashes with an out-of-memory error when exporting more
than 500 items. Affects the weekly report workflow that several customers rely on.
Medium priority — workaround is to export smaller date ranges.

## Action items

Marco to assign the iOS Safari bug to the frontend team immediately.
Ana to schedule a migration window review with the database team.
