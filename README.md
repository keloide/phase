# fra-stingerquill continuation archive

This branch contains only an export of the original run and its referenced evidence. The implementation branch and checkpoint PR are unchanged.

## Short handoff

- Checkpoint: [keloide/phase#13](https://github.com/keloide/phase/pull/13), Draft, HEAD `3f970dbcf01138de751de77aa5d26462637cd3e7`.
- Current phase: phase 2 implemented, not accepted. Phase 1 was accepted at `ab1363d1f30141c118db0c83de0da617e0c07c65`; original BASE is `ec27a8d1a09b3f3e0c0ded9a08b8e6c0aef190ff`. No final frozen upstream BASE exists.
- Preserve the authoritative phase-fit entries 1–580, `budget=until-clean`, plan13 / implementation19 / remote22FORK22ALL, every FAIL and the current two-failed-NON-predictions stop. This export resets nothing.
- Current C19 native full Step 5 is green, including all 27 Essence tests. M10's complete 35,880-face C8→C19 comparison has three expected scope changes, zero regressions and zero new semantic findings. Overall V21 remains FAIL due its expired publication wait.
- Outstanding: original MED1 lifetime and MED2 primary-control findings await formal retirement by a full review. Both latest mandatory reviewer dispatches failed at model capacity before reviewing anything.
- Next step: restore and verify the complete original history, read CURRENT repository instructions, obtain concrete fresh-reviewer capacity and explicit authorization resolving the existing NON-prediction stop, then conduct the full C8→C19 phase-2 implementation review. Final acceptance/integration/synchronization/generation/current-head checks/reviews/Gate A/B/scope/upstream PR are still pending.

## Download and restore

Download this branch as a GitHub ZIP, unzip it, then run from the directory containing the parts:

```bash
# Verify each part (the combined filename in SHA256SUMS is absent until concatenation).
sha256sum --check --ignore-missing SHA256SUMS
cat fra-stingerquill-continuation.tar.gz.part-* > fra-stingerquill-continuation.tar.gz
sha256sum --check --ignore-missing SHA256SUMS
mkdir restored
tar -xzf fra-stingerquill-continuation.tar.gz -C restored
```

`restored/engine-implementer-runs/fra-stingerquill/` is the complete original run directory. `external-files/` retains the original path layout for logs, exports, archives, source snapshots, policy readings and inputs. `archive-files.json` contains all file hashes and safe identical-content hardlinks; `source-checkpoints.bundle` preserves the original source/proof commits with immutable originalBASE as prerequisite. Restore the run into the resolved Git common-dir only after checking identity and history; do not restart or reconstruct it.

See [HANDOFF.md](HANDOFF.md) for the full continuation contract, verification identities, limitations and restoration instructions. No credentials, .git configuration, daemon sockets, build caches or foreign Hive worktree are packaged.

## External dependency

The complete pinned-input ZIP, run37483983597/artifact11424046878, is not present locally and its GitHub download redirects to an Azure host denied by this environment's proxy. Its exact download link, hash, size and **2026-10-20 expiry** are preserved in `remote-artifacts.json` inside the archive. Obtain it through an authorized environment before expiry. Included local AtomicCards/sidecars are not a substitute for the full input pin. All persisted native evidence, including the licensed C8 retained outputs, is included.

## Archive integrity

Compressed size: 2,058,918,496 bytes in 23 ordered parts (each at most 90,000,000 bytes). Combined SHA256: `afe38f947f001ce21f40c305b9721ff080776adab84bed88048854a969064d88`. Full run history SHA256: `5b4b05b70400e91f87a0393c1ff364113d2c8880c65de8531f9c1d983d2e2de7`. All 30,406 TAR members, payload hashes and safe deduplication links were independently read and verified before publication.
