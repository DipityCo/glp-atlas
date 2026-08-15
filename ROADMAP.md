# GLP Atlas Roadmap

GLP Atlas is an open-source GLP-1 dose and progress tracker. The app is AGPL-3.0
and always will be. Revenue comes from a hosted service that removes the work of
running it yourself, not from access to the app, its features, or a user's own
data.

---

## Free forever

A standing commitment, not a current-state description. These do not move to a
paid tier in any future version.

- Unlimited medication and peptide logging, brand or compounded; schedule and
  full history
- Titration schedules, injection sites, and medication supply
- Medication levels across the dosing cycle
- Reconstitution and draw-volume math
- Weight, measurements, symptoms and side effects, notes
- Basic reminders, including Home Assistant
- Core charts and trends
- Full data export (CSV and JSON) and full data deletion
- Privacy controls, no-account operation, self-hosting
- Accessibility, including large type and high contrast
- Any warning that is genuinely safety-relevant

## Paid

| Area | What you get |
|---|---|
| Sync | Managed encrypted multi-device sync, backups, restore history, device migration |
| Imports | Apple Health / Health Connect, smart scales, CGMs, wearables, nutrition and sleep apps |
| Analytics | Cross-variable correlations across symptoms, weight, sleep, activity, nutrition, and dose changes |
| Reports | Longitudinal PDF/CSV reports built for appointments |
| Sharing | Revocable, time-boxed, read-only links for a clinician or care team |
| Labs | Import results, graph over time, flag against the lab's own reference ranges |
| Smart logging | Photo-to-meal, label parsing, natural-language entry, auto-categorization |
| Automation | Rules like "prompt me to log symptoms the day after a scheduled injection" |
| Dashboards | Unlimited widgets, saved views, comparison periods, custom metrics |
| Timeline | Searchable unified history with generated observations |
| Travel | Timezone-aware schedules, packing checklists, medication documentation storage |
| Household | Family and caregiver profiles with separated records |
| Journal | Voice notes, attachments, tagging, full-text search, AI summaries of your own entries |
| Provenance | Audit history of when a value was entered, imported, edited, or deleted |
| Platform | Personal API tokens, webhooks, Shortcuts, spreadsheet sync |
| Hosted API | Published quotas on the hosted API; self-hosted stays unlimited |
| Cosmetic | Themes and customization |
| Beta | Opt-in early access to new analysis and visualization features |
| Support | Email support, migration help, priority triage |
| Membership | Supporter badge, roadmap voting, early builds |

## Never

- Automated dose-change recommendations, or anything that reads as one
- Diagnosis, or lab interpretation presented as diagnosis
- Advice to alter a medication schedule without clinician involvement
- Advertising, or selling or brokering user data
- Paywalled export, deletion, or safety features

---

## Capabilities

### Tracking

The free promise, with no account and nothing sent to a server we run.

- Local database and schema for doses, weight, measurements, symptoms, notes
- Logging flows for every tracked record type
- Titration schedules, injection sites, and medication supply
- Medication levels across the dosing cycle; reconstitution and draw-volume math
- Schedule and history; local reminders
- Core charts and trends

### Data ownership

- Export and import: CSV and JSON, complete and round-trippable
- A passcode or biometric gate on the app itself, for a device already unlocked
- Delete-everything that actually deletes everything
- Self-hosting documentation

Records go to the device and to whatever service the user chooses. The project
runs no server that receives them.

### Accessibility

Screen reader support, dynamic type, contrast, and motion reduction. Decoration
never carries meaning on its own.

### Safety framing

The wellness/medical-claim boundary, written down once and applied consistently
across copy. An observation states what the data shows; it does not recommend,
and it does not imply causation.

### Sync

- Accounts, encrypted sync, conflict resolution
- Automatic backups, restore history, device migration
- Billing and subscription management; cancellation leaves data intact and
  exportable
- The free tier gains no server dependency

### Imports

- Apple Health and Health Connect
- Smart scales, wearables, CGMs
- Nutrition and sleep apps
- Deduplication against manual entries; provenance recorded per value

Manual entry stays free.

### Reports, sharing, and labs

- Longitudinal PDF/CSV reports: weight, adherence, side effects, measurements,
  labs, notes
- Configurable date ranges and annotation
- Temporary, revocable, read-only share links
- Lab tracking: import results, graph over time, flag out-of-range using the
  lab's supplied reference ranges, attach educational explanations framed as
  context, never as interpretation

### The personal health timeline

One searchable timeline over every logged and imported source, with generated
observations grounded in the user's own data.

- Cross-variable correlation and historical pattern detection
- Full-text search across notes, journal, and events
- Custom dashboards: unlimited widgets, saved views, comparison periods
- Every observation traceable to the records that produced it

### Platform and automation

- Personal API tokens, webhooks, Shortcuts, spreadsheet sync
- Hosted API with published quotas; self-hosted remains unlimited
- Automation rules; basic reminders stay free, Home Assistant included
- Smart logging: photo-to-meal, label parsing, natural-language entry
- Journal: voice notes, attachments, tagging
- Provenance and audit history surfaced in the UI
- Travel tools: timezone-aware schedules, packing checklists, document storage
- Family and household profiles with properly separated records

### Professional

Dietitians, clinics, and coaches. Priced well above consumer, a materially
different product, and a much larger commitment.

- Client dashboards and rosters
- Consent management and revocation
- Bulk report generation
- Administrative controls, roles, audit logs

Handling identified health data on behalf of a provider brings compliance
obligations (HIPAA and BAAs in the US, GDPR/MDR considerations in the EU) that
touch hosting, logging, retention, subprocessors, and support.

---

## Pricing shape

| Tier | Price | Who |
|---|---|---|
| Free / self-hosted | $0 | Individuals; anyone technical; anyone who wants their records on their own device |
| Premium (hosted) | ~$5/mo, or ~$42 annually | Most users |
| Professional | Substantially higher, per seat | Dietitians, clinics, coaches |

A one-time lifetime option or supporter pricing is worth considering for the
open-source audience, which reacts poorly to subscriptions but well to funding
development directly.

---

## Risks and open questions

- **The encryption model.** End-to-end encryption is the strongest privacy claim
  and the strongest reason to pay, but it forecloses server-side processing,
  which AI summaries, some analytics, and clinician sharing need. The options are
  client-side-only processing, a documented and user-consented decryption
  boundary for specific features, or per-feature keys. The choice is expensive to
  revisit and users will ask, so it should be explicit and documented.
- **Regulatory framing.** Analytics, pattern detection, and lab flagging sit near
  the line between a general-wellness product and a regulated medical device.
  Observational language is the mitigation, applied consistently in copy, and
  worth review before anything in that area ships rather than after.
- **Scope.** The premium list is long. Sync, imports, and reports are the three
  that make the subscription worth buying; the rest are retention, not
  acquisition. Shipping those three well beats shipping twelve partly.
- **Licensing.** AGPL-3.0 keeps hosted forks honest, but contributions without a
  CLA make future relicensing effectively impossible. Decide before accepting
  substantial outside contributions.
- **Free-tier credibility.** The free-forever list only works if it is versioned
  and never quietly amended. It should live in the repo, and changes to it should
  be visible in history.
