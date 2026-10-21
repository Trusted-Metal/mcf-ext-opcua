#!/usr/bin/env bash
# Load RELEASE_DEPLOY_KEY and configure git to push over SSH.
# This lets @semantic-release/git bypass branch ruleset protections via the
# deploy key (which has "Always allow" bypass on the main and release rulesets).
set -euo pipefail

mkdir -p ~/.ssh
echo "$RELEASE_DEPLOY_KEY" > ~/.ssh/deploy_key
chmod 600 ~/.ssh/deploy_key

# Validate key and print fingerprint for log verification
FINGERPRINT=$(ssh-keygen -y -f ~/.ssh/deploy_key | ssh-keygen -E sha256 -lf -)
echo "Deploy key fingerprint: ${FINGERPRINT}"

eval "$(ssh-agent -s)"
ssh-add ~/.ssh/deploy_key

ssh-keyscan -t ed25519 github.com >> ~/.ssh/known_hosts 2>/dev/null

export GIT_SSH_COMMAND="ssh -i ~/.ssh/deploy_key -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes"

# Switch remote to SSH and export the URL for semantic-release --repository-url
REPO_URL="git@github.com:${GITHUB_REPOSITORY}.git"
git remote set-url origin "${REPO_URL}"
echo "SEMANTIC_RELEASE_REPOSITORY_URL=${REPO_URL}" >> "${GITHUB_ENV}"
echo "GIT_SSH_COMMAND=${GIT_SSH_COMMAND}" >> "${GITHUB_ENV}"

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
echo "Deploy key configured - push will use SSH as deploy key"
