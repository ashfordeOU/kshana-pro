# Payments webhook fixtures

Self-made webhook bodies in the shapes the two providers publish (Paddle Billing
notification events; Lemon Squeezy webhook events), with test IDs. No provider account
exists yet (owner gate 12), so none of these was recorded from a live sandbox; the tests
sign each body with the provider's published scheme and a test secret. Times are
replaced by the test (`{{OCCURRED}}`) so ordering can be exercised.
