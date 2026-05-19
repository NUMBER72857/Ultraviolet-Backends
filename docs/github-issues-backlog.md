<!--
GitHub issue import backlog for Ultraviolet.
This file exists because GitHub API access was unavailable from the local
environment; it preserves the planned issue set so it can be created with the
companion script once authentication and network access are fixed.
-->

# GitHub Issues Backlog

These 150 issues cover the payment, reconciliation, payout, ledger, frontend,
scraping, AI, and ops hardening work needed before production.

1. [backend][stellar] Verify payment destination account matches invoice treasury account
2. [backend][stellar] Verify payment amount matches invoice gross atomic amount
3. [backend][stellar] Verify payment asset code is configured USDC
4. [backend][stellar] Verify payment asset issuer matches configured USDC issuer
5. [backend][stellar] Verify payment memo matches invoice payment memo
6. [backend][stellar] Enforce transaction hash uniqueness across verified payments
7. [backend][stellar] Reject failed Stellar transactions during reconciliation
8. [backend][stellar] Reject payments on wrong network passphrase
9. [backend][stellar] Persist observed ledger sequence for verified payments
10. [backend][stellar] Persist observed source account for verified payments
11. [backend][stellar] Record rejection reason for every rejected observed payment
12. [backend][stellar] Add Horizon transaction lookup client interface
13. [backend][stellar] Implement Horizon payment operation parser
14. [backend][stellar] Handle multi-operation transactions explicitly
15. [backend][stellar] Reject ambiguous multi-payment transactions
16. [backend][stellar] Support fee-bump transaction envelope lookup
17. [backend][stellar] Preserve inner transaction hash for fee-bump payments
18. [backend][stellar] Add muxed-account destination policy
19. [backend][stellar] Add muxed-account source capture
20. [backend][stellar] Add native XLM rejection path for USDC invoices
21. [backend][stellar] Add memo type validation for text memos
22. [backend][stellar] Add memo length validation against Stellar limits
23. [backend][stellar] Add Horizon timeout configuration
24. [backend][stellar] Add Horizon retry classification
25. [backend][stellar] Add Horizon outage audit event
26. [backend][reconciliation] Persist reconciliation cursor per treasury account
27. [backend][reconciliation] Resume polling from stored cursor after restart
28. [backend][reconciliation] Add cursor advancement transaction boundary
29. [backend][reconciliation] Poll Horizon payments for treasury account
30. [backend][reconciliation] Match observed payment to invoice by memo
31. [backend][reconciliation] Match observed payment to invoice by transaction hash submission
32. [backend][reconciliation] Add retry policy for transient Horizon failures
33. [backend][reconciliation] Add dead-letter state for unrecoverable reconciliation errors
34. [backend][reconciliation] Expire pending invoices after expires_at
35. [backend][reconciliation] Prevent expired invoices from becoming paid without explicit policy
36. [backend][reconciliation] Add reconciliation advisory audit logs
37. [backend][reconciliation] Add worker shutdown handling
38. [backend][reconciliation] Add worker concurrency guard
39. [backend][reconciliation] Add idempotent payment insertion on duplicate polling
40. [backend][reconciliation] Add metrics for verified and rejected payments
41. [backend][reconciliation] Add reconciliation lag metric
42. [backend][reconciliation] Add cursor table migration tests
43. [backend][reconciliation] Add payment_attempt retry count
44. [backend][reconciliation] Add retry backoff timestamp to payment_attempts
45. [backend][reconciliation] Add admin endpoint to inspect reconciliation status
46. [backend][payouts] Define signer isolation boundary for payout submission
47. [backend][payouts] Add payout worker configuration validation
48. [backend][payouts] Select queued payouts with SKIP LOCKED
49. [backend][payouts] Build Stellar payout transaction XDR
50. [backend][payouts] Submit signed payout transaction to Horizon
51. [backend][payouts] Persist submitted payout transaction hash
52. [backend][payouts] Record payout attempt for every submission
53. [backend][payouts] Add payout retry policy
54. [backend][payouts] Dead-letter repeatedly failing payouts
55. [backend][payouts] Reconcile submitted payout success
56. [backend][payouts] Reconcile payout destination account
57. [backend][payouts] Reconcile payout amount
58. [backend][payouts] Reconcile payout asset
59. [backend][payouts] Reconcile payout memo
60. [backend][payouts] Add payout failure audit logs
61. [backend][payouts] Add payout success audit logs
62. [backend][payouts] Add payout signer health check
63. [backend][payouts] Add signer unavailable failure mode
64. [backend][payouts] Add payout worker metrics
65. [backend][payouts] Add payout reconciliation test fixtures
66. [backend][auth] Add merchant bootstrap CLI for first owner
67. [backend][auth] Add PBKDF2 password hash generation CLI
68. [backend][auth] Add password change endpoint
69. [backend][auth] Add session revocation endpoint
70. [backend][auth] Add session listing endpoint for merchant users
71. [backend][auth] Add role-based authorization helper tests
72. [backend][auth] Add login rate limiting by email
73. [backend][auth] Add account lockout after repeated failures
74. [backend][auth] Add password reset token schema
75. [backend][auth] Add password reset endpoint
76. [backend][auth] Add merchant user invitation flow
77. [backend][auth] Add merchant user disable flow
78. [backend][auth] Add auth audit logs for login and logout
79. [backend][auth] Add session cleanup worker
80. [backend][auth] Add secure cookie option for browser clients
81. [backend][ledger] Add ledger transaction builder abstraction
82. [backend][ledger] Post ledger entries when payment is verified
83. [backend][ledger] Post merchant payable entry on verified payment
84. [backend][ledger] Post platform fee revenue entry on verified payment
85. [backend][ledger] Post treasury cash entry on verified payment
86. [backend][ledger] Post payout settlement ledger entries
87. [backend][ledger] Add invariant test for balanced entries per asset
88. [backend][ledger] Add immutable posted transaction tests
89. [backend][ledger] Add ledger event for invoice state transition
90. [backend][ledger] Add ledger event for payout state transition
91. [backend][ledger] Add ledger account seed validation
92. [backend][ledger] Add admin ledger transaction inspection endpoint
93. [backend][ledger] Add ledger reconciliation report query
94. [backend][ledger] Add duplicate ledger posting guard
95. [backend][ledger] Add metadata PII filter for ledger events
96. [backend][http] Replace in-memory rate limiter with distributed limiter
97. [backend][http] Add request ID propagation
98. [backend][http] Add JSON error schema tests
99. [backend][http] Add strict production CORS allowlist validation
100. [backend][http] Add body limit integration test
101. [backend][http] Add PII-safe tracing fields
102. [backend][http] Add structured audit log sink
103. [backend][http] Add readiness check for worker dependencies
104. [backend][http] Add graceful shutdown for HTTP and workers
105. [backend][http] Add API versioning policy document
106. [backend][tests] Add testnet valid payment end-to-end test
107. [backend][tests] Add testnet wrong destination failure test
108. [backend][tests] Add testnet wrong amount failure test
109. [backend][tests] Add testnet wrong asset failure test
110. [backend][tests] Add testnet wrong memo failure test
111. [backend][tests] Add duplicate hash failure test
112. [backend][tests] Add Horizon outage failure test
113. [backend][tests] Add payout submission failure test
114. [backend][tests] Add payout reconciliation success test
115. [backend][tests] Add expired invoice reconciliation test
116. [backend][tests] Add auth protected route integration tests
117. [backend][tests] Add idempotency conflict integration tests
118. [backend][tests] Add database migration smoke test
119. [backend][tests] Add ledger immutability migration test
120. [backend][tests] Add load test plan for invoice creation
121. [frontend][wallet] Build Freighter wallet connect flow
122. [frontend][wallet] Build payment XDR preview screen
123. [frontend][wallet] Submit signed XDR through Freighter
124. [frontend][wallet] Display Stellar transaction hash after submission
125. [frontend][checkout] Poll public invoice state until paid or expired
126. [frontend][checkout] Add checkout expired state
127. [frontend][checkout] Add checkout wrong network guidance
128. [frontend][checkout] Add payment submitted pending reconciliation state
129. [frontend][merchant] Add authenticated merchant login screen
130. [frontend][merchant] Add session persistence and logout
131. [frontend][merchant] Add invoice list empty state
132. [frontend][merchant] Add invoice detail payment attempts panel
133. [frontend][accessibility] Add keyboard navigation pass
134. [frontend][accessibility] Add screen reader labels for checkout controls
135. [frontend][accessibility] Add color contrast audit
136. [scraping] Define scraping worker isolation contract
137. [scraping] Add scraper job table schema
138. [scraping] Add scraper robots and rate policy
139. [scraping] Add scraper output review queue
140. [scraping] Add scraper failure audit logs
141. [ai] Define AI advisory boundary policy
142. [ai] Add AI recommendation provenance fields
143. [ai] Prevent AI from writing payment truth fields
144. [ai] Add admin review workflow for AI suggestions
145. [ai] Add prompt and output logging with PII filtering
146. [ops] Document environment contract for Railway deployment
147. [ops] Add production secret checklist
148. [ops] Add database backup and restore runbook
149. [ops] Add worker process deployment topology
150. [ops] Add incident runbook for payment reconciliation outage
