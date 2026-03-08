# Deployment Setup (One-Time)

Steps to set up the deployment pipeline from scratch. This only needs to be done once per developer machine.

## Prerequisites

### 1. AWS CLI

Install via winget:
```
winget install Amazon.AWSCLI
```

### 2. AWS Credentials

Create an IAM access key for your user:
1. AWS Console → IAM → Users → your user → Security credentials → Create access key
2. Use case: "Command Line Interface (CLI)"
3. Description tag: `terraform-admin`

Configure the CLI:
```
aws configure
```
- Access Key ID: (from step above)
- Secret Access Key: (from step above)
- Default region: `us-east-1`
- Output format: `json`

Verify:
```
aws sts get-caller-identity
```

### 3. GitHub CLI

Install via winget:
```
winget install GitHub.cli
```

Authenticate:
```
gh auth login -h github.com
```

Choose "Login with a web browser" when prompted. Verify:
```
gh auth status
```

### 4. Terraform

Install via winget:
```
winget install HashiCorp.Terraform
```

Restart your shell after install to pick up the PATH change.

Verify:
```
terraform --version
```

## Create AWS Infrastructure

From the project root:

```
cd infra
terraform init
terraform plan    # review what will be created
terraform apply   # type "yes" to confirm
```

This creates: S3 bucket, CloudFront distribution, ACM certificate, Route53 DNS record, and GitHub OIDC role.

The ACM certificate validation may take a few minutes.

## Set GitHub Secrets

After `terraform apply` completes, set two secrets on the GitHub repository:

```
gh secret set AWS_DEPLOY_ROLE_ARN --body "$(cd infra && terraform output -raw deploy_role_arn)"
gh secret set CLOUDFRONT_DISTRIBUTION_ID --body "$(cd infra && terraform output -raw cloudfront_distribution_id)"
```

Or manually:
1. `terraform output` — shows the values
2. GitHub repo → Settings → Secrets and variables → Actions → New repository secret
3. Add `AWS_DEPLOY_ROLE_ARN` and `CLOUDFRONT_DISTRIBUTION_ID`

## Verify

Push to master. The GitHub Actions workflow will:
1. Build the Windows binaries
2. Upload them to S3
3. Invalidate CloudFront cache

Check `https://arcade.seanshubin.com` — the download page should appear.

## Teardown

To remove all AWS resources:
```
cd infra
terraform destroy
```
