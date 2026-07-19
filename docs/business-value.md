# Business value & goals

<samp>**Business value** &nbsp;·&nbsp; [Architecture](architecture.md) &nbsp;·&nbsp; [Setup](setup.md) &nbsp;·&nbsp; [Terraform](terraform.md) &nbsp;·&nbsp; [Deep-dive](article.md) &nbsp;·&nbsp; [↩ README](../README.md)</samp>

> A plain-language guide for product, engineering, and operations leaders — the
> problem this addresses, the value it creates, and how humans stay firmly in
> control. **No code required.**

<details open>
<summary><b>Contents</b></summary>

1. [The problem, in business terms](#1-the-problem-in-business-terms)
2. [What "agentic 3rd-line support" is](#2-what-agentic-3rd-line-support-is)
3. [The business value](#3-the-business-value)
4. [Staying in control — governance & guardrails](#4-staying-in-control--governance--guardrails)
5. [The cost model](#5-the-cost-model)
6. [Who it's for](#6-who-its-for)
7. [What this project is — and isn't](#7-what-this-project-is--and-isnt)
8. [The goal](#8-the-goal)

</details>

---

## 1. The problem, in business terms

When something breaks in production, the work is expensive and slow — and most of
it is repetitive:

- **Incidents cost money by the minute.** Lost transactions, degraded customer
  experience, and reputational risk all scale with how long a problem goes
  unnoticed and unresolved (your "mean time to resolution").
- **The people who *can* fix it are your scarcest resource.** Third-line support —
  the engineers who change the code — is where escalations pile up. Their time is
  expensive and finite.
- **Most of the toil is undifferentiated.** Reading logs, correlating events,
  writing up a clear bug report, and confirming it isn't a known duplicate is
  hours of manual work that rarely uses an engineer's real expertise.
- **Alert fatigue and duplicate escalations** bury the signal. The same issue gets
  re-reported; noise crowds out the things that actually matter.

The net effect: slow resolution, burned-out engineers, and a backlog full of
tickets that are hard to act on.

---

## 2. What "agentic 3rd-line support" is

It's a small team of AI **agents** that handle the repetitive first 80% of a
production incident, and hand the two decisions that carry real risk to a human:

1. **Detect** — agents watch the logs and surface real problems (including subtle
   ones a fixed alert would miss).
2. **Triage → ticket** — an agent writes a *complete* bug report (what happened,
   how to reproduce it, expected vs. actual behaviour, the evidence, a proposed
   fix) and checks it isn't a duplicate.
3. **A human approves** which findings become work. *(Gate 1.)*
4. **Fix** — a coding agent implements the smallest fix, adds a test, and opens a
   pull request. It **never** ships on its own.
5. **A human reviews and merges** the change. *(Gate 2.)*

> [!IMPORTANT]
> The goal is **not** to remove people. It is to move your engineers off the
> repetitive middle of the process and onto the two judgement calls that matter —
> *which problems are worth fixing* and *whether a fix is safe to ship.*

---

## 3. The business value

| Business goal | How the system delivers it |
|---------------|----------------------------|
| **Resolve incidents faster** | Detection and triage happen continuously and in seconds, not after someone notices — shrinking mean-time-to-resolution. |
| **Free up expensive engineers** | Agents do the log-reading, correlation, and write-up. Engineers arrive at a ready-to-act ticket — or a ready-to-review pull request. |
| **Stop duplicated work** | A dedup registry means the same issue is never re-filed; recurrences are recorded and auto-closed against the original. |
| **Consistent, actionable tickets** | Every ticket follows the same complete format (repro steps, evidence, severity, proposed fix) — no more half-written escalations. |
| **A full audit trail** | Every finding, decision, and outcome is logged. You can always answer "what did the system see, and what did we do about it?" |
| **Lower operational cost** | The whole stack scales to zero when idle and is torn down with one command — you pay for what you use. |
| **Reduced risk** | Nothing reaches production without human review; the system proposes, it never pushes. |

---

## 4. Staying in control — governance & guardrails

Autonomy is deliberately **dialled up where mistakes are cheap and reversible, and
down where they aren't.** That principle is what makes this safe to adopt:

- **Two human gates.** People decide *which findings become work* and *which code
  ships*. Every fix arrives as a reviewable pull request.
- **Nothing is silently dropped.** Every signal is recorded with its outcome
  (ticketed, closed-as-duplicate, or ignored-with-a-reason) — a complete,
  inspectable ledger.
- **Least-privilege and keyless.** Each component has only the access it needs, and
  there are no long-lived credentials in the cloud path.
- **Only synthetic data.** The reference implementation never touches real
  customer data.
- **Every step is visible and restartable.** Operators can see each agent's run
  history (success/failure, timing, logs) and re-run any step from a single
  console.

> [!NOTE]
> In risk terms: the agents draft; humans approve; everything is logged. That
> combination is what lets you get the efficiency of automation without ceding
> control of production.

---

## 5. The cost model

- **Pay-for-use.** Every component except the demo's log generator scales to zero
  when idle — there's no standing fleet to pay for.
- **A budget guardrail.** A spend alert can be enabled out of the box.
- **Ephemeral by design.** The entire environment is created and destroyed with one
  command each — ideal for evaluation, training, and demos without lingering cost.

---

## 6. Who it's for

- **Engineering & platform leaders** evaluating where agents can safely reduce
  operational load.
- **SRE / DevOps / support organisations** drowning in escalations and alert noise.
- **Product & innovation teams** who want a concrete, honest example of "agentic
  operations" rather than a slide deck.
- **Educators and architects** teaching how to build agent systems responsibly.

---

## 7. What this project is — and isn't

> [!IMPORTANT]
> This is an **open, educational reference implementation** — a working, runnable
> blueprint you can study, stand up, and adapt. It is **not** a turnkey commercial
> product, and it runs against a synthetic app rather than your real services.

What that means for a business reader:

- **It's a proof of the pattern**, end to end, that you can trust because you can
  see every part of it.
- **Adapting it to your stack is real work** — connecting your services, runbooks,
  and review process. The value here is the validated pattern and the guardrails,
  not a drop-in tool.
- **The economics are honest.** It's built to be stood up, demonstrated, and torn
  down cheaply — so you can evaluate the idea before committing to it.

---

## 8. The goal

The goal of **agentic support** is to let software organisations resolve
production problems faster and with less human toil — *without* giving up human
judgement over what matters.

The goal of **this project** is to show, concretely and honestly, how to build that
safely: where an AI agent genuinely helps, where deterministic automation or a
human is the better choice, and how to keep a person in control of every decision
that carries real risk.

---

For the *how* behind all of this, continue to the technical docs:
**[Architecture](architecture.md)** · **[Deep-dive article](article.md)** ·
**[Terraform, explained](terraform.md)** · **[Setup & runbook](setup.md)**.
