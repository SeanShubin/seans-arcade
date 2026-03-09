# Operations Reference

Where everything lives and how to check on it.

## Local Machine

| What | Location |
|------|----------|
| AWS credentials | `~/.aws/credentials` |
| AWS config (region) | `~/.aws/config` |
| Lightsail SSH key | `~/.ssh/lightsail-key.pem` |
| Terraform state | `infra/terraform.tfstate` (git-ignored) |
| Terraform lock | `infra/.terraform.lock.hcl` (git-ignored) |
| Arcade client config | `%APPDATA%\seans-arcade\config.toml` |
| Arcade client log | `%APPDATA%\seans-arcade\arcade.log` |

## Relay VM (relay.seanshubin.com)

SSH in: `scripts/ssh-relay.sh` or `scripts/ssh-relay.cmd`

| What | Location on VM |
|------|---------------|
| Identity registry | `/opt/arcade-relay/data/identity_registry.toml` |
| Chat logs | `/opt/arcade-relay/data/logs/` |
| Deploy script | `/usr/local/bin/deploy-relay.sh` |
| Docker container name | `arcade-relay` |

Runtime secrets (`RELAY_SECRET`, `S3_BUCKET`, AWS credentials) are **not** stored on the VM — they are injected as environment variables from GitHub Actions secrets during deployment. See [deployment-setup.md](deployment-setup.md#set-github-secrets).

### Common VM commands

```bash
# View relay logs (stdout)
sudo docker logs arcade-relay

# View relay logs (follow)
sudo docker logs -f arcade-relay

# Check relay status
sudo docker ps

# Restart relay
sudo docker restart arcade-relay

# View chat logs
ls /opt/arcade-relay/data/logs/

# View identity registry
cat /opt/arcade-relay/data/identity_registry.toml

# Reset a player's identity
# Edit identity_registry.toml and remove their entry, then restart
```

## AWS Console

| Service | What to look for |
|---------|-----------------|
| **Lightsail** → Instances → `arcade-relay` | VM status, metrics, browser SSH |
| **Lightsail** → Networking → `arcade-relay-ip` | Static IP (100.48.239.191) |
| **S3** → `arcade.seanshubin.com` | Client binaries and index.html |
| **CloudFront** → `E2CGL5D6QSNH8Y` | CDN status, invalidations |
| **ECR** → `arcade-relay` | Relay Docker images |
| **Route53** → `seanshubin.com` zone | DNS records for arcade and relay subdomains |
| **ACM** → `arcade.seanshubin.com` | SSL certificate status |
| **IAM** → Roles → `arcade-github-deploy` | CI deploy permissions |

## GitHub

| What | Where |
|------|-------|
| CI workflow | Actions tab, or `.github/workflows/ci.yml` |
| Deploy secrets | Settings → Secrets → Actions |
| `AWS_DEPLOY_ROLE_ARN` | IAM role ARN for OIDC |
| `CLOUDFRONT_DISTRIBUTION_ID` | `E2CGL5D6QSNH8Y` |
| `RELAY_SSH_KEY` | Lightsail SSH private key |
| `RELAY_SECRET` | Shared secret for relay handshake |
| `S3_BUCKET` | S3 bucket for chat persistence and admin |
| `AWS_ACCESS_KEY_ID` | IAM user access key for relay S3 access |
| `AWS_SECRET_ACCESS_KEY` | IAM user secret key for relay S3 access |

## Terraform Outputs

Run from `infra/`:
```
terraform output
```

| Output | Current value |
|--------|--------------|
| `site_url` | `https://arcade.seanshubin.com` |
| `relay_address` | `relay.seanshubin.com:7700` |
| `relay_ip` | `100.48.239.191` |
| `relay_instance_name` | `arcade-relay` |
| `ecr_repository_url` | `964638509728.dkr.ecr.us-east-1.amazonaws.com/arcade-relay` |
| `cloudfront_distribution_id` | `E2CGL5D6QSNH8Y` |
| `deploy_role_arn` | `arn:aws:iam::964638509728:role/arcade-github-deploy` |
| `s3_bucket` | `arcade.seanshubin.com` |

## Debugging Checklist

**Client can't connect to relay:**
1. Is the relay running? SSH in, `sudo docker ps`
2. Is the relay secret correct? `cat /opt/arcade-relay/relay-secret` vs client's `config.toml`
3. Is DNS resolving? `nslookup relay.seanshubin.com`
4. Is UDP 7700 open? Check Lightsail firewall in console
5. Is the client pointing at the right address? Check `relay_address` in `config.toml`

**CI deploy failed:**
1. Check GitHub Actions tab for error details
2. ECR push failed? Check IAM role permissions
3. SSM command failed? Check Lightsail instance is running, SSM agent is active
4. S3 upload failed? Check IAM role permissions

**Download page not updating:**
1. Check CloudFront invalidation completed (console → CloudFront → Invalidations)
2. Hard refresh browser (Ctrl+Shift+R)
3. Check S3 bucket contents in console
