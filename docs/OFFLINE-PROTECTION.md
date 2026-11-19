# Offline protection of the patent-candidate methods

Owner decision, 19 November 2026. This is how the patent-candidate methods are protected
when they must run on a customer's own machine (on-prem or airgapped), before their patents
are filed. It is a layered posture; no single layer is extraction-proof, and the honest
limit is stated at the end.

## What is being protected

The patent candidates (roadmap 0.42, `protect/trade-secret-register.toml`): E1 cross-system
attribution, E3 spoof-safe ASF learning, E4 meacon detection, E7 delay-bounded LDC
authentication, F1 self-calibrating simulator loop, F2 integrity-driven ASF survey planner,
C2 first-plug conformance, B2 hardware-anchored waveform evidence. Their CLI entry points
are `eloran selfcal`, `eloran survey-plan`, `eloran guard` and `eloran bench firstplug`
(plus the attribution library used by `guard`).

## The layers

1. **Feature gate — kept out of every ordinary build.** The methods compile only with
   `--features unfiled-methods` (off by default). The public build, Desktop Pro
   (`apps/desktop-pro`, default features only) and every package exclude them; their
   subcommands report *"a patent-pending method not included in this build"*. CI builds and
   tests both with and without the feature. This is the first and strongest line: the code
   is simply absent from anything a general customer receives.

2. **Hardened on-prem build — the only build that carries them.** When a vetted, contracted
   customer needs the methods on their own machine, the binary is built by
   `scripts/protect/build-onprem.sh` with the `release-onprem` profile (`Cargo.toml`): fat
   LTO and one codegen unit inline the method bodies across crate boundaries so they do not
   survive as tidy, separately named functions; `panic = "abort"` drops the unwinding
   tables and their source strings; symbols are stripped. The script then runs
   `scripts/protect/symbol-check.sh --require-stripped`: the binary must carry no
   Arksa-exclusive or Core name and no symbol table.

3. **Node-locked at runtime.** Each generating entry point calls `require_bound`
   (`src/cli/eloran_cmd.rs`): it runs only under a *machine-bound* `pro-eloran` licence —
   there is no Evaluation fallback, and the signed token must name this machine
   (`kshana-pro license machine-id`; machine binding is enforced in
   `src/license/mod.rs`). A floating token, or the binary copied to another machine, cannot
   run the method. `--dev` bypasses only in a debug build; the on-prem profile is a release
   build.

4. **Encrypted, machine-bound data delivery.** Tunable or valuable data a method needs
   (reference grids, coefficients, conformance vectors, signing material) ships as a Pro
   pack (`src/propack/`): sealed with ChaCha20-Poly1305, keyed per licence *and* per machine
   (HKDF from the server's pack master secret, the token id and the device's machine hash),
   watermarked per licensee, and signed. A pack opens on exactly one device under exactly
   one licence; a lifted copy is inert and a leaked plaintext carries the licensee's
   watermark.

5. **Patent.** The methods are filed as patents. Once filed, the method can be disclosed and
   the gate relaxed on its own schedule; until then it stays behind layers 1–4.

## The honest limit

Layers 2–4 stop copying, casual extraction, and running the method off the machine it is
licensed to. They do **not** make a method that executes on hardware the customer controls
extraction-proof against a determined, well-resourced reverse engineer on that licensed
machine. That residual risk is covered by **the patent**, not by the binary, and by shipping
the on-prem build only to vetted, contracted customers. Hardening buys time; the patent is
the moat. The highest-value methods that need not run on the customer's machine are instead
kept off it entirely (hosted verdict-only service / Arksa-exclusive Core); see the VCprep
placement note.

## Checklist before an on-prem release

- [ ] Customer is vetted and under contract (NDA + licence terms).
- [ ] Licence issued **machine-bound** to the customer's device hash, `pro-eloran`
      (or `-hw`) tier.
- [ ] `scripts/protect/build-onprem.sh --out <dir>` is green (build + symbol-check).
- [ ] Data packs issued per device via `src/propack` (issuer).
- [ ] Patent application for each included method is filed or counsel has cleared the ship.
