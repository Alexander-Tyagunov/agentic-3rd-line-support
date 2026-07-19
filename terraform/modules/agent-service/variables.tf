variable "name" {
  type        = string
  description = "Cloud Run service name."
}

variable "project_id" {
  type        = string
  description = "GCP project ID."
}

variable "location" {
  type        = string
  description = "Region for the Cloud Run service."
}

variable "image" {
  type        = string
  description = "Container image URI. Defaults to the public Cloud Run 'hello' image so `plan` works before real images exist; CI overrides per deploy."
}

variable "service_account_id" {
  type        = string
  description = "account_id for the dedicated service account created for this service (must be 6-30 chars)."
}

variable "env" {
  type        = map(string)
  description = "Plain environment variables."
  default     = {}
}

variable "secret_env" {
  type = list(object({
    name    = string
    secret  = string # Secret Manager secret id
    version = string # e.g. "latest"
  }))
  description = "Environment variables sourced from Secret Manager."
  default     = []
}

variable "min_instances" {
  type        = number
  description = "Minimum instances (0 = scale to zero)."
  default     = 0
}

variable "max_instances" {
  type    = number
  default = 3
}

variable "cpu" {
  type    = string
  default = "1"
}

variable "memory" {
  type    = string
  default = "512Mi"
}

variable "cpu_idle" {
  type        = bool
  description = "true = CPU throttled between requests (default). false = CPU always allocated (needed for background work like the log flooder)."
  default     = true
}

variable "ingress" {
  type    = string
  default = "INGRESS_TRAFFIC_ALL"
}

variable "invokers" {
  type        = list(string)
  description = "IAM members granted roles/run.invoker on this service (e.g. a Pub/Sub push SA, the scheduler SA, or 'allUsers' for a public demo endpoint)."
  default     = []
}

variable "labels" {
  type    = map(string)
  default = {}
}
