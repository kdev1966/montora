# Code signing policy

Montora has no paid signing certificate. What each platform gets today:

| Platform | Signature |
|---|---|
| Windows | Not signed yet. SmartScreen may warn on first launch: **More info** › **Run anyway**. Montora will apply to [SignPath Foundation](https://signpath.org/), which signs open-source projects for free. |
| macOS | Ad-hoc signed, not notarised: Montora has no Apple Developer account and is not distributed through the App Store. Open it the first time with right-click › **Open**. |
| Linux | Packages are not signed. Each release lists the SHA-256 of every file. |
| Android | The APK is signed with Montora's own release key, outside the Play Store. |

The self-updater checks each download against the SHA-256 published with the
release. A signature on the update manifest, so that a compromised release
page cannot serve a different build, is planned.

## How a release is made

- Every release file is built by GitHub Actions from a tagged commit of
  <https://github.com/kdev1966/montora>, by the workflows in
  [`.github/workflows`](../.github/workflows). Nothing built on a person's
  machine is published.
- A release is built only after the tests pass on the tagged commit.
- The workflows and build scripts are part of the source and are reviewed like
  any other change.

## Team and roles

| Role | Members |
|---|---|
| Maintainer, reviewer and release approver | [@kdev1966](https://github.com/kdev1966) |

## Privacy policy

This program will not transfer any information to other networked systems
unless specifically requested by the user or the person installing or operating
it. Montora reaches the network only when you ask it to:

- **Check for updates** in Settings asks GitHub for the list of Montora's
  releases, and installing one downloads it from GitHub.
- **Downloading a model** (transcription, speech, cutout) fetches it from the
  address shown in Settings. By default that is Concat's model mirror on
  GitHub.
- **The Remote API** is off by default. Turned on, it listens on this computer
  (`127.0.0.1`) unless you give it another address, and answers only callers
  that hold its token.
- **A cloud AI provider**, once Montora has them, receives what you send it
  only after you have chosen that provider and entered your own key.

Nothing is sent about you, your projects or your media, and there is no
telemetry.
