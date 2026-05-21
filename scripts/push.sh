#!/usr/bin/env bash
# Push to master and watch the deploy through to its real conclusion.
#
# A bare `git push` only starts the CI/CD pipeline (build, S3 upload, relay
# redeploy). This script pushes, finds the workflow run for the pushed commit,
# streams its live progress, then announces the verdict.
#
# The verdict comes from the run's recorded `conclusion`, never from the exit
# code of `gh run watch`: that command also exits non-zero on its own transient
# failures (a GitHub API blip, a dropped connection) while the run is still
# going, which would announce a still-running deploy as failed.

set -euo pipefail
cd "$(dirname "$0")/.."

git push

sha=$(git rev-parse HEAD)

# The run does not appear instantly after the push — poll for up to ~60s.
run_id=""
for ((i = 0; i < 30; i++)); do
    run_id=$(gh run list --limit 20 --json databaseId,headSha \
        --jq "[.[] | select(.headSha == \"$sha\")][0].databaseId // empty" \
        2>/dev/null || true)
    if [ -n "$run_id" ]; then break; fi
    sleep 2
done
if [ -z "$run_id" ]; then
    echo "No workflow run found for $sha after 60s"
    exit 1
fi

# Stream live progress. Its exit code is deliberately ignored (see header).
gh run watch "$run_id" || true

# Read the verdict only from the run's real conclusion. Poll until the run is
# genuinely "completed" — this also covers `gh run watch` bailing out early.
conclusion=""
for ((i = 0; i < 60; i++)); do
    result=$(gh run view "$run_id" --json status,conclusion \
        --jq '"\(.status)|\(.conclusion // "")"' 2>/dev/null || true)
    if [ "${result%%|*}" = "completed" ]; then
        conclusion="${result#*|}"
        break
    fi
    sleep 10
done

# Best-effort spoken feedback; degrades to a printed line and a terminal bell.
announce() {
    if command -v say >/dev/null 2>&1; then
        say "$1"
    elif command -v spd-say >/dev/null 2>&1; then
        spd-say "$1"
    elif command -v espeak >/dev/null 2>&1; then
        espeak "$1" >/dev/null 2>&1
    elif command -v powershell.exe >/dev/null 2>&1; then
        powershell.exe -NoProfile -Command \
            "Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).Speak('$1')" \
            >/dev/null 2>&1
    else
        printf '\a'
    fi
}

if [ "$conclusion" = "success" ]; then
    echo "deployed to production"
    announce "deployed to production" || true
    exit 0
else
    echo "Deploy did not succeed (conclusion: ${conclusion:-unknown})"
    announce "deploy failed" || true
    exit 1
fi
