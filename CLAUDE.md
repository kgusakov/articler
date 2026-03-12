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

#### Error Handling
Each crate defines its own `error.rs` module using the `snafu` crate. There is no shared `result` crate — errors are crate-local and composed via `snafu`'s derive macros.

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

#### Crate: `types` (`crates/types/`)
Shared type aliases used across crates (zero dependencies):
- `Id = i64`: Type alias for entity IDs
- `ReadingTime = i32`: Type alias for reading time estimates

#### Crate: `token_storage` (`crates/token_storage/`)
Hybrid OAuth token storage (in-memory + database-backed):
- `TokenStorage`: Thread-safe token manager with `DashMap`
- `Claim`: Token payload with `user_id` and `client_id`
- `NewToken`: Generated token response (access_token, expires_in, refresh_token)
- Access tokens: in-memory with 1-hour expiration
- Refresh tokens: database-backed with 30-day expiration, persistent across server restarts
- Automatic garbage collection of expired tokens

#### Crate: `article_scraper` (`crates/article_scraper/`)
Web content extraction from HTML and PDF documents:
- `Scraper`: HTTP client with optional proxy support (`ALL_PROXY`)
- `Document`: Extracted article data (title, content_html, content_text, image_url, mime_type, language, published_at, reading_time)
- `extract()`: Fetches and parses a URL into a `Document`
- `extract_or_fallback()`: Gracefully handles errors with minimal fallback data
- Sub-modules: `html` (HtmlExtractor via `dom_smoothie`), `pdf` (PdfExtractor via `mupdf`), `helpers`

#### Crate: `app_state` (`crates/app_state/`)
Shared application state container for Actix-web:
- `AppState`: Holds database pool, token storage, scraper, and Handlebars instance
- `AppState::new(pool, scraper, handlebars)`: Factory constructor
- Re-exports `Claim`, `NewToken`, `TokenStorage` from `token_storage` crate

#### Crate: `auth` (`crates/auth/`)
User authentication logic extracted from the server crate:
- `find_user()`: User authentication with password verification
- Depends on: `db`, `helpers` crates

#### Crate: `wallabag_api` (`crates/wallabag_api/`)
Wallabag-compatible REST API handlers, extracted from the server crate:
- `wallabag/mod.rs`: Route configuration and shared types
- `wallabag/entries.rs`: Entry CRUD handlers and request/response types
- `wallabag/tags.rs`: Tag management handlers
- `oauth.rs`: OAuth2 token endpoint and authentication middleware
- `models.rs`: Domain models for API responses
  - `Entry`: Main article/entry model with URL, content, metadata
  - `Tag`: Article categorization
  - `Annotation` and `Range`: Article annotations (not yet fully implemented)

#### Crate: `server` (`crates/server/`)
HTTP/API layer with the main binary and web interface:

1. **Binary** (`src/main.rs`):
   - Application entry point named `articler`
   - Initializes database, runs migrations, starts HTTP server

2. **Web Layer** (`src/web/`):
   - `mod.rs`: Web scope configuration with session middleware (`actix-session` cookie-based sessions)
   - `fake_ui.rs`: Minimal HTML pages for Android app OAuth flow
   - `ui.rs`: Full web UI with login, article listing, and archiving
     - `/login`: Login page (Handlebars-rendered)
     - `/`: Index page showing unarchived articles with counters (unread, all, starred, archived)
     - `/do_login`: Login form submission handler
     - `/do_archive`: Archive article handler
     - Session-based authentication (stores `user_id` in cookie session)

3. **Application Setup** (`src/app.rs`):
   - `app()`: Creates Actix-web application with routes and static file serving
   - `http_server()`: Initializes HTTP server with cookie key for sessions
   - `init_handlebars()`: Registers Handlebars templates and partials

4. **Templating** (`crates/server/templates/`):
    - Handlebars templates for the web UI, compiled into binary via `include_str!`
    - Page templates: `page.hbs`, `page_article.hbs`, `page_articles.hbs`, `page_clients.hbs`
    - Partial templates: `login.hbs`, `navigation.hbs`, `article.hbs`, `article_cards.hbs`, `articles_and_categories.hbs`, `base.hbs`, `categories.hbs`, `clients.hbs`
    - OAuth flow templates: `fake_client_create.hbs`, `fake_client_create_result.hbs`, `fake_development.hbs`

5. **Static Assets** (`crates/server/static/`):
    - SVG icons for the web UI (All, Archived, Delete, FavoriteOff, FavoriteOn, Logo, MarkRead, MarkUnRead, Profile, Search, Settings, Tagged)
    - Embedded into binary at build time via `actix-web-static-files` and `build.rs`
    - Served at `/static/` path

6. **Build Script** (`build.rs`):
    - Uses `static-files` crate to generate embedded static resources from `./static` directory

7. **Tests** (`tests/`):
    - Integration tests with fixtures and expected JSON responses
    - Uses `sqlx::test` macro with migrations from workspace root

#### Crate: `cli` (`crates/cli/`)
CLI tool for administration:
- Binary name: `articlerctl`
- Depends on: `db`, `helpers` crates
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
- **Shared state**: `AppState` (in `app_state` crate) holds shared resources (pool, token storage, scraper, handlebars)
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

- **Use component classes for templates**: When designing new pages and templates, always use the component classes from `crates/server/tailwind/input.css` first — `btn-primary`, `btn-outline`, `btn-danger`, `btn-copy`, `btn-icon` for buttons; `input-field`, `input-underline`, `input-ghost`, `input-wrapper` for inputs. If a new pattern is needed, add a new class to `@layer components` in `input.css`. Only in rare cases should custom inline utility classes be written directly on an element.

## Notes

- The codebase uses Rust edition 2024
- Entry-tag relationship uses a LEFT JOIN pattern to fetch entries with their associated tags
- Session cookies are not marked secure by default (TODO for production HTTPS)
- NEVER remove TODO automatically, ask me about it ALWAYS
- When using DashMap - get/remove and etc. methods which have "**Locking behaviour:** May deadlock if called when holding any sort of reference into the map." in docks must be called only when no another references to the map exists in current thread

# Individual Preferences
- @~/.claude/articler.md