# Security & Privacy Policy

This is a **public repository**. It must never contain:

- API keys, tokens, passwords, signing certificates or any credential
- Real bank statements, spreadsheets, database files, backups or exports
- Real names, RUTs, account/card numbers or amounts from anyone's finances

How the app handles secrets at runtime:
- Device keys, database encryption keys and optional AI provider keys live **only in the macOS Keychain**.
- Nothing is sent over the network except LAN sync between paired devices and features the user explicitly enables (AI provider, FX rates).

Guardrails: `.gitignore` blocks data files; CI runs a secret scanner (gitleaks) on every push and PR.

Found a vulnerability? Please open a GitHub security advisory (Security → Report a vulnerability) instead of a public issue.
