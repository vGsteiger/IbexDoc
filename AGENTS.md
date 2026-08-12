# Agent instructions

Instructions for AI coding agents (Claude Code and similar) working in this repository.
Human contributors should follow [CONTRIBUTING.md](CONTRIBUTING.md), which these rules build on.

## Always work on a branch, then push and open a PR

Never commit directly to `main`. For every code change, without waiting to be asked:

1. **Branch first.** Create a branch off `main` before making edits, named
   `<type>/<short-description>` per [CONTRIBUTING.md](CONTRIBUTING.md#branch-naming)
   (`feat/`, `fix/`, `docs/`, `chore/`, `refactor/`, `test/`, `ci/`, `style/`).
   When the work resolves a GitHub issue, include the number: `fix/337-sqlcipher-key-guard`.
2. **Verify before committing.** Run the relevant format/lint/tests from [CONTRIBUTING.md](CONTRIBUTING.md#code-style) (e.g., Rust: `cd dokassist/src-tauri && cargo fmt && cargo clippy -- -D warnings`; Frontend: `cd dokassist && pnpm lint && pnpm test`).
   Do not commit a change that adds new warnings or fails checks.
3. **Commit** using [Conventional Commits](https://www.conventionalcommits.org/), and
   reference the issue in the body (`Fixes #337`) so it closes on merge.
4. **Push** the branch to `origin` with upstream tracking (`git push -u origin <branch>`).
5. **Open a PR** against `main` with `gh pr create`:
   - Fill in [the PR template](.github/pull_request_template.md) — description, type of
     change, checklist, release notes. Tick only what is actually true; annotate items that
     do not apply (`n/a, internal guard`) rather than silently leaving them blank.
   - Apply a version label: `patch`, `minor`, `major`, or `skip-release`. A PR without one
     does not get a correct release bump.
   - State anything a reviewer would otherwise have to discover: untestable branches,
     skipped scope, assumptions made.
6. **Report the PR URL** back to the user.

Do not merge the PR. A maintainer reviews and merges; if auto-merge is desired, apply the `merge when ready` label once the PR is approved and all checks are green (per [CONTRIBUTING.md](CONTRIBUTING.md)).

### Scope

- One concern per branch and PR. If a second, unrelated change is needed (a workflow doc, a
  drive-by fix), put it on its own branch and open a separate PR.
- Do not commit unrelated pre-existing changes that happen to be in the working tree. Stage
  files explicitly by path rather than using `git add -A`.

### Exceptions

- Pushing and PR creation are outward-facing. If the user has asked you to stop before that
  point, or the work is exploratory, still commit on a branch and say what remains.
- Trivial non-repository work (scratch files, local experiments, answering a question)
  needs no branch.
