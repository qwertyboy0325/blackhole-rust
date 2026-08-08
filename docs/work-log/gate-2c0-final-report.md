# Gate 2C0 — Physical Thin-Disk Emission (Page–Thorne + Planck + g³)

**Status:** prior authoritative PASS @ `551f69e` **INVALIDATED** by owner closure
`5225301622` (Page–Thorne `F∝Q` missing `1/(B√C)`; non-independent numerical
oracle; truncation/frame/tolerance authority). Physics root fix in progress;
awaiting clean re-evaluate + report-only follow-up.

Corrected conventions: [`docs/physical-disk-emission-v1.md`](../physical-disk-emission-v1.md).

```text
Gate 2C0 previous PASS             INVALIDATED (5225301622)
PR #18                             DRAFT / NOT MERGED
Gate 2C1                           NOT AUTHORIZED
```

The superseded PASS narrative below is retained only as historical record of the
invalidated evaluate at `551f69e` / report tip `75d1aa6`.

---

## Historical record (INVALIDATED) — evaluate at 551f69e

| Field | Value |
| --- | --- |
| Commit | `551f69eaf932543f30698496ec379781e42de4f0` |
| Result then | `PASS` (`authoritative: true`) — **now void** |
| Evaluation content digest | `1d67819cdd07f0b8f75c6983cdc51d9a0412a99ba36ea2bac6c977b5a5e764db` |
| Closure | `5225301622` |

Root causes (do not re-accept without fix evidence):

1. Production `F = (3GMṀ)/(8πr³)·Q` omitted mandatory `1/(B√C)` for the repo `Q` convention.
2. Numerical oracle used `(dΩ/dr)^{-1}` and compared self-defined `Q`, not conservation-law flux.
3. Planck truncation normalized against another finite band instead of `σT⁴/π`.
4. Frame `Deserialize` bypass + closure `max_abs` coupled to `max_rel`.
5. Provisional tolerances / explore grid claimed as authoritative.
