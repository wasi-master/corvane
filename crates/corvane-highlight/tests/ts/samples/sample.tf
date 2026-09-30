# Static website bucket with optional access logging.
terraform {
  required_version = ">= 1.5"
  required_providers {
    aws = { source = "hashicorp/aws", version = "~> 5.0" }
  }
}

variable "site_name" {
  type        = string
  description = "Name prefix for all resources"
  default     = "docs"
}

variable "enable_logging" { default = true }

locals {
  bucket_name = "${var.site_name}-${terraform.workspace}"
  tags        = { Project = var.site_name, ManagedBy = "terraform" }
  /* Only list the extensions we serve. */
  extensions = ["html", "css", "js"]
}

resource "aws_s3_bucket" "site" {
  bucket = local.bucket_name
  tags   = merge(local.tags, { Tier = "web" })
}

resource "aws_s3_bucket_logging" "site" {
  count         = var.enable_logging ? 1 : 0
  bucket        = aws_s3_bucket.site.id
  target_bucket = aws_s3_bucket.site.id
  target_prefix = "logs/%{if var.site_name != ""}${var.site_name}/%{endif}"
}

// Outputs
output "types" {
  value = { for ext in local.extensions : ext => upper(ext) if length(ext) > 2 }
}

output "note" {
  value = <<-EOT
    Bucket ${aws_s3_bucket.site.bucket} serves ${length(local.extensions)} types.
  EOT
}
