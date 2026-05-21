# Push to master and watch the deploy through to its real conclusion.
#
# A bare `git push` only starts the CI/CD pipeline (build, S3 upload, relay
# redeploy). This script pushes, finds the workflow run for the pushed commit,
# streams its live progress, then announces the verdict out loud.
#
# The verdict comes from the run's recorded `conclusion`, never from the exit
# code of `gh run watch`: that command also exits non-zero on its own transient
# failures (a GitHub API blip, a dropped connection) while the run is still
# going, which would announce a still-running deploy as failed.

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    git push
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $sha = (git rev-parse HEAD).Trim()

    # The run does not appear instantly after the push — poll for up to ~60s.
    $runId = $null
    for ($i = 0; $i -lt 30; $i++) {
        try {
            $raw = gh run list --limit 20 --json databaseId,headSha 2>$null
            if ($raw) {
                $runs = $raw | ConvertFrom-Json
                $match = $runs | Where-Object { $_.headSha -eq $sha } | Select-Object -First 1
                if ($match) {
                    $runId = $match.databaseId
                    break
                }
            }
        } catch {
            # transient gh / API error — keep polling
        }
        Start-Sleep -Seconds 2
    }
    if (-not $runId) {
        Write-Host "No workflow run found for $sha after 60s"
        exit 1
    }

    # Stream live progress. Its exit code is deliberately ignored (see header).
    gh run watch $runId

    # Read the verdict only from the run's real conclusion. Poll until the run
    # is genuinely "completed" — this also covers `gh run watch` bailing early.
    $conclusion = $null
    for ($i = 0; $i -lt 60; $i++) {
        try {
            $raw = gh run view $runId --json status,conclusion 2>$null
            if ($raw) {
                $run = $raw | ConvertFrom-Json
                if ($run.status -eq 'completed') {
                    $conclusion = $run.conclusion
                    break
                }
            }
        } catch {
            # transient gh / API error — keep polling
        }
        Start-Sleep -Seconds 10
    }

    Add-Type -AssemblyName System.Speech
    $speak = New-Object System.Speech.Synthesis.SpeechSynthesizer
    if ($conclusion -eq 'success') {
        $speak.Speak('deployed to production')
        exit 0
    } else {
        Write-Host "Deploy did not succeed (conclusion: $conclusion)"
        $speak.Speak('deploy failed')
        exit 1
    }
} finally {
    Pop-Location
}
