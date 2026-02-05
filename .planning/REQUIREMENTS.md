# Requirements: Dynamic Coinbase Switching for SV2 Pool

**Defined:** 2026-02-05
**Core Value:** Sub-100ms coinbase switching enables fair time-shared mining by allowing rapid address rotation while miners continuously work, without waiting for Bitcoin Core template updates.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### API

- [ ] **API-01**: API endpoint accepts POST /api/coinbase with {address, user_id} payload
- [ ] **API-02**: API validates Bitcoin address format using bitcoin crate parser
- [ ] **API-03**: API validates user_id constraints (max 128 chars, alphanumeric + underscore/hyphen)
- [ ] **API-04**: API returns proper HTTP status codes (200 success, 400 invalid input, 500 internal error, 503 concurrent update)
- [ ] **API-05**: API binds to localhost only (127.0.0.1) for MVP security

### Template

- [ ] **TMPL-01**: Pool recreates jobs from stored last_future_template with new coinbase outputs
- [ ] **TMPL-02**: Pool updates coinbase_outputs field in ChannelManagerData
- [ ] **TMPL-03**: Pool distributes NewMiningJob to all standard channels
- [ ] **TMPL-04**: All connected miners receive updated jobs within 100ms of API call
- [ ] **TMPL-05**: Integration test validates shares accepted after coinbase switch (confirms merkle path theory)

### Webhook

- [ ] **WEBHOOK-01**: Pool sends HTTP POST webhook when share validated
- [ ] **WEBHOOK-02**: Webhook payload includes user_id (from job-to-user mapping)
- [ ] **WEBHOOK-03**: Webhook payload includes share_hash, difficulty, job_id, timestamp
- [ ] **WEBHOOK-04**: Webhook payload includes is_block flag (true for BlockFound, false for Valid)
- [ ] **WEBHOOK-05**: Webhook payload includes channel_id, downstream_id, sequence_number
- [ ] **WEBHOOK-06**: Pool tracks job_id → user_id mapping in ChannelManagerData
- [ ] **WEBHOOK-07**: Job-to-user mapping handles lagging shares submitted after coinbase switch
- [ ] **WEBHOOK-08**: Webhook delivery uses fire-and-forget pattern (non-blocking)
- [ ] **WEBHOOK-09**: Webhook failures logged but don't block pool operation

### Concurrency

- [ ] **CONC-01**: Lock strategy prevents deadlock between API calls and template provider
- [ ] **CONC-02**: API calls and template updates can occur concurrently without data races
- [ ] **CONC-03**: Pool cleans up stale job-to-user mappings on SetNewPrevHash (prevents memory leak)
- [ ] **CONC-04**: Lock contention minimized to achieve sub-100ms broadcast requirement

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Channel Types

- **CHAN-01**: Extended channel support (NewExtendedMiningJob distribution)
- **CHAN-02**: Group channel support (aggregate mining across downstreams)
- **CHAN-03**: Full share validation proof to backend (requires extended channels)

### Production Hardening

- **PROD-01**: Bearer token authentication for API endpoint
- **PROD-02**: Rate limiting (token bucket: 10 req/sec, burst: 5)
- **PROD-03**: Advanced webhook retry with exponential backoff
- **PROD-04**: Webhook dead-letter queue for failed deliveries
- **PROD-05**: Metrics and monitoring (Prometheus/Grafana)
- **PROD-06**: Graceful degradation under load

### Deployment

- **DEPLOY-01**: Testnet deployment configuration
- **DEPLOY-02**: Mainnet deployment configuration
- **DEPLOY-03**: Multiple pool instance support (distributed state)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Extended channels | Defer until full share validation proof needed, standard channels sufficient for MVP |
| Group channels | Defer until multi-downstream aggregation needed, not required for MVP |
| External share validation | Trust boundary unclear, adds complexity, validate internally only for MVP |
| Sophisticated rate limiting | Basic protection sufficient, optimize after load testing |
| Multiple pool instances | Distributed state is complex, validate single-instance concept first |
| Frontend application | Separate project, this phase focuses on pool modifications only |
| Backend orchestration API | Separate project, this phase validates core pool feasibility |
| Sidecar proxy component | Separate project, manual API testing sufficient for MVP |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| API-01 | Phase 1 | Pending |
| API-02 | Phase 1 | Pending |
| API-03 | Phase 1 | Pending |
| API-04 | Phase 1 | Pending |
| API-05 | Phase 1 | Pending |
| TMPL-01 | Phase 1 | Pending |
| TMPL-02 | Phase 1 | Pending |
| TMPL-03 | Phase 1 | Pending |
| TMPL-04 | Phase 1 | Pending |
| TMPL-05 | Phase 1 | Pending |
| WEBHOOK-01 | Phase 2 | Pending |
| WEBHOOK-02 | Phase 2 | Pending |
| WEBHOOK-03 | Phase 2 | Pending |
| WEBHOOK-04 | Phase 2 | Pending |
| WEBHOOK-05 | Phase 2 | Pending |
| WEBHOOK-06 | Phase 2 | Pending |
| WEBHOOK-07 | Phase 2 | Pending |
| WEBHOOK-08 | Phase 2 | Pending |
| WEBHOOK-09 | Phase 2 | Pending |
| CONC-01 | Phase 1 | Pending |
| CONC-02 | Phase 1 | Pending |
| CONC-03 | Phase 2 | Pending |
| CONC-04 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 27 total
- Mapped to phases: 27/27 (100%)
- Unmapped: 0

---
*Requirements defined: 2026-02-05*
*Last updated: 2026-02-05 after roadmap creation*
