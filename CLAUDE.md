# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

articler is a Rust implementation of a Wallabag-compatible read-it-later service. It provides a REST API and a web UI for managing reading list entries with support for tagging, archiving, and starring articles.

## Build and Development Commands

### Building
```bash
# Build entire workspace
cargo build
cargo build --release

# Build specific crate
cargo build -p server
cargo build -p db
cargo build -p helpers
```

### Running the Application
```bash
# Set required environment variables
export DATABASE_URL=sqlite://articler.db
export HTTP_PORT=8080
export COOKIE_KEY=your-secret-key-at-least-64-bytes-long
cargo run -p server

# Or run the binary directly by name
cargo run --bin articler
```

Required environment variables:
- `DATABASE_URL`: SQLite database connection string (e.g., `sqlite://articler.db`)
- `HTTP_PORT`: Port number for the HTTP server
- `COOKIE_KEY`: Secret key for session cookie encryption

Optional environment variables:
- `ALL_PROXY`: Proxy URL for the scraper (e.g., `socks5://localhost:1080`)
- `RUST_LOG`: Log level for `env_logger`

### Testing

Run all tests:
```bash
cargo test
```

Run tests for a specific crate:
```bash
cargo test -p server
cargo test -p db
cargo test -p helpers
```

Run integration tests only (located in `crates/server/tests/`):
```bash
cargo test -p server --test api_test
cargo test -p server --test oauth_test
cargo test -p server --test web_ui_test
```

Run a specific test:
```bash
cargo test test_get_entries
```

Integration tests use `sqlx::test` macro with migrations from `migrations/` directory (at workspace root) and fixtures from `crates/server/tests/fixtures/`. Each test creates a fresh database, applies migrations, loads fixtures, and compares responses against expected JSON files in `crates/server/tests/json/` using `serde_json_assert::assert_json_eq!`.

Tests are organized by functionality:
- `crates/server/tests/api_test.rs`: Entry and tag API endpoint tests
- `crates/server/tests/oauth_test.rs`: OAuth token and authentication tests
- `crates/server/tests/mobile_app_ui_stub.rs`: Tests for minimal HTML UI endpoints used in mobile OAuth flow
- `crates/server/tests/web_ui_test.rs`: Web UI integration tests (login, index, archiving) using `scraper` crate for HTML parsing

### Database Migrations

Migrations are located in the `migrations/` directory and are automatically applied during tests using sqlx's testing framework. The schema includes tables for entries, tags, entry_tags (junction table), users, clients, and tokens.

## Architecture

### Multi-Crate Architecture

The application is organized as a Cargo workspace with multiple crates, following a clean, module-based architecture with functional async functions instead of trait-based OOP patterns:

#### Crate: `result` (`crates/result/`)
Shared error handling types used across all crates:
- `ArticlerResult<T>`: Application-wide result type alias
- `ArticlerError`: Custom error type with source tracking and caller location
- Automatic conversion from any `std::error::Error` type via `From` trait
- Uses `#[track_caller]` to capture error origin location
- Implements `ResponseError` for Actix-web integration (due to orphan rule)

#### Crate: `helpers` (`crates/helpers/`)
Utility functions shared across the application (password hashing/verification, UID generation, string hashing).

#### Crate: `db` (`crates/db/`)
Database layer with repository modules for data access:
- **Repository Layer** (`repository/`): Database access with module-based functions
  - `users.rs`: User authentication functions (`find_by_username`, `find_by_id`)
  - `clients.rs`: OAuth client management functions
  - `entries.rs`: Entry CRUD operations, filtering, and querying
  - `tags.rs`: Tag management and linking to entries
  - `tokens.rs`: Database-backed token storage (create, find, delete, expire)
  - All functions accept `&SqlitePool` or `impl Executor` for database access
  - No traits - just pure async functions
- Re-exports `ArticlerError` and `ArticlerResult` from `result` crate for convenience

#### Crate: `server` (`crates/server/`, formerly `api`)
HTTP/API layer with the main binary and web interface:

1. **Binary** (`src/main.rs`):
   - Application entry point named `articler`
   - Initializes database, runs migrations, starts HTTP server

2. **REST Layer** (`src/rest/`):
   - `wallabag/`: Wallabag-compatible REST API endpoints
     - `mod.rs`: Route configuration and shared types
     - `entries.rs`: Entry CRUD handlers and request/response types
     - `tags.rs`: Tag management handlers
   - `oauth.rs`: OAuth2 token endpoint and authentication middleware

3. **Web Layer** (`src/web/`):
   - `mod.rs`: Web scope configuration with session middleware (`actix-session` cookie-based sessions)
   - `fake_ui.rs`: Minimal HTML pages for Android app OAuth flow
   - `ui.rs`: Full web UI with login, article listing, and archiving
     - `/login`: Login page (Handlebars-rendered)
     - `/`: Index page showing unarchived articles with counters (unread, all, starred, archived)
     - `/do_login`: Login form submission handler
     - `/do_archive`: Archive article handler
     - Session-based authentication (stores `user_id` in cookie session)

4. **Models Layer** (`src/models.rs`): Domain models for API responses
   - `Entry`: Main article/entry model with URL, content, metadata
   - `Tag`: Article categorization
   - `Annotation` and `Range`: Article annotations (not yet fully implemented)

5. **Authentication** (`src/auth.rs`):
   - `find_user()`: User authentication with password verification

6. **Application State** (`src/app.rs`):
   - `AppState`: Holds database pool, token storage, scraper, and Handlebars instance
   - `app()`: Creates Actix-web application with routes and static file serving
   - `http_server()`: Initializes HTTP server with cookie key for sessions
   - `app_state_init()`: Factory function for `AppState`
   - `init_handlebars()`: Registers Handlebars templates and partials

7. **Token Storage** (`src/token_storage.rs`):
   - Hybrid OAuth token storage (in-memory + database-backed)
   - Access tokens: in-memory with 1-hour expiration
   - Refresh tokens: database-backed with 30-day expiration, persistent across server restarts
   - Automatic cleanup of expired tokens

8. **Scraper** (`src/scraper.rs`):
   - Web scraping functionality for article content
   - Uses `dom_smoothie` and `reqwest`

9. **Templating** (`templates/`):
    - Handlebars templates for the web UI, compiled into binary via `include_str!`
    - `index.hbs`: Main layout template with partial inclusion
    - `login.hbs`: Login form partial
    - `main.hbs`: Article listing partial
    - `navigation.hbs`: Navigation bar partial with counters

10. **Static Assets** (`static/`):
    - SVG icons for the web UI (All, Archived, Delete, FavoriteOff, FavoriteOn, Logo, MarkRead, MarkUnRead, Profile, Search, Settings, Tagged)
    - Embedded into binary at build time via `actix-web-static-files` and `build.rs`
    - Served at `/static/` path

11. **Build Script** (`build.rs`):
    - Uses `static-files` crate to generate embedded static resources from `./static` directory

12. **Tests** (`tests/`):
    - Integration tests with fixtures and expected JSON responses
    - Uses `sqlx::test` macro with migrations from workspace root

#### Crate: `cli` (`crates/cli/`)
CLI tool for administration:
- Binary name: `articlerctl`
- Depends on: `result`, `db`, `helpers` crates
- Uses `clap` for command-line argument parsing
- Commands:
  - `create-user`: Creates a new user with username, password, name, and email
  - `create-client`: Creates an OAuth client for an existing user, returns client_id and client_secret

### Data Flow

Request → HTTP handler → Repository function → Database

The API layer converts between:
- HTTP query parameters (e.g., `EntriesRequest`) → repository parameters (e.g., `EntriesCriteria`)
- Database rows (e.g., `EntryRow`, `TagRow`) → API models (`Entry`, `Tag`)

### Key Design Patterns

- **Module-based functions**: Database access through standalone async functions instead of traits
- **Direct pool passing**: Functions accept `&SqlitePool` or `impl Executor<'_, Database = Db>`
- **Type conversions**: Clean separation between database types and API types using `From`/`TryFrom`
- **Shared state**: `AppState` holds shared resources (pool, token storage, scraper, handlebars)
- **Embedded assets**: Static files and templates compiled into the binary for single-binary deployment
- **Session-based auth**: Web UI uses cookie sessions via `actix-session`; REST API uses OAuth tokens

### Important Data Conversions

- Booleans: API uses integer representation (0/1) via `BoolFromInt` serde attribute
- Timestamps: Stored as Unix timestamps (i64), converted to `DateTime<Utc>` for API responses
- Tags: Comma-separated strings in query params, parsed via `StringWithSeparator`
- URLs: String in database, parsed to `Url` type for API responses

### Entry Query Filtering

The `/api/entries` endpoint supports filtering by:
- `archive`: archived status (0/1)
- `starred`: starred status (0/1)
- `public`: public visibility (0/1)
- `since`: Unix timestamp for entries updated after
- `page` and `perPage`: pagination
- `sort`: column to sort by (created/updated/archived)
- `order`: sort direction (asc/desc)
- `detail`: response detail level (metadata/full)
- `tags`: comma-separated tag filter (not yet implemented)
- `domain_name`: filter by domain (not yet implemented)


## Coding Style

- **No code comments**: Do not write inline code comments (`//` or `/* */`). Code should be self-explanatory through clear variable names, function names, and structure. Only write doc-comments (`///` or `//!`) when necessary for public API documentation.

## Notes

- The codebase uses Rust edition 2024
- Entry-tag relationship uses a LEFT JOIN pattern to fetch entries with their associated tags
- Session cookies are not marked secure by default (TODO for production HTTPS)
- NEVER remove TODO automatically, ask me about it ALWAYS

# Individual Preferences
- @~/.claude/articler.md