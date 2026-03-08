# --- ECR Repository ---

resource "aws_ecr_repository" "relay" {
  name                 = "arcade-relay"
  image_tag_mutability = "MUTABLE"
  force_delete         = true

  image_scanning_configuration {
    scan_on_push = false
  }
}

resource "aws_ecr_lifecycle_policy" "relay" {
  repository = aws_ecr_repository.relay.name

  policy = jsonencode({
    rules = [
      {
        rulePriority = 1
        description  = "Keep only last 3 images"
        selection = {
          tagStatus   = "any"
          countType   = "imageCountMoreThan"
          countNumber = 3
        }
        action = { type = "expire" }
      }
    ]
  })
}

# --- Lightsail Instance ---

resource "aws_lightsail_instance" "relay" {
  name              = "arcade-relay"
  availability_zone = "us-east-1a"
  blueprint_id      = "amazon_linux_2023"
  bundle_id         = "nano_3_0" # $5/month, 512MB, 2 vCPU, dual-stack (IPv4+IPv6)

  user_data = <<-EOF
    #!/bin/bash
    set -e

    # Install Docker
    yum install -y docker
    systemctl enable docker
    systemctl start docker

    # Install SSM agent (usually pre-installed on Amazon Linux 2023)
    yum install -y amazon-ssm-agent || true
    systemctl enable amazon-ssm-agent
    systemctl start amazon-ssm-agent

    # Install AWS CLI (for ECR login)
    yum install -y aws-cli

    # Create deploy script
    cat > /usr/local/bin/deploy-relay.sh <<'SCRIPT'
    #!/bin/bash
    set -e
    REGION=us-east-1
    ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
    ECR_URI="$ACCOUNT_ID.dkr.ecr.$REGION.amazonaws.com/arcade-relay"

    aws ecr get-login-password --region $REGION | docker login --username AWS --password-stdin "$ACCOUNT_ID.dkr.ecr.$REGION.amazonaws.com"
    docker pull "$ECR_URI:latest"
    docker stop arcade-relay 2>/dev/null || true
    docker rm arcade-relay 2>/dev/null || true
    docker run -d \
      --name arcade-relay \
      --restart unless-stopped \
      --network host \
      -e RELAY_SECRET \
      -v /opt/arcade-relay/data:/data \
      "$ECR_URI:latest" \
      relay --data-dir /data
    SCRIPT
    chmod +x /usr/local/bin/deploy-relay.sh

    mkdir -p /opt/arcade-relay/data

    # Relay secret is read from a file on the VM, set once via SSH
    cat > /usr/local/bin/get-relay-secret.sh <<'SCRIPT'
    #!/bin/bash
    cat /opt/arcade-relay/relay-secret 2>/dev/null || echo ""
    SCRIPT
    chmod +x /usr/local/bin/get-relay-secret.sh
  EOF
}

# --- Static IP ---

resource "aws_lightsail_static_ip" "relay" {
  name = "arcade-relay-ip"
}

resource "aws_lightsail_static_ip_attachment" "relay" {
  static_ip_name = aws_lightsail_static_ip.relay.name
  instance_name  = aws_lightsail_instance.relay.name
}

# --- Firewall: allow UDP 7700 ---

resource "aws_lightsail_instance_public_ports" "relay" {
  instance_name = aws_lightsail_instance.relay.name

  port_info {
    protocol  = "udp"
    from_port = 7700
    to_port   = 7700
  }

  # SSH for initial setup and SSM fallback
  port_info {
    protocol  = "tcp"
    from_port = 22
    to_port   = 22
  }
}

# --- DNS record ---

resource "aws_route53_record" "relay" {
  zone_id = data.aws_route53_zone.main.zone_id
  name    = "relay.seanshubin.com"
  type    = "A"
  ttl     = 300
  records = [aws_lightsail_static_ip.relay.ip_address]
}

# --- Outputs ---

output "relay_instance_name" {
  value       = aws_lightsail_instance.relay.name
  description = "Lightsail instance name (used by CI for SSM commands)"
}

output "relay_address" {
  value = "relay.seanshubin.com:7700"
}

output "ecr_repository_url" {
  value = aws_ecr_repository.relay.repository_url
}

output "relay_ip" {
  value = aws_lightsail_static_ip.relay.ip_address
}
