# Admin Dashboard (Superseded)

This design has been replaced by the [Admin CLI](admin-cli.md). The S3 data flow is unchanged — only the consumer changed from a static web dashboard to the `arcade-ops` binary. The rationale: a single CLI tool is simpler to build, requires no web hosting or browser auth, and the operator already has AWS credentials on their machine.
