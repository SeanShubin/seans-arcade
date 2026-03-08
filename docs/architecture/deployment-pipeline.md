# Deployment Pipeline

How code goes from a git push to downloadable binaries at `arcade.seanshubin.com`.

## Pipeline Flow

```
git push master
    → GitHub Actions: build-windows job
        → cargo build --release (arcade, relay, arcade-cli)
        → upload artifacts
    → GitHub Actions: deploy job (depends on build)
        → download artifacts
        → authenticate to AWS via OIDC (no stored credentials)
        → aws s3 sync → uploads .exe files + index.html
        → aws cloudfront create-invalidation → clears CDN cache
    → arcade.seanshubin.com serves updated binaries
```

## AWS Infrastructure

Managed by Terraform in `infra/`. All resources are in `us-east-1` (required for CloudFront + ACM).

| Resource | Purpose |
|----------|---------|
| S3 bucket (`arcade.seanshubin.com`) | Stores the downloadable binaries and index.html |
| CloudFront distribution | CDN, HTTPS termination, edge caching |
| ACM certificate | SSL/TLS for `arcade.seanshubin.com` |
| Route53 A record | Points `arcade.seanshubin.com` to CloudFront |
| IAM OIDC provider | Trusts GitHub Actions tokens |
| IAM role (`arcade-github-deploy`) | Assumed by CI; scoped to this bucket + distribution |

The S3 bucket is private. Only CloudFront can read from it via Origin Access Control. No public S3 URLs.

## Authentication

GitHub Actions authenticates to AWS via OIDC federation:

1. GitHub mints a short-lived JWT for the workflow run
2. AWS validates the token against the GitHub OIDC provider
3. AWS issues temporary credentials scoped to the `arcade-github-deploy` role
4. The role is restricted to: S3 operations on the site bucket, CloudFront invalidation on the site distribution
5. The role can only be assumed from `refs/heads/master` of this repository

No long-lived AWS credentials are stored anywhere.

## GitHub Secrets

| Secret | Value | Source |
|--------|-------|--------|
| `AWS_DEPLOY_ROLE_ARN` | IAM role ARN | `terraform output deploy_role_arn` |
| `CLOUDFRONT_DISTRIBUTION_ID` | CloudFront distribution ID | `terraform output cloudfront_distribution_id` |

## Terraform Setup

One-time setup:

```
cd infra
terraform init
terraform plan
terraform apply
```

Terraform state is stored locally (`infra/terraform.tfstate`), excluded from git via `infra/.gitignore`.

## Cache Behavior

CloudFront default TTL is 300 seconds (5 minutes). Every deploy invalidates all paths (`/*`), so users get the new binaries immediately after deployment. Between deploys, CloudFront serves cached copies from edge locations.
