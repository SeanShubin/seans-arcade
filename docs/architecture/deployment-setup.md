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

This creates:
- S3 bucket, CloudFront distribution, ACM certificate, Route53 DNS record (static site)
- ECR repository (relay Docker images)
- Lightsail VM with Docker and SSH agent (relay server)
- Static IP and DNS record for `relay.seanshubin.com`
- GitHub OIDC role with permissions for S3, CloudFront, ECR, and SSH

The ACM certificate validation may take a few minutes. The Lightsail VM takes 1-2 minutes to boot and run its user_data script.

## Set GitHub Secrets

After `terraform apply` completes, set all secrets on the GitHub repository. **All secrets live here** — nothing is stored on the VM. If the VM is destroyed and recreated, a CI push restores everything automatically.

### Infrastructure secrets (from Terraform)

```
gh secret set AWS_DEPLOY_ROLE_ARN --body "$(cd infra && terraform output -raw deploy_role_arn)"
gh secret set CLOUDFRONT_DISTRIBUTION_ID --body "$(cd infra && terraform output -raw cloudfront_distribution_id)"
gh secret set RELAY_SSH_KEY --body "$(cat ~/.ssh/lightsail-key.pem)"
```

### Relay runtime secrets

```
gh secret set RELAY_SECRET --body "your-relay-secret"
gh secret set S3_BUCKET --body "arcade.seanshubin.com"
gh secret set AWS_ACCESS_KEY_ID --body "your-iam-access-key-id"
gh secret set AWS_SECRET_ACCESS_KEY --body "your-iam-secret-access-key"
```

The `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` are for an IAM user with S3 read/write access to the `arcade.seanshubin.com` bucket. These are separate from the OIDC role used by CI — the relay needs long-lived credentials because it runs continuously on the VM, not in a GitHub Actions workflow.

Or manually:
1. `terraform output` — shows infrastructure values
2. GitHub repo → Settings → Secrets and variables → Actions → New repository secret
3. Add all seven secrets listed above

## Verify

Push to master. The GitHub Actions workflow will:
1. Build Windows, macOS, and Linux relay binaries (in parallel)
2. Upload client binaries to S3
3. Invalidate CloudFront cache
4. Build relay Docker image, push to ECR
5. Deploy relay to Lightsail VM via SSH

Check:
- `https://arcade.seanshubin.com` — download page with Windows and macOS binaries
- Run the client — it should connect to `relay.seanshubin.com:7700`

## Local Development

To run the relay locally instead of using the AWS relay:

```
RELAY_SECRET=test cargo run -p relay
```

Then set `relay_address = "127.0.0.1:7700"` in your local `config.toml`, or use a separate data dir:

```
arcade.exe --data-dir local/dev
```

And edit `local/dev/seans-arcade/config.toml` to point at localhost.

## Teardown

To remove all AWS resources:
```
cd infra
terraform destroy
```
