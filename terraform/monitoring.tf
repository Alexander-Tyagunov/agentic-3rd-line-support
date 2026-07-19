# Cost guard: a billing budget with alert thresholds. Gated on billing_account
# because creating budgets needs the billing account id and billing.budgets
# permissions (often held at the billing-account/org level).

resource "google_billing_budget" "budget" {
  count           = var.billing_account != "" ? 1 : 0
  billing_account = var.billing_account
  display_name    = "agentic-3rd-line-support budget"

  budget_filter {
    projects = ["projects/${local.project_number}"]
  }

  amount {
    specified_amount {
      currency_code = "USD"
      units         = tostring(var.budget_amount_usd)
    }
  }

  threshold_rules {
    threshold_percent = 0.5
  }
  threshold_rules {
    threshold_percent = 0.9
  }
  threshold_rules {
    threshold_percent = 1.0
  }
}
