# Releasing

Released crates: `o1-contract`, `o1-core`, `o1-sync`, `o1-predict`. Each is published on its
own, by its own tag, by `.github/workflows/release.yml` in
<https://github.com/jameshuntt/o1>, through crates.io Trusted Publishing.
Nothing else publishes; no long-lived crates.io token exists for this repository.

## Steps for one release

1. Choose the SemVer bump deliberately from what changed in that crate's public API and
   behaviour (before 1.0: a breaking change moves the minor version, anything else the patch
   version).
2. Set `version` in `crates/<crate>/Cargo.toml`. If a dependant must require the new version,
   update the `version` beside `path` in the root `[workspace.dependencies]` too.
3. Add a `## <version> (<date>)` entry, date as `YYYY-MM-DD`, to `crates/<crate>/CHANGELOG.md`
   (the release workflow refuses a tag without it) and a `## <crate> <version> (<date>)` entry
   to the root `CHANGELOG.md`.
4. Commit, push the branch, and wait for CI (`.github/workflows/ci.yml`) to be green.
5. Create an annotated tag on that commit and push it:

   ```sh
   git tag -a <crate>-v<version> -m "<crate> <version>"
   git push origin <crate>-v<version>
   ```

6. The workflow checks that the tag names one of the four crates, that the version equals the
   manifest's, that the changelog has the entry, that the version is not on crates.io yet and,
   for `o1-core`, `o1-sync` and `o1-predict`, that the `o1-contract` version they require is on
   crates.io. It runs that crate's tests, clippy, rustdoc, the 1.85 build and
   `cargo package`, then publishes that one crate.

A tag whose version is already on crates.io runs the same checks and the Trusted Publishing
exchange and publishes nothing. That is how the hand-published 0.1.0 tags run.

When several crates move together, tag and publish `o1-contract` first and wait until it is on
crates.io before pushing the dependants' tags.

## One-time setup per crate

crates.io has configured a Trusted Publisher only on a crate that already exists; if that is
still so, the first version of each crate is published by hand (`cargo publish -p <crate>`
with a personal token, `o1-contract` first) and the publisher is added afterwards. On
crates.io, crate → Settings → Trusted Publishing → Add, GitHub:

| field | value |
|---|---|
| repository owner | `jameshuntt` |
| repository name | `o1` |
| workflow filename | `release.yml` |
| environment | `release` |

In the GitHub repository, create the environment `release` (Settings → Environments); add
required reviewers there if a publication should wait for an approval.
