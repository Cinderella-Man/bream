# Pinned BEAM process observations

These fixtures investigate bounded reference questions. They do not implement or qualify Bream and are not the upstream Erlang/OTP test suite.

`bream_reference_probe.erl` isolates each family in a fresh process, captures observations, and cleans up spawned children. Each hibernation boundary runs in a separate VM. Notification-order fixtures wait until both notifications are materialized before consuming them, to distinguish mailbox order from receive timing. They use 100 samples per option combination; that is not exhaustive race coverage.

Exact commands, executable/source identities, results and limitations are recorded in the [parent evidence index](../README.md) and [research report 29](../../../docs/research/29-pinned-reference-observations.md). The separate ignored source-build directory leaves `upstream/otp` untouched. Crypto, SSH, SSL, Java, wx and ODBC support are disabled in the reference build; their algorithms are outside this investigation.
