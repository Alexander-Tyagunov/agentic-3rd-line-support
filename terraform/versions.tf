terraform {
  required_version = ">= 1.6.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
    google-beta = {
      source  = "hashicorp/google-beta"
      version = "~> 6.0"
    }
  }

  # Remote state (recommended for anything beyond a throwaway). Create the bucket
  # first, then uncomment and `terraform init -migrate-state`.
  #
  # backend "gcs" {
  #   bucket = "YOUR-TF-STATE-BUCKET"
  #   prefix = "agentic-3rd-line-support"
  # }
}
