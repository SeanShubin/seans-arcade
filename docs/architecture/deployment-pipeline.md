# Deployment Pipeline

How code goes from a git push to running services.

## Pipeline Flow

```
git push master
    → GitHub Actions (parallel builds):
        → build-windows: cargo build --release (arcade, relay, arcade-ops)
        → build-macos: cargo build --release (arcade, relay, arcade-ops)
        → build-linux-relay: cargo build --release (relay only)
    → GitHub Actions: deploy job (depends on all builds)
        → authenticate to AWS via OIDC
        → upload client binaries to S3
        → invalidate CloudFront cache
        → build relay Docker image, push to ECR
        → deploy relay to Lightsail VM via SSH
    → arcade.seanshubin.com serves updated client binaries
    → relay.seanshubin.com:7700 runs updated relay
```

## AWS Infrastructure

Managed by Terraform in `infra/`. All resources are in `us-east-1`.

| Resource | Purpose |
|----------|---------|
| S3 bucket (`arcade.seanshubin.com`) | Stores downloadable client binaries and index.html |
| CloudFront distribution | CDN, HTTPS termination, edge caching |
| ACM certificate | SSL/TLS for `arcade.seanshubin.com` |
| Route53 A record (`arcade.seanshubin.com`) | Points to CloudFront |
| Route53 A record (`relay.seanshubin.com`) | Points to Lightsail static IP |
| ECR repository (`arcade-relay`) | Stores relay Docker images (last 3 kept) |
| Lightsail instance (`arcade-relay`) | Runs the relay Docker container |
| Lightsail static IP | Stable IP for the relay VM |
| IAM OIDC provider | Trusts GitHub Actions tokens |
| IAM role (`arcade-github-deploy`) | Assumed by CI; scoped permissions |

## Authentication

GitHub Actions authenticates to AWS via OIDC federation:

1. GitHub mints a short-lived JWT for the workflow run
2. AWS validates the token against the GitHub OIDC provider
3. AWS issues temporary credentials scoped to the `arcade-github-deploy` role
4. The role can only be assumed from `refs/heads/master` of this repository

The role has permissions for: S3 (site bucket), CloudFront (invalidation), ECR (push images).

No long-lived AWS credentials are stored anywhere.

## GitHub Secrets

All secrets are stored as GitHub Actions repository secrets. No secrets are stored as files on the VM — the VM is fully disposable. See [architecture-decisions.md](../architecture-decisions.md#all-runtime-secrets-stored-in-github-actions-secrets) for rationale.

| Secret | Value | Source |
|--------|-------|--------|
| `AWS_DEPLOY_ROLE_ARN` | IAM role ARN for OIDC | `terraform output deploy_role_arn` |
| `CLOUDFRONT_DISTRIBUTION_ID` | CloudFront distribution ID | `terraform output cloudfront_distribution_id` |
| `RELAY_SSH_KEY` | Lightsail SSH private key | `~/.ssh/lightsail-key.pem` |
| `RELAY_SECRET` | Shared secret for relay handshake | Operator-chosen passphrase |
| `S3_BUCKET` | S3 bucket for chat persistence and admin | `arcade.seanshubin.com` |
| `AWS_ACCESS_KEY_ID` | IAM user access key for relay S3 access | IAM Console → Create access key |
| `AWS_SECRET_ACCESS_KEY` | IAM user secret key for relay S3 access | IAM Console → Create access key |

## Relay Deployment

The relay deploy flow:

1. CI builds the relay binary on `ubuntu-latest` (Linux x86_64)
2. CI packages it into a Docker image using `Dockerfile.relay`
3. CI pushes the image to ECR as `arcade-relay:latest`
4. CI SSHs into the Lightsail VM (key stored as `RELAY_SSH_KEY` GitHub secret)
5. The VM stops the old container and starts a new one with secrets injected as environment variables

All runtime secrets (`RELAY_SECRET`, `S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`) are passed from GitHub Actions secrets to the Docker container as `-e` flags. No secrets are stored on the VM filesystem.

Relay data (identity registry, logs) is stored at `/opt/arcade-relay/data` on the VM, mounted as a Docker volume.

## Cache Behavior

CloudFront default TTL is 300 seconds (5 minutes). Every deploy invalidates all paths (`/*`), so users get the new binaries immediately after deployment.
