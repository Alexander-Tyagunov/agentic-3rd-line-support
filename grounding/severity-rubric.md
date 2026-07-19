# Severity rubric

The triage agent scores every ticket S1–S4 using this rubric. When in doubt,
round **up** one level and say why.

| Severity | Meaning | Examples |
|----------|---------|----------|
| **S1** | Customer-impacting outage or silent money/data loss; SLO breached now | Payment captures failing; orphaned transactions accumulating; auth fully down |
| **S2** | Serious degradation; SLO at risk; a clear correctness bug | Sustained error spike; DB pool exhaustion; inventory oversell |
| **S3** | Minor/contained; no immediate customer impact | Elevated latency within SLO; noisy but non-fatal errors |
| **S4** | Cosmetic / informational | Logging format issues; deprecation warnings |

Inputs to consider: SLO breach (yes/no), blast radius (one user vs all),
reversibility (silent data/money loss ranks higher), and rate/trend (rising vs flat).
