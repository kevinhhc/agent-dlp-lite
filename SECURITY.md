# Security

Version 0.2.0 is a private development candidate. It is an application hook guardrail, with temporary loopback confirmation and a bounded local decision journal. Read [the security model](docs/privacy-security.md) before evaluating it. Same-user malicious processes and host hook bypasses remain outside its enforcement boundary.

Report vulnerabilities privately to the repository owner through your existing trusted channel. Do not post credentials, customer prompts, transcripts, history previews or sensitive policy values in public issues. No dedicated reporting address or response guarantee is currently advertised.

Include the version, operating system, Agent product/version, a synthetic reproduction, expected/actual behavior and redacted logs. Check hook loading, confirmation timeout and host failure handling independently from detector behavior. Never test with production secrets.
