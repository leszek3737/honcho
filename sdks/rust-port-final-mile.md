# Honcho Rust SDK — droga do v0.1 (fazy 10-12) — v2

Granularny rozpis zadań zamykających port Rust SDK. Punkt wyjścia: fazy 0-9 z [rust-port-tdd-plan.md](rust-port-tdd-plan.md) zaimplementowane.

> Wersja **v2** zastępuje pierwszą wersję dokumentu. Zmiany: dodano brakujące luki z faz 0-9 (SSE cancel-safety, parity fixtures, anemiczne `compile_assertions`), skonkretyzowano komendy, usunięto wymyślony `xtask` (sdks/rust nie jest workspace'em), zdjęto fałszywe pozytywy (przykład buduje się OK, MSRV bez matrix), dodano decyzje (nazwa crate'u, LICENSE, Debug derives).

---

## 0. Stan faktyczny (zweryfikowany)

| Co | Status |
|---|---|
| `cargo build` | ✅ |
| `cargo test` | ⚠️ 192/204 OK; 12 fail w `conclusion_types` (broken openapi path po `f19f34e`) |
| Liczba testów | 204 w 23 binarach |
| `sdks/rust` jest Cargo workspace | ❌ nie — standalone crate |
| `LICENSE-APACHE` w `sdks/rust/` | ❌ brak (README się odwołuje do nieistniejącego pliku) |
| `CHANGELOG.md` w `sdks/rust/` | ❌ brak |
| CI workflow `rust.yml` | ❌ brak (są tylko python/docker workflows) |
| `tracing` feature wired | ⚠️ jeden callsite (`src/http/sse.rs:176`) |
| `compile_assertions.rs` | ⚠️ anemiczne — tylko `HttpClient`, `Conclusion`, `ConclusionScope`; brak `Honcho`, `Peer`, `Session`, `Message`, `HonchoError` |
| Debug derive na `Peer`, `DialecticStream` | ❌ brak (zamierzone? nieudokumentowane) |
| SSE cancel-safety test | ❌ brak — tylko pagination ma `drop(stream)` (gap faza 8) |
| Python-source fixtures dla `to_openai`/`to_anthropic` | ❌ brak — testy ręczne, nie pochodzą z Python SDK (gap faza 6 / parity matrix) |
| Crate name | `honcho-ai` (plan v2 pisze `honcho` — niespójność) |
| Nazwa branchy worktree | `claude/zen-shannon-71f443` |

---

## Faza A — Cleanup zaległości faz 0-9 (1 dzień)

Bez tego CI nie startuje a niezmienniki TDD z planu są fikcyjne.

### A.1 Fix OpenAPI fixture path

[tests/common/mod.rs:14](sdks/rust/tests/common/mod.rs:14):

```rust
// before:
let path = Path::new(&manifest_dir).join("../../../docs/v3/openapi.json");
// after:
let path = std::env::var("HONCHO_OPENAPI_SPEC")
    .map(PathBuf::from)
    .unwrap_or_else(|_| Path::new(&manifest_dir).join("../../docs/v3/openapi.json"));
```

- **DoD:** `cargo test --test conclusion_types` 24/24 OK.
- **Czas:** 0.25h

### A.2 Fix dead_code w `tests/decode_error.rs`

Pole `id` w `RequiresString` jest tam celowo (sprawdza ścieżkę dekodera), więc `#[allow(dead_code)]` z jednoliniowym komentarzem WHY.

- **DoD:** `cargo test 2>&1 | grep warning` puste.
- **Czas:** 0.1h

### A.3 SSE cancel-safety test (gap faza 8)

Plan faza 8: "Cancel-safety: `tokio::pin!(stream); stream.next().await; drop(stream);` — wiremock widzi disconnect w <100ms." Test nie istnieje, tylko pagination ma drop test.

- [ ] `tests/sse_cancel.rs`: wiremock zwraca slow SSE (1 chunk, potem 5s `tokio::time::sleep`), klient czyta jeden chunk, dropuje strumień. Assert: TCP `Connection: close` widoczny w mockserver expectations <100ms od dropa.
- [ ] Drugi test: malformed JSON w środku → `Err` w streamie, brak panic.
- **DoD:** 2 testy w `tests/sse_cancel.rs` zielone.
- **Czas:** 1h

### A.4 Import `to_openai`/`to_anthropic` fixtures z Python SDK

Plan section 17 parity matrix: "shape per fixture / strukturalnie równe (fixtures z Pythona)". Obecne testy w [tests/session_context.rs](sdks/rust/tests/session_context.rs) są ręcznie pisane — nie udowadniają parity.

- [ ] Skrypt `sdks/rust/tests/fixtures/parity/_generate.py` używający `honcho` Python SDK do wytworzenia kanonicznych shape'ów: 3 scenariusze × 2 formaty = 6 JSON files w `tests/fixtures/parity/`
- [ ] `tests/session_context_parity.rs` ładuje fixtures, buduje analogiczny `SessionContext` w Ruście, porównuje canonical JSON.
- [ ] Skrypt uruchamiany ręcznie (`uv run python tests/fixtures/parity/_generate.py`), regenerowany przy zmianach API.
- **DoD:** 6 testów parity zielone.
- **Czas:** 2h

### A.5 Rozszerzyć `compile_assertions.rs`

Obecnie 3 asercje, brak `Honcho`, `Peer`, `Session`, `Message`, `HonchoError`, return types `Send + 'static`.

- [ ] Dodać:
  ```rust
  assert_impl_all!(honcho_ai::Honcho: Send, Sync, Clone);
  assert_impl_all!(honcho_ai::Peer: Send, Sync, Clone);
  assert_impl_all!(honcho_ai::Session: Send, Sync, Clone);
  assert_impl_all!(honcho_ai::Message: Send, Sync, Clone, Debug);
  assert_impl_all!(honcho_ai::error::HonchoError: Send, Sync, std::error::Error + 'static);
  ```
- [ ] Dla return types: jeden bound assertion-test per critical path (paginate, chat_stream, search) — `fn _check<F: Future + Send + 'static>(_: F){}`.
- **DoD:** `cargo test --test compile_assertions` zielony, ≥10 asercji.
- **Czas:** 0.5h

### A.6 Debug derive decyzja: `Peer`, `DialecticStream`

Brak `Debug` na `Peer` ([src/peer.rs](sdks/rust/src/peer.rs)) i `DialecticStream` ([src/dialectic_stream.rs](sdks/rust/src/dialectic_stream.rs)) — guideline Rust API C-DEBUG.

- [ ] `Peer`: dodać ręczny `impl Debug` który nie loguje pól z cache (race condition na RwLock); print `Peer { id, workspace_id }`.
- [ ] `DialecticStream`: `Debug` z polem `state: <hidden>` (Stream + reqwest::Response nie są Debug, więc opaque).
- **DoD:** `assert_impl_all!(Peer: Debug)` + `assert_impl_all!(DialecticStream: Debug)` zielone.
- **Czas:** 0.5h

### A.7 `#[non_exhaustive]` audit

Skrypt sprawdza że każdy `pub enum`/`pub struct` w `src/types/` ma `#[non_exhaustive]`:

```bash
# wykryj brakujące
rg "^pub (enum|struct)" sdks/rust/src/types/ -B1 | rg -B1 -v non_exhaustive
```

- [ ] Uzupełnić brakujące (jeśli są) — patrząc na obecny grep: 14 plików ma, niektóre typy w `types/common.rs` (7 linii) prawdopodobnie nie.
- **DoD:** powyższy `rg` zwraca pustkę.
- **Czas:** 0.5h

### A.8 Rust CI workflow

Nowy plik `.github/workflows/rust-sdk.yml`:

```yaml
name: rust-sdk
on:
  pull_request:
    paths: ['sdks/rust/**', 'docs/v3/openapi.json', '.github/workflows/rust-sdk.yml']
  push:
    branches: [main]
    paths: ['sdks/rust/**', 'docs/v3/openapi.json']
jobs:
  test:
    runs-on: ubuntu-latest
    defaults: { run: { working-directory: sdks/rust } }
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.80
      - uses: Swatinem/rust-cache@v2
        with: { workspaces: 'sdks/rust' }
      - run: cargo fmt --all --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo test --all-features
      - run: cargo doc --no-deps --all-features
        env: { RUSTDOCFLAGS: '-D warnings' }
  msrv:
    runs-on: ubuntu-latest
    defaults: { run: { working-directory: sdks/rust } }
    steps:
      - uses: actions/checkout@v4
      - uses: taiki-e/install-action@cargo-msrv
      - run: cargo msrv verify
```

- **DoD:** zielone na PR + branch protection wymaga.
- **Czas:** 1.5h (z debugiem cache + uprawnień)

### A.9 Decyzja: nazwa crate'u

Plan v2 mówi `honcho`, Cargo.toml ma `honcho-ai`. Trzy opcje:

| Opcja | Plus | Minus |
|---|---|---|
| Zostawić `honcho-ai` | nazwa już zajęta przez nas na crates.io (do sprawdzenia), spójna z `@honcho-ai/sdk` (npm) | rozjazd z planem |
| Zmienić na `honcho` | krótko, zgodnie z planem | konflikt z istniejącym crate'em `honcho` na crates.io (skontrolować!) |
| Dual-name (lib name `honcho`, package `honcho-ai`) | `use honcho::*` user-side | komplikacja |

- [ ] **Krok 1:** `cargo search honcho` i `cargo search honcho-ai` — sprawdzić co jest zajęte.
- [ ] **Krok 2:** decyzja maintainera. Zaktualizować plan v2 i Cargo.toml.
- **Czas:** 0.5h + czas na decyzję poza scope

### A.10 LICENSE-APACHE w `sdks/rust/`

README [sdks/rust/README.md:21](sdks/rust/README.md:21) linkuje `LICENSE-APACHE` ale plik nie istnieje. Skopiować z repo root lub uzupełnić.

- [ ] `cp /Users/leszek/git/honcho/LICENSE sdks/rust/LICENSE-APACHE` (lub `LICENSE`, zależnie od nazwy w root)
- [ ] Cargo.toml ma `license = "Apache-2.0"` ✅
- **DoD:** plik istnieje, `cargo publish --dry-run` go uwzględnia.
- **Czas:** 0.1h

**Faza A: ~7h ≈ 1 dzień** (więcej niż v1 → 0.5d, bo A.3 i A.4 to realne gapy a nie kosmetyka)

---

## Faza 10 — Blocking facade (1 dzień)

### 10.1 Feature flag + cfg

W [Cargo.toml](sdks/rust/Cargo.toml) dodać `blocking = []` do `[features]`. Także w `[package.metadata.docs.rs]`: `features = ["blocking", "tracing"]`.

- **Czas:** 0.25h

### 10.2 Runtime singleton + panic guard

`src/blocking/runtime.rs`:

```rust
use std::sync::OnceLock;
use tokio::runtime::{Builder, Runtime};

static RT: OnceLock<Runtime> = OnceLock::new();

pub(super) fn rt() -> &'static Runtime {
    RT.get_or_init(|| {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build blocking runtime")
    })
}

pub(super) fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    if tokio::runtime::Handle::try_current().is_ok() {
        panic!(
            "honcho_ai::blocking called from inside a running Tokio runtime; \
             use the async API (honcho_ai::Honcho) instead"
        );
    }
    rt().block_on(fut)
}
```

- [ ] Test `tests/blocking_runtime_guard.rs`: `#[tokio::test]` + `std::panic::catch_unwind(|| blocking::Honcho::builder()...build().workspaces())`, sprawdzić message.
- **Czas:** 1h (razem z testem)

### 10.3 `blocking::Honcho` wrapper

`src/blocking/client.rs` — wrapper nad `crate::Honcho`. Pattern dla każdej metody:

```rust
pub fn workspaces(&self) -> impl Iterator<Item = Result<String>> + '_ {
    crate::blocking::iter::BlockingIter::new(self.inner.workspaces())
}
pub fn search(&self, query: impl Into<String>) -> Result<Vec<MessageResponse>> {
    super::runtime::block_on(self.inner.search(query))
}
```

Metody do sportowania (cross-check z [src/client.rs](sdks/rust/src/client.rs)):
`builder`, `peer`, `session`, `workspaces`, `search`, `queue_status`, `schedule_dream`, `delete_workspace`, `get_metadata`, `set_metadata`, `get_configuration`, `set_configuration`.

- **Czas:** 1h

### 10.4 Stream → Iterator adapter

`src/blocking/iter.rs`:

```rust
pub struct BlockingIter<S> { stream: S }
impl<S: Stream + Unpin> Iterator for BlockingIter<S> {
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> {
        super::runtime::block_on(self.stream.next())
    }
}
```

- [ ] Test: 3-page pagination → `.collect::<Vec<_>>()` daje 6 itemów; mid-stream `Err` propaguje się jako `Some(Err(..))` a nie panic.
- **Czas:** 0.75h

### 10.5 `blocking::{Peer,Session,Conclusion,ConclusionScope}`

Każdy plik `src/blocking/<module>.rs` to wrapper na odpowiedniku async, według tego samego patternu z 10.3 i 10.4.

**Lista metod do upewnić** (cross-check):
- `Peer`: refresh, chat, chat_with_options, chat_stream, message, search, get_card, set_card, card, representation, context, sessions, conclusions, get_metadata, set_metadata, get_configuration, set_configuration
- `Session`: refresh, add_peers, set_peers, remove_peers, get_peer_configuration, set_peer_configuration, add_messages, messages, clone, context, delete, upload_file, metadata getters/setters
- `ConclusionScope`: list, query, create, delete, representation

**Special case `chat_stream`:** zwraca `ChatStreamBuilder` (lazy). W blockingu: `chat_stream_send() -> impl Iterator<Item = Result<String>>` jako finalizer buildera.

- **Czas:** 2h

### 10.6 Smoke test + lib re-exports

- [ ] `src/lib.rs`: `#[cfg(feature = "blocking")] pub mod blocking;` + `#[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]`
- [ ] `tests/blocking_smoke.rs`: jeden e2e przez wiremock pokrywający Honcho→Peer→Session→chat→stream.
- **Czas:** 1h

**Faza 10: ~6h ≈ 1 dzień**

---

## Faza 11 — Integration + parity (1 dzień)

### 11.1 Harness

`tests/integration/common.rs`:

```rust
pub fn try_client() -> Option<Honcho> {
    let key = std::env::var("HONCHO_API_KEY").ok()?;
    let url = std::env::var("HONCHO_URL").unwrap_or_else(|_| "http://localhost:8000".into());
    Some(Honcho::builder().api_key(key).base_url(url).build().ok()?)
}

pub fn unique_id(prefix: &str) -> String {
    format!("{}-{}", prefix, chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0))
}
```

Pattern testu: `let Some(c) = try_client() else { return; }` — testy `#[tokio::test]` po prostu skipują się gdy brak env. Zero ignore-flag, zero feature flag.

- **Czas:** 1h

### 11.2 Lifecycle e2e

`tests/integration/lifecycle.rs`:
1. `ensure_workspace` — sprawdzić idempotency (drugi call: wiremock-style asercja, że workspace istnieje).
2. 2 peers + session + 10 messages.
3. `chat` → niepuste.
4. `search("...")` → strumień drain.
5. `context(tokens=2000, summary=true)` → shape check.
6. Cleanup: `session.delete()`, `client.delete_workspace(ws)`.

- **Czas:** 1.5h

### 11.3 Streaming e2e

`tests/integration/streaming.rs`: `chat_stream("...")` + drain, porównanie z `chat()` (równe lub semantically equivalent po lowercase strip).

- **Czas:** 0.5h

### 11.4 Upload e2e

`tests/integration/upload.rs`: upload 100KB tekstowego, potem `session.messages()` zawiera referencję.

- **Czas:** 0.5h

### 11.5 Parity scenarios — Python jako source of truth

Bez `xtask`. Skrypt `sdks/rust/scripts/regen_parity.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."  # repo root
uv run python sdks/rust/tests/fixtures/parity/_generate.py
```

`sdks/rust/tests/fixtures/parity/_generate.py` używa Python SDK z `sdks/python` i dumpuje 3 scenariusze do JSON-a:
- `small/`: 1 peer, 3 messages, `to_openai("assistant")`
- `multi_peer/`: 3 peers, mieszane role
- `with_summary/`: session z `summary=true`

`tests/parity.rs` ładuje JSON i porównuje canonical shape (ignore: `id`, `created_at`, `updated_at`).

- [ ] Skrypt
- [ ] Generator Python
- [ ] Test Rust loader + comparator
- **Czas:** 2h

### 11.6 CI integration job

Dodać job do `.github/workflows/rust-sdk.yml`:

```yaml
  integration:
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    services:
      postgres: { image: pgvector/pgvector:pg16, ports: ['5432:5432'], env: { POSTGRES_PASSWORD: pass } }
      redis: { image: redis:7, ports: ['6379:6379'] }
    env:
      HONCHO_API_KEY: ${{ secrets.HONCHO_TEST_KEY }}
      HONCHO_URL: http://localhost:8000
    steps:
      - uses: actions/checkout@v4
      - uses: astral-sh/setup-uv@v3
      - run: uv sync
      - run: uv run alembic upgrade head
      - run: uv run fastapi dev src/main.py &
      - run: sleep 5 && curl -f http://localhost:8000/health
      - uses: dtolnay/rust-toolchain@stable
      - run: cd sdks/rust && cargo test --test 'integration_*'
```

- **DoD:** zielone na merge do main.
- **Czas:** 1.5h

**Faza 11: ~7h ≈ 1 dzień**

---

## Faza 12 — Docs, examples, release (1 dzień)

### 12.1 `missing_docs` final pass

[src/lib.rs:18](sdks/rust/src/lib.rs:18) ma `missing_docs` w deny. Komendy weryfikujące:

```bash
cd sdks/rust && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

Punkty zapalne: moduły bez doc-comment (sprawdzić `pub mod` declarations), nowo dodane blocking module.

- **Czas:** 1h

### 12.2-12.5 Examples

Z `required-features` dla blocking. Każdy ma być standalone, kompilować się na CI, mieć top-comment z run-command.

| Plik | Cel | Czas |
|---|---|---|
| `examples/quickstart.rs` | Honcho → peer → session → chat (15-20 linii) | 0.5h |
| `examples/streaming.rs` | chat_stream + stdout drain | 0.25h |
| `examples/blocking.rs` (`required-features = ["blocking"]`) | sync wersja quickstart | 0.25h |
| `examples/multi_peer.rs` | 3 peers, observation configs | 0.5h |
| `examples/upload.rs` | upload + retrieval z disk | 0.25h |

Istniejący `examples/conclusions.rs` — buduje się ✅, zostaje.

- **Łącznie:** 1.75h

### 12.6 README rewrite

[sdks/rust/README.md](sdks/rust/README.md) — zastąpić "TODO" sekcję:

- Installation: `cargo add honcho-ai` (lub `honcho`, zależnie od A.9)
- Quickstart (≤15 linii — kopia z `examples/quickstart.rs`)
- Features (rustls/native-tls/blocking/tracing) — krótki paragraf
- MSRV: 1.80 (z linkiem do MSRV policy)
- Status badge: `![rust-sdk](https://github.com/plastic-labs/honcho/actions/workflows/rust-sdk.yml/badge.svg)`
- Links: docs.rs, repo, OpenAPI spec

- **Czas:** 0.75h

### 12.7 CHANGELOG.md

`sdks/rust/CHANGELOG.md` w formacie Keep-a-Changelog. Wpis v0.1.0:

- Added (per faza, jednoliniowo): Errors+Types (55 schemas), HTTP+Retry (5 codes, Retry-After parsing), Pagination Stream, Honcho client + workspace, Peer, Session, Message+Upload (multipart streaming), SSE streaming + cancel-safety, Conclusion+ConclusionScope, Blocking facade (feature-gated)
- Known limitations: brak webhooks/keys API, brak auto-reconnect SSE, MSRV 1.80

- **Czas:** 0.5h

### 12.8 Cargo metadata + version bump

[Cargo.toml](sdks/rust/Cargo.toml):

```toml
version = "0.1.0"  # bump z "0.1.0-alpha.0"
keywords = ["honcho", "ai", "memory", "agents"]  # ✅ już ma
categories = ["api-bindings", "asynchronous"]    # ✅ już ma

[package.metadata.docs.rs]
all-features = true
features = ["blocking", "tracing"]
rustdoc-args = ["--cfg", "docsrs"]
targets = ["x86_64-unknown-linux-gnu"]
```

- **Czas:** 0.25h

### 12.9 Public API surface (opcjonalne)

`cargo install cargo-public-api && cargo public-api --simplified > public-api.txt` zatwierdzić jako baseline. Dodaje pre-commit check w v0.2.

- **Czas:** 0.25h (lub skip — polish)

### 12.10 Publish dry-run

```bash
cd sdks/rust
cargo package --list   # audyt zawartości
cargo publish --dry-run
```

Sprawdzić:
- Package <500KB
- `target/`, `tests/fixtures/parity/_generate.py`, `scripts/` excluded (jeśli niechciane) — dodać `exclude = [...]` w `[package]`
- Brak `path = "..."` dep — sdks/rust nie jest workspace ✅

- **Czas:** 0.5h

### 12.11 Tag

Po akceptacji maintainera (poza scope dokumentu):
- `git tag rust-sdk-v0.1.0`
- GitHub Release z CHANGELOG body
- `cargo publish` manualnie

- **Czas:** 0.25h przygotowania

**Faza 12: ~6.25h ≈ 1 dzień**

---

## Harmonogram

| Faza | Czas | Kumul. |
|---|---|---|
| A. Cleanup faz 0-9 | 1.0 | 1.0 |
| 10. Blocking | 1.0 | 2.0 |
| 11. Integration + parity | 1.0 | 3.0 |
| 12. Docs + release | 1.0 | 4.0 |
| Bufor (CI flakes, parity edge cases, decyzje review) | 1.0 | **5.0** |

**Łącznie: 5 dni roboczych** (v1 zaniżało do 4 nie uwzględniając gapów A.3, A.4).

---

## Critical path

```
A.1 + A.8 (CI start) ──┬──→ A.3, A.4 (test gaps) ──→ A.5-A.7 (asercje, debug, non_exhaustive)
                       │
                       └──→ A.9, A.10 (decyzje, license)
                                │
                                ├──→ 10.x (blocking) ──┐
                                │                      │
                                └──→ 11.x (integration)┴──→ 12.x (docs + release)
```

Fazy 10 i 11 idą równolegle — różne pliki, brak dependency. Faza 12 czeka na obie.

---

## Co usunięte z v1 i dlaczego

| Wycięte | Powód |
|---|---|
| `xtask/Cargo.toml` (workspace member) | sdks/rust **nie jest workspace** — dodawanie wymagałoby restrukturyzacji. Zastąpione skryptem bash |
| Audyt `examples/conclusions.rs` | zweryfikowane: `cargo build --examples` zielony |
| MSRV matrix toolchain | `1.80` + stable = redundant; `cargo-msrv verify` wystarczy |
| Granularne 0.1h tasking | nie służy planowaniu, tylko bzdurom |
| Mieszanka `#[ignore]` + feature flag + env var dla integration | wybrane: czysty env var (`HONCHO_API_KEY`) z skip-if-missing pattern |
| Osobne 10.5, 10.6, 10.7 | scalone w jedno 10.5 (ten sam pattern) |

---

## Co dodane w v2 (luki z faz 0-9 niewidoczne w v1)

| Dodane | Skąd luka |
|---|---|
| A.3 SSE cancel-safety test | plan faza 8 wymaga, kod tego nie ma; tylko pagination ma drop test |
| A.4 Python parity fixtures | plan section 17 wymaga "fixtures z Pythona" dla `to_openai`/`to_anthropic`; obecne testy ręczne |
| A.5 Rozszerzenie compile_assertions | tylko 3 asercje, brakuje Honcho/Peer/Session/Message/HonchoError |
| A.6 Debug derive (Peer, DialecticStream) | Rust API Guidelines C-DEBUG; brak na tych typach |
| A.7 `#[non_exhaustive]` audit | plan niezmiennik 18.6 wymaga, brak weryfikacji |
| A.9 Crate name decision | niespójność plan v2 (`honcho`) vs Cargo.toml (`honcho-ai`) |
| A.10 LICENSE-APACHE | README się odwołuje, plik nie istnieje |

---

## Open questions (do decyzji przed startem)

1. **Crate name na crates.io** — `honcho-ai` czy `honcho`? (A.9)
2. **`tracing` rozszerzyć w v0.1?** — obecnie 1 callsite; wewnętrznie minimum byłoby `span` per request w HttpClient. Może iść do v0.2.
3. **Integration CI: serwer in-CI czy staging endpoint?** — Plan zakładał własny server uruchamiany w job. Alternatywa: stały staging endpoint + `HONCHO_TEST_KEY` jako secret. In-CI = bardziej hermetyczne, staging = szybsze.
4. **MSRV downward?** — Kod nie używa `LazyLock` ani 1.80-only API; `OnceLock` jest od 1.70. Można obniżyć MSRV do 1.75 (async trait stabilization). Zysk: szerszy user base.
5. **`cargo public-api` baseline lockdown?** — Dobry dla stabilności v0.1.x, ale dodaje pre-commit complexity. Decyzja maintainera.
6. **Webhooks API w v0.1?** — Plan jawnie out-of-scope. Confirm że to nie zmieniło się od czasu pisania planu.

---

## Niezmienniki przez całą drogę

1. **Red przed green** — żaden commit z implementacją bez testu (commit `f19f34e Change folder` to nie commit z planu, ale zostawił regression — A.1 to lekcja: testy muszą działać niezależnie od layoutu repo).
2. **Brak `unwrap`/`expect` w `src/`** — clippy deny w [src/lib.rs:13](sdks/rust/src/lib.rs:13) ✅; nowo dodany kod (blocking, parity) musi trzymać.
3. **Każdy nowy pub item ma `///`** — compiler enforce przez `missing_docs`.
4. **`#[non_exhaustive]` na nowych publicznych enum/struct** (A.7 sprawdza obecne).
5. **`Send + Sync + 'static` na publicznych Future/Stream return** (A.5 dodaje asercje).
6. **CI zielone przed mergem** — `cargo fmt`, `clippy -D warnings`, `test`, `doc -D warnings`, `msrv verify` (A.8).
