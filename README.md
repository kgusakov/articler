# Articler

A self-hosted, Wallabag-compatible read-it-later service. Lightweight, single-binary, SQLite-backed.

## Features

- **Wallabag API v2 compatibility** — use existing Android, iOS, and browser extension Wallabag clients out of the box
- **Web UI** — article listing, archiving, starring, and full-text search
- **Article extraction** — automatically scrapes and stores article content from URLs
- **Multi-user support**
- **Single-binary deployment** — all static assets and templates embedded in the binary

## Quick Start

### Alpine Linux

Download the `.apk` package from the [releases page](../../releases) and install it:

```sh
apk add --allow-untrusted articler.apk
```

Create your first user and an OAuth client for Wallabag apps:

```sh
su -s /bin/sh articler
source /etc/conf.d/articler
articlerctl create-user myusername mypassword "My Name" me@example.com
articlerctl create-client myusername "My Phone"
```

The `create-client` command prints the `client_id` and `client_secret` needed to connect a Wallabag app.

### Building from Source
#### Requirements
- cargo
- libclang and libfontconfig1 - to build mupdf dependency

    **macOS**

    ```sh
    xcode-select --install
    brew install llvm
    export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
    ```

    **Ubuntu / Debian**

    ```sh
    sudo apt-get install -y build-essential libclang-dev libfontconfig1-dev
    ```

- tailwindcss - to produce minified css with only needed classes

    **macOS**

    ```sh
    ARCH=$(uname -m | sed 's/x86_64/x64/;s/arm64/arm64/') && \
    curl -fsSL "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-macos-${ARCH}" \
    -o /usr/local/bin/tailwindcss && chmod +x /usr/local/bin/tailwindcss
    ```

    **Ubuntu / Debian**

    ```sh
    ARCH=$(uname -m | sed 's/x86_64/x64/;s/aarch64/arm64/') && \
    curl -fsSL "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-${ARCH}" \
    -o ~/.local/bin/tailwindcss && chmod +x ~/.local/bin/tailwindcss
    ```

Then build:

```sh
export TAILWIND_REBUILD=1
export TAILWIND_BIN=<tailwind binary path>
cargo build --release
```

Produces:
- `target/release/articler` — the HTTP server
- `target/release/articlerctl` — the CLI admin tool

## Configuration

| Variable       | Required | CLI | Server | Description                                      |
|----------------|----------|-----|--------|--------------------------------------------------|
| `DATABASE_URL` | Yes      | Yes | Yes    | SQLite connection string, e.g. `sqlite://articler.db` |
| `HTTP_PORT`    | Yes      | No  | Yes    | Port to listen on                                |
| `COOKIE_KEY`   | Yes      | No  | Yes    | Session encryption key (minimum 64 bytes)        |
| `ALL_PROXY`    | No       | No  | Yes    | SOCKS5 proxy URL for article scraping            |
| `RUST_LOG`     | No       | Yes | Yes    | Log level, e.g. `info`, `debug`                  |

## Wallabag Client Setup

Articler implements the Wallabag API v2.6.12. Any Wallabag-compatible client works:

- [Android app](https://github.com/wallabag/android-app)
- [iOS app](https://github.com/wallabag/ios-app)
- [Browser extensions](https://github.com/wallabag/wallabagger)

When configuring the client, use:
- **Server URL**: `https://your-host`
- **Client ID / Secret**: obtained from `articlerctl create-client` or web UI
