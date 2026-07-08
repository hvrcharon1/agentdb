# Manual Publish Steps

These are one-time or per-token actions that cannot be automated by CI. After completing
each section the corresponding workflow will run automatically on every future version tag.

---

## 1. Snap Store — register name and generate CI credentials

The snap name is `datacules-agentdb` (as declared in `snap/snapcraft.yaml`).

**Prerequisites:** An [Ubuntu One](https://login.ubuntu.com/) account and `snapcraft`
installed on an Ubuntu machine (or WSL 2).

```bash
# Install snapcraft if not already present
sudo snap install snapcraft --classic

# Log in with your Ubuntu One account
snapcraft login

# Register the snap name (only needed once — skip if already registered)
snapcraft register datacules-agentdb

# Export credentials that CI will use to publish
# The file will contain a base64-encoded macaroon token.
snapcraft export-credentials --snaps datacules-agentdb snap.creds

# Encode the credentials for GitHub Secrets (no line wrapping)
base64 -w0 snap.creds > snap.creds.b64
cat snap.creds.b64
```

Then in the GitHub repository:

1. Go to **Settings → Secrets and variables → Actions → New repository secret**
2. Name: `SNAPCRAFT_STORE_CREDENTIALS`
3. Value: paste the output of `cat snap.creds.b64`

The workflow `.github/workflows/snap-publish.yml` will use this secret automatically on
every `v*.*.*` tag push.

---

## 2. WinGet — first manual PR submission

The WinGet CI workflow (`.github/workflows/winget-publish.yml`) uses
[winget-releaser](https://github.com/vedantmgoyal9/winget-releaser) to automate
subsequent submissions. However, the **very first submission** for a new package must be
a manual PR to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) so the
package identity can be reviewed and approved.

### 2a. Prerequisites

- A GitHub account (the PR is opened against `microsoft/winget-pkgs`)
- A GitHub **PAT** (classic or fine-grained) with `repo` + `workflow` scopes saved as the
  repository secret `WINGET_TOKEN` — this is what winget-releaser uses for future PRs.

### 2b. Get the real SHA256 for the v0.6.0 installer

Before opening the PR, replace the placeholder SHA256 in
`winget/manifests/d/Datacules/AgentDB/0.6.0/Datacules.AgentDB.installer.yaml`.

```bash
# Linux / WSL
curl -sL https://github.com/hvrcharon1/agentdb/releases/download/v0.6.0/agentdb-x86_64-pc-windows-msvc.zip \
  | sha256sum

# Windows PowerShell
$hash = (Get-FileHash -Algorithm SHA256 (Invoke-WebRequest `
  "https://github.com/hvrcharon1/agentdb/releases/download/v0.6.0/agentdb-x86_64-pc-windows-msvc.zip" `
  -OutFile agentdb.zip; "agentdb.zip")).Hash.ToLower()
echo $hash
```

Update the `InstallerSha256` field in
`winget/manifests/d/Datacules/AgentDB/0.6.0/Datacules.AgentDB.installer.yaml` with the
real 64-character hex value.

### 2c. Submit the PR

```bash
# Fork microsoft/winget-pkgs (first time only)
gh repo fork microsoft/winget-pkgs --clone

# Copy the manifests into the fork
cp -r winget/manifests/d/Datacules winget-pkgs/manifests/d/

# Create a branch
cd winget-pkgs
git checkout -b add-Datacules.AgentDB-0.6.0

# Stage and commit
git add manifests/d/Datacules/AgentDB/0.6.0/
git commit -m "Add Datacules.AgentDB version 0.6.0"

# Push and open the PR
git push origin add-Datacules.AgentDB-0.6.0
gh pr create \
  --repo microsoft/winget-pkgs \
  --title "Add Datacules.AgentDB version 0.6.0" \
  --body "New package submission for AgentDB v0.6.0 — single-file embedded database for AI agents."
```

### 2d. Manifest files (for reference)

The three manifests live at:

```
winget/manifests/d/Datacules/AgentDB/0.6.0/
├── Datacules.AgentDB.yaml                  (version manifest)
├── Datacules.AgentDB.installer.yaml        (installer — update SHA256 before submitting)
└── Datacules.AgentDB.locale.en-US.yaml     (package metadata)
```

Key fields in the installer manifest:

| Field | Value |
|-------|-------|
| `InstallerUrl` | `https://github.com/hvrcharon1/agentdb/releases/download/v0.6.0/agentdb-x86_64-pc-windows-msvc.zip` |
| `InstallerType` | `zip` |
| `NestedInstallerType` | `portable` |
| `PortableCommandAlias` | `agentdb` |

### 2e. After approval

Once the first PR is merged, all subsequent version bumps are handled automatically by
`.github/workflows/winget-publish.yml` — no manual steps needed.

---

## 3. Chocolatey — first submission and moderation

The workflow `.github/workflows/choco-publish.yml` pushes packages to
[chocolatey.org](https://chocolatey.org) automatically. The first submission goes through
a **manual moderation queue** that typically takes **1–3 business days**.

### 3a. Set the API key secret

1. Create or log in to your account at [chocolatey.org](https://chocolatey.org)
2. Navigate to **Account → API Keys** and generate a key
3. In GitHub: **Settings → Secrets and variables → Actions → New repository secret**
   - Name: `CHOCO_API_KEY`
   - Value: the API key from chocolatey.org

### 3b. Check moderation status

After pushing, visit:

```
https://chocolatey.org/packages/agentdb/<version>
```

Or search: `https://chocolatey.org/packages?q=agentdb`

The package status will show `Pending` until a moderator approves it. You may receive an
email with reviewer feedback; respond and re-submit if changes are requested.

Once the first version is approved, subsequent versions may be auto-approved if the
package has a good track record.

---

## 4. crates.io — token rotation

The workflow `.github/workflows/publish.yml` publishes the `datacules-agentdb` crate.

### Generate a new publish token

1. Go to [crates.io/settings/tokens](https://crates.io/settings/tokens)
2. Click **New Token**
3. Name it something recognizable, e.g. `agentdb-ci-publish`
4. Select the **`publish-update`** scope (allows publishing new versions of existing
   crates; does not allow creating new crates or yanking)
5. Click **Create**
6. Copy the token immediately — it is shown only once

### Update the GitHub secret

1. In GitHub: **Settings → Secrets and variables → Actions**
2. Find `CARGO_REGISTRY_TOKEN` and click **Update**
3. Paste the new token and save

The workflow uses `cargo publish --package datacules-agentdb --token ${{ secrets.CARGO_REGISTRY_TOKEN }}`.

> **Note:** If you need to publish a brand-new crate name for the first time (not just a
> new version), use the `publish-new` scope instead, or omit scope restrictions entirely
> for the initial token, then rotate to `publish-update` afterward.
