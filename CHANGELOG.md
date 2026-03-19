# Changelog

## [2026.3.18] - 2026-03-18

### Added
- Initial project scaffold
- Core playbook parser (TOML canonical format)
- YAML to TOML and TOML to YAML converter
- Module trait with 4 core modules: ark, argonaut, file, verify
- Module registry
- Local transport (shell exec, file copy/read)
- Transport trait for SSH and daimon (planned)
- Markdown playbook parser (extracts sections for hoosh translation)
- Daimon client (agent registration, fleet inventory)
- Hoosh client (NL to TOML translation)
- MCP server with 6 tools: sutra_apply, sutra_plan, sutra_check, sutra_inventory, sutra_translate, sutra_convert
- CLI with subcommands: apply, check, plan, translate, convert, inventory, modules, validate, nl
- Example playbooks in TOML, YAML, and Markdown formats
- Example inventory file
- CI/CD workflows (ci.yml, release.yml)
- 30+ tests across all crates
