# Gate 2C0 — Physical Thin-Disk Emission (Page–Thorne + Planck + g³)

**Status:** authoritative evaluate **PASS** after final authority closures
`5225301622` + `5225371466` — pending owner merge. Gate 2C1 **not** authorized.

## Evaluated commit

| Field | Value |
| --- | --- |
| Commit | `75421441cc4babaa61aeb8362c848157aac8f52a` |
| Branch | `gate-2c0-physical-emission` |
| Result | `PASS` (`authoritative: true`) |
| Evaluation content digest | `82727d7663c03eda7b94d14d22ceb6d2b832edad4bb97b6d18056025b30730a3` |
| Checks | 40 / 40 PASS |
| Closures | `5225301622` (PT root), `5225371466` (emission-frame + freeze docs) |

Prior tips: physics root `a760427` (PASS, now superseded for emission-frame
hashing); invalidated original `551f69e`.

## Gate digests (128² × `physical-spectral-grid-v1`)

| Quantity | Digest |
| --- | --- |
| physical emission frame | `5e3b15023df9bf3debed9666d65a3c762cfe83fe9885e7a5c8b3565dc19a383e` |
| physical spectral frame | `136b1fbcc76beb08ea38aa24d16803d621da20bad5b7ebfecc7a13c260aa8dd1` |
| physical spectral grid v1 | `ceb3db28082bb357e50cac2635b221711bf79ea2806f2c25b60c61ca901162d5` |
| inherited 2B0 frequency | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |

Emission/spectral digests changed vs `a760427` after hashing
`gravitational_radius_m` / `radius_m` / `inside_isco` into the emission
authority (expected for `5225371466`). Grid digest unchanged.

## Closure `5225371466`

| Item | Resolution |
| --- | --- |
| F1 frame cross-fields | bounds / ISCO / positive-emission policy; `F=σT⁴` @ `1e-12`; `radius_m=r_g·(r/M)` with hashed `gravitational_radius_m`; tamper tests |
| F2 freeze docs | removed stale “exploratory pending freeze”; documented why **256** not 128/512 |

## Physics / spectral (unchanged from `5225301622` accept)

- `F = (3GMṀ)/(8πr³)·Q/(B√C)`
- conservation-law flux oracle (`−Ω_,r` numerator)
- truncation vs `σT⁴/π`
- independent abs/rel spectral maxima
- frozen `physical-spectral-grid-v1` (256)

Gate SB max-rel ≈ `1.21e-4` (tol `5e-4`); PT algebraic↔numerical worst rel ≈ `1.9e-8`.

## Owner stop

```text
Gate 2C0 Page–Thorne physics       PASS
Emission-frame authority           PASS (5225371466)
Frozen-grid documentation          PASS
Authoritative eval @ 7542144       PASS
PR #18                             DRAFT / NOT MERGED
Gate 2C1                           NOT AUTHORIZED
```
