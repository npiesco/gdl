# Release smoke checklist

- Build release binaries with `make release`.
- Confirm `target/debug` is absent after release cleanup.
- Compare `gdl` and `gdl-mcp` output on the live repository.
- Capture status and diff byte counts in the release notes.
- Re-run dogfood after changing MCP configuration.

Dogfood smoke should use the release binaries:

```sh
target/release/gdl --repo . --format plain --color never status
target/release/gdl --repo . --format plain --color never diff PLAN.md --area worktree
target/release/gdl-mcp
```
