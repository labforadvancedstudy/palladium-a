# Alan von Palladium: Vision & Roadmap — SUPERSEDED

> **This document has been retired. Its body is deleted rather than corrected.**
> Sequencing lives in [`docs/contributing/MILESTONES.md`](../contributing/MILESTONES.md), and what
> 1.0 requires lives in [`docs/contributing/1.0-requirements.tsv`](../contributing/1.0-requirements.tsv),
> where every row carries an owning milestone and a command that decides it.

## Why it was deleted rather than updated

It was written in the fictional present, and there was nothing in it to update. It opened
*"Palladium α v0.7 has achieved what many thought impossible"* and scored the project 100/100, 97/100
and 90/100 against Turing, von Neumann and Shannon. It carried a comparison table — compile time,
code size, network throughput — against Rust 1.74, for a period in which no `.pd` program in this
repository had ever produced an executable, because the C runtime the driver passed to gcc had never
been committable under a blanket `*.c` in `.gitignore`. It scheduled *"Q3 2025: Self-Hosting
Milestone"* and *"Q4 2026: Language Freeze"*. Self-hosting was first achieved, as a byte-identical
fixed point, on 2026-08-21.

It also assigned **v0.7** to a state of the project that never existed, while
[`MILESTONES.md`](../contributing/MILESTONES.md) assigns v0.7 to a milestone that has not started.
Two documents in one repository claiming the same version number for a past and a future is the
confusion that let *"v0.6: Self-hosting achieved"* survive in a version history for a year.

The banner it carried said "PROPOSAL — not implemented", which flags a design that has not been
built. That was the wrong instrument: nothing here was an unbuilt *design*. The numbers were
presented as measurements, and no benchmark producing any of them exists in this repository. A
banner cannot make a fabricated measurement safe to keep, so the measurements are gone.

The same reasoning removed "Implementation Roadmap", "Summary Statistics" and "Version History"
from [`PALLADIUM_V1_FEATURES.md`](../reference/features/PALLADIUM_V1_FEATURES.md#what-was-removed-from-this-document-and-why).
This is that cleanup finishing.

## Where the material worth keeping now lives

| Was here | Now |
|---|---|
| Phase and quarter plan | [`MILESTONES.md`](../contributing/MILESTONES.md) — M2…M9 and 1.0.0, each exiting on one command |
| "What 1.0 is" | [`1.0-requirements.tsv`](../contributing/1.0-requirements.tsv) — 178 enumerated rows, each with its evidence |
| Performance claims | [`palladium_vs_rust_comparison.md`](../contributing/palladium_vs_rust_comparison.md), which reports `benchmarks/run_benchmarks.sh` |
| What the language is | [`language-spec.md` Part I](../specification/language-spec.md#part-i-normative-specification) |
| What `pdc` does today | [the implementation status annex](../specification/language-spec.md#part-ii-implementation-status-annex) |
| The mission | A systems language with Rust's safety, provable termination and no lifetime annotations. It needs no scoreboard |
