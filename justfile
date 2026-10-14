sed := if os() == "macos" { "gsed" } else { "sed" }

[private]
default:
    @just --list

bump v:
    {{ sed }} -i -E "s|^(version = \").*\"|\1{{ v }}\"|" Cargo.toml
    {{ sed }} -i -E \
        "s|(LABEL org\.opencontainers\.image\.version=).*|\1{{ v }}|" \
        Dockerfile
    {{ sed }} -i  -E "s|(<span class=\"version\">v).*(</span>)|\1{{ v }}\2|" \
        index.html

    # regenerate Cargo.lock
    cargo build

fmt:
    cargo fmt
    just --fmt
