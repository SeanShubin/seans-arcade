output "cloudfront_distribution_id" {
  value       = aws_cloudfront_distribution.site.id
  description = "CloudFront distribution ID (needed by CI for cache invalidation)"
}

output "s3_bucket" {
  value       = aws_s3_bucket.site.bucket
  description = "S3 bucket name (needed by CI for upload)"
}

output "site_url" {
  value = "https://${local.domain}"
}
