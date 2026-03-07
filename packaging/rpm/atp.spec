Name:           atp
Version:        1.1.0
Release:        1%{?dist}
Summary:        Agentic Text Processor — successor to grep, sed, and awk
License:        AGPL-3.0-only
URL:            https://github.com/nervosys/AgenticTextProcessor
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  cargo >= 1.75.0
BuildRequires:  rust >= 1.75.0

%description
ATP is an agentic-first text processing toolkit that unifies grep, sed,
and awk functionality with a modern query language (AQL), structured
JSON output, pipeline composition, and AI agent integration via MCP.

Features:
- Unified CLI with 19 subcommands
- AQL query language with variables, control flow, and user-defined functions
- POSIX-compatible drop-in binaries (atp-grep, atp-sed, atp-awk)
- MCP server mode for AI agent integration
- VS Code extension with AQL syntax highlighting
- Regulatory compliance (FIPS, NIST SP 800-53, CMMC 2.0)
- Performance: parallel search (rayon), memory-mapped I/O
- Property-based testing and fuzz harness

%prep
%autosetup -n AgenticTextProcessor-%{version}

%build
cargo build --release --workspace

%install
install -Dm755 target/release/atp %{buildroot}%{_bindir}/atp
install -Dm755 target/release/atp-grep %{buildroot}%{_bindir}/atp-grep
install -Dm755 target/release/atp-sed %{buildroot}%{_bindir}/atp-sed
install -Dm755 target/release/atp-awk %{buildroot}%{_bindir}/atp-awk

%check
cargo test --workspace

%files
%license LICENSE
%doc README.md CHANGELOG.md ROADMAP.md
%{_bindir}/atp
%{_bindir}/atp-grep
%{_bindir}/atp-sed
%{_bindir}/atp-awk

%changelog
* Mon Apr 01 2026 Nervosys <opensource@nervosys.ai> - 1.1.0-1
- AQL v2: variables, conditionals, group-by, user-defined functions
- Configuration system (~/.atp/config.toml, .atprc)
- MCP server mode for AI agent integration
- Performance: parallel search, memory-mapped I/O
- VS Code extension with AQL syntax highlighting
- Property-based testing and fuzz harness
- Benchmark regression CI gate
- Homebrew, Scoop, Debian, RPM packaging

* Mon Mar 15 2026 Nervosys <opensource@nervosys.ai> - 1.0.0-1
- Initial release
