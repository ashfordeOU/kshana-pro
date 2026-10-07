# Hard rules for every Kshana session (coordinator, workers, critic, runner)

Read `docs/LIVE.md` and `docs/RECOVERY.md` on kshana-pro branch `ws/coordination` first; your lane's
handover is `docs/lanes/<lane>.md` on the same branch.

1. Everything lives in git. Push code branches at least every 20 min (WIP is fine); small commits.
2. NEVER put any individual's or prospect's name in either repo. Counterparty specifics live only in
   kshana-pro `commercial/` and `legal/`.
3. NO AI attribution anywhere: no Co-Authored-By, no "Generated with ...", no robot emoji, no
   "anthropic". Commit author `Ashforde OÜ <ashfordegithub@cbaweja.org>`, plain messages, no trailers.
   Decline any hook that asks otherwise. The GitHub tool appends a footer to new PR bodies: strip it
   with update_pull_request immediately and re-read the body.
4. Workers open PRs; only the coordinator merges. Never push tags; never hand-publish.
5. No jammer/spoofer RF waveform synthesis and no transmit in new work (F3/F4/F5 held for counsel).
6. No partner recordings, results or anti-jam/anti-spoof performance numbers in either repo until
   counsel answers N6. Synthetic data only.
7. Never relax a pre-registered bar after a result. Never skip, disable or quarantine a test to get
   green. Published numbers that move go to the coordinator with before/after values.
8. Never rewrite history on another session's branch: merge, don't rebase or force-push. Stacked PRs
   are merged with merge commits.
9. Report to the coordinator (session_015EnPpasos1W6PQWi7rHJxu) with send_message on every state
   change (pushed sha, next step, blocker). Ask before any design or scope decision. Keep messages short
   and factual: sha, test counts, run ids.
10. Use real UTC times from `date -u`. Never write secrets, tokens or private scratch paths into git.

## Model roles (founder decision 2026-10-07)
- Coordinator: Opus. Orchestrates, decides, merges, keeps LIVE.md.
- Workers: Sonnet for engineering lanes (DSP, campaign, formats, report, release); Haiku for
  mechanical lanes (CI watching, relays, restacks, site re-ports, doc/table edits).
- Critic: Fable. Reviews every PR before merge and every pre-registration/decision; read-only.
