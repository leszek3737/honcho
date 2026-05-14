# Honcho Rust SDK — Roadmap do v0.2.0 (v2)

**Źródła**: `sdks/SUMMARY.md` (audyt 2026-05-13), `sdks/rust/CHANGELOG.md`, weryfikacja kodu w `sdks/rust/src/` i `sdks/rust/tests/`.
**Cel**: zamknąć parytet z Python SDK v2.1.1 w zakresie correctness + wire-level, wydać v0.2.0.
**Zakres**: **wyłącznie Rust SDK**. Bugi Pythona przeniesione do `PYTHON_SDK_FIXES.md` (osobny dokument).

---

## 0. Filozofia tej wersji

1. **Trust-but-verify**: każde HIGH/MEDIUM z SUMMARY weryfikujemy w kodzie przed implementacją. CHANGELOG Rust v0.2.0 deklaruje rzeczy, których nie ma — nie ufamy mu.
2. **Atomic PR**: jedno zadanie = jeden PR ≤ 400 linii diff. Wyjątek: zmiana sygnatury `next_page` (cross-cutting).
3. **Wire-parity fixtures**: każda zmiana ścieżki HTTP/body weryfikowana przez `tests/fixtures/parity/` (infra już istnieje).
4. **Single source of truth**: CHANGELOG aktualizowany w tym samym PR co kod. `[Unreleased]` zawsze odzwierciedla `main`.

---

## 1. Stan aktualny (weryfikacja w kodzie)

| Element | Status w `main` | CHANGELOG mówi | Prawda |
|---------|----------------|----------------|--------|
| `Cargo.toml` version | `0.1.1` | `0.2.0 - 2025-05-13` | rozjazd, nigdy nie wydane |
| `Page::next_page` | `Option<Self>` (`pagination.rs:148`) | `Result<Option<Page>>` | **CHANGELOG kłamie** |
| `collect_all_pages` | `Vec<T>` (`blocking/iter.rs:34`) | `Result<Vec<T>>` | **CHANGELOG kłamie** |
| `DialecticStream::final_response` | `&str` (`dialectic_stream.rs:45`) | `FinalResponse` struct | **CHANGELOG kłamie** |
| `ChatStreamBuilder::send` | wraca raw stream | (impl) `DialecticStream` | wrap brakuje |
| `Peer::set_configuration` | `HashMap<String, Value>` (`peer.rs:255`) | typed | nadal untyped |
| `examples/` | 12 plików | — | **OK, brak luki** |
| `to_openai`/`to_anthropic` peer_card | `serde_json::to_string` (Rust OK) | — | bug po stronie Pythona |
| `tests/fixtures/parity/` | istnieje, używany w `session_context_parity.rs` | — | używamy do nowych testów |
| `blocking::Session::upload_file` | brak | — | luka potwierdzona |
| `Honcho::workspaces` | bez `filters` (`client.rs:619`) | — | luka potwierdzona |

**Wniosek**: 4 z 7 HIGH z SUMMARY rzeczywiście HIGH. Kluczowa praca: pagination errors + DialecticStream wrap + typed PeerConfig + session.representation filters + blocking upload + wire schema fixes.

---

## 2. Non-goals (v0.2.0)

Te rzeczy **nie** wchodzą do v0.2.0 — żeby ograniczyć scope:

- Webhooks (M14) → v0.3.0
- `create_scoped_key()` JWT utility (L44) → v0.3.0
- API key rotation (M27) → v0.3.0 (wymaga refaktoru `HttpClient`)
- Konsolidacja split methods `peers()`/`peers_with_filters()` (M17) — celowy kompromis Rust idiomatic, dokumentujemy
- Builder vs kwargs (L62), getters vs attrs (L63) — naming difference, nie bugi
- SSE reconnection (L40)
- Per-request header override (L32) → v0.3.0

---

## 3. Risk register

| ID | Ryzyko | Mitygacja |
|----|--------|-----------|
| R1 | Breaking change w `next_page`/`collect_all_pages` zerwie istniejących użytkowników 0.1.1 | Wersja semver-major (0.2.0). MIGRATION.md sekcja "Pagination errors". Code mod snippet w docs. |
| R2 | Typed `PeerConfig` z `deny_unknown_fields` odrzuca forward-compat klucze które serwer akceptuje | Dodajemy `set_configuration_raw` jako escape hatch. Decyzja udokumentowana. |
| R3 | Zmiana wrapowania `ChatStreamBuilder::send()` w `DialecticStream` zmienia typ zwracany — breaking | Świadomie breaking, semver-major. Doc snippet. |
| R4 | Parity test fixtures rozjadą się z serwerem | `tests/fixtures/parity/_generate.py` regeneruje. CI gate na regen w PR co tyka kontrakt. |
| R5 | Blocking `upload_file` z streamem może deadlockować runtime | Test integracyjny z dużym plikiem (>1MB) na CI. Użyj `tokio::task::spawn_blocking` jeśli sync `Read` blokuje. |
| R6 | MSRV bump (edition 2024, rust 1.88) może wykluczyć użytkowników | Nie ruszamy MSRV w 0.2.0. Reaffirm w `Cargo.toml`. |

---

## 4. Faza 0 — Verification & setup (0.5 dnia)

Przed kodowaniem, weryfikacja stanu i przygotowanie scaffoldu.

| ID | Zadanie | Plik(i) | AC |
|----|---------|---------|-----|
| V0.1 | Verify HIGH findings against current code (powyższa tabela rozszerzona) | — | Dokument: każdy H1–H7 oznaczony VERIFIED / FALSE / PARTIAL |
| V0.2 | Pełen `cargo test --all-features` + `cargo test --features blocking` baseline. Zapisz failujące testy. | — | Lista znanych failów (jeśli są) w PR `chore/baseline` |
| V0.3 | Audyt CHANGELOG: każdy bullet z `[0.2.0]` oznacz `[DONE]`/`[TODO]`/`[FALSE]`. Przenieś niezaimplementowane do `[Unreleased]`. Bump `Cargo.toml` → `0.2.0-alpha.1`. | `CHANGELOG.md`, `Cargo.toml` | CHANGELOG zgodny z faktycznym kodem |
| V0.4 | Dodaj sekcję `## Pagination errors (0.2.0)` w `MIGRATION.md` z code mod snippetami | `MIGRATION.md` | Gotowy szablon do uzupełnienia po implementacji |
| V0.5 | Dodaj `tests/fixtures/parity/peer_card_repr/` jeśli nie istnieje (na potrzeby T2.1) | `tests/fixtures/parity/` | Fixture katalog gotowy |

---

## 5. Faza 1 — Correctness (P0): pagination errors

**Cel**: zatrzymać silent data loss. Jedno spójne PR pakietu (cross-cutting).

### Plan PR

PR `fix/pagination-propagate-errors` zawiera wszystkie poniższe zmiany w jednym diff, bo refactor sygnatury rozsiewa się po 8 plikach.

| ID | Zadanie | Plik:linia | Szczegół | AC |
|----|---------|------------|----------|-----|
| P1.1 | `Page::next_page` → `Result<Option<Self>>` | `types/pagination.rs:148` | Usuń `.ok()?` (linia 157). Zwróć `?` przy błędzie, `Ok(None)` gdy brak `next_fetcher` lub `!has_next()`, `Ok(Some(...))` przy sukcesie. | Test wiremock: HTTP 500 na stronie 2 → `Err`, nie `Ok(None)`. |
| P1.2 | `collect_all_pages` → `Result<Vec<T>>` | `blocking/iter.rs:34` | Propaguj `?` przez pętlę `while let Some(next) = current.next_page().await?`. | Test: błąd na 2. stronie → `Err`, nie obcięty `Vec`. |
| P1.3 | `Page::into_stream` items → `Result<T>` | `types/pagination.rs` (oba miejsca z `into_stream`) | Stream emituje `Result<T>` per item. Błąd paginacji = `Some(Err(...))`. | Test: stream wypluwa Err po 500. |
| P1.4 | Update callers blocking | `blocking/client.rs:136,153,161,178`; `blocking/peer.rs:129`; `blocking/session.rs:119` | Usuń podwójne `Ok(...)`. Sygnatury blocking nadal `Result<Vec<T>>`, ale teraz propagują real error. | `cargo build --features blocking` zielony. |
| P1.5 | Update caller `blocking/client.rs:190` (BlockingIter loop) | `blocking/client.rs:190`, `blocking/iter.rs:41` | `while let Some(next) = current.next_page().await?` z `?` rozprasowanym do błędu iteratora. | Iter zwraca `Err` w `next()` zamiast `None` po 500. |
| P1.6 | MIGRATION.md uzupełnij | `MIGRATION.md` | Sekcja "Pagination errors" z before/after kodem. | Doc widoczny w 0.2.0 release notes. |
| P1.7 | CHANGELOG: R-07/R-08 do `[0.2.0-alpha.1]` jako Breaking | `CHANGELOG.md` | Wpisy zgodne z faktem. | Diff CHANGELOG w tym PR. |
| P1.8 | Test parity: snapshot błędu paginacji | `tests/pagination.rs` (rozszerz) | Mock 500 na stronie 2, weryfikuj typ zwracany. | Test zielony. |

**Wykluczone z tego PR**: zmiana logiki retry (osobny ticket), zmiana paginacji w innych endpoint-ach.

---

## 6. Faza 2 — Wire schema parity (P0/P1)

Każde zadanie = osobny PR ≤ 400 LOC.

### T2.1 — `Session::representation` pełne parametry (H5)

| | |
|--|--|
| **PR** | `feat/session-representation-builder` |
| **Plik** | `src/session.rs:1095`, `src/types/session.rs` (dopisać options) |
| **Akcja** | `Session::representation_builder() -> SessionRepresentationBuilder` z `bon::Builder` polami: `target`, `search_query`, `search_top_k`, `search_max_distance`, `include_most_frequent`, `max_conclusions`. Stara `representation(peer_id)` deleguje do buildera z defaultami. |
| **AC** | Wire body request identyczne z Pythonem dla każdej kombinacji (test parity z fixture). `cargo doc` pokazuje builder. |
| **Wire test** | `tests/session_representation_parity.rs` z fixture `tests/fixtures/parity/session_representation/`. |

### T2.2 — Typed `PeerConfig` (H6, M8)

| | |
|--|--|
| **PR** | `feat/typed-peer-config` |
| **Plik** | `src/types/peer.rs` (typ + serde), `src/peer.rs:255-271` (sygnatura), `src/peer.rs` PeerInner cache |
| **Akcja** | `pub struct PeerConfig { pub observe_me: Option<bool> }` z `#[serde(deny_unknown_fields)]`. Migracja `set_configuration(PeerConfig)` / `get_configuration() -> PeerConfig`. Cache `RwLock<Option<PeerConfig>>`. Dodaj `set_configuration_raw(HashMap)` + `get_configuration_raw() -> HashMap` jako escape hatch (parytet z `Honcho::*_raw`). |
| **AC** | `deny_unknown_fields` weryfikowany testem. Raw API działa. CHANGELOG: Breaking. |

### T2.3 — `DialecticStream` wrapping (H4, R-27)

| | |
|--|--|
| **PR** | `feat/dialectic-stream-wrap` |
| **Pliki** | `src/peer.rs:939` (`ChatStreamBuilder::send`), `src/dialectic_stream.rs` (typ `FinalResponse`) |
| **Akcja** | (a) `pub struct FinalResponse { pub content: String }` (b) `DialecticStream::final_response(&self) -> &FinalResponse` (refactor accumulated → FinalResponse wewn.) (c) `ChatStreamBuilder::send()` zwraca `Result<DialecticStream<impl Stream<Item=Result<String>>>>` zamiast raw `Pin<Box<...>>`. |
| **AC** | Test E2E SSE: można iterować + odczytać `.final_response().content` + `is_complete()`. Breaking — semver dokumentowany. |

### T2.4 — Create-with-metadata (M1, M2)

| | |
|--|--|
| **PR** | `feat/create-with-metadata` |
| **Pliki** | `src/client.rs:322,344`, `src/types/peer.rs` (PeerCreate), `src/types/session.rs` (SessionCreate) |
| **Akcja** | `Honcho::peer_builder(id).metadata(...).configuration(...).get_or_create()`. Analogicznie `session_builder()` + `peers: Vec<PeerSpec>`. Wykorzystaj istniejące `PeerCreate`/`SessionCreate` (część z 17 nieużywanych structów — M31 częściowo rozwiązany przy okazji). Zachowaj `Honcho::peer(id)` jako convenience. |
| **AC** | Jedna runda HTTP zamiast `peer()` + `set_metadata()`. Wire body identyczny z Python `client.peer(id, metadata=..., configuration=...)`. |

### T2.5 — `session.messages_with_options` (M3)

| | |
|--|--|
| **PR** | `feat/session-messages-filter` |
| **Plik** | `src/session.rs:722` |
| **Akcja** | Builder z `filter: Option<HashMap>`, `page`, `size`, `reverse`. Mirror `session.search_with_options` schemy. |
| **AC** | Wire test: body request match z Python `session.get_messages(filter=...)`. |

### T2.6 — Blocking `upload_file` + `upload_file_streamed` (H7)

| | |
|--|--|
| **PR** | `feat/blocking-upload-file` |
| **Plik** | `src/blocking/session.rs` |
| **Akcja** | Dodaj `pub fn upload_file(&self) -> BlockingUploadFileBuilder<'_>`. Builder owija async, `.send()` woła `block_on()`. Streamed wariant przyjmuje `impl Read + Send` zamiast `AsyncRead`. Wewn. konwersja przez `tokio_util::io::ReaderStream`. |
| **AC** | Test integracyjny w `tests/blocking_upload.rs`: upload >1MB pliku przeciw wiremock matcherowi multipart. Brak deadlocku przy dużych plikach (R5). |

### T2.7 — Drobne wire fixes (M4, M5, M6, L1)

PR `chore/wire-schema-fixes` — zbiorczy:

| ID | Plik:linia | Fix |
|----|------------|-----|
| T2.7.a | `types/message.rs:86` | Usuń `skip_serializing_if` z `filters` w `MessageSearchOptions` żeby wysyłać `null` jak Python |
| T2.7.b | `types/peer.rs:147` | `max_conclusions` default `Some(25)` zamiast `None` |
| T2.7.c | `types/workspace.rs:107` | Dokumentacja double-Option na `WorkspaceUpdate.metadata` + helper `clear_metadata()` |
| T2.7.d | `client.rs:619` | `Honcho::workspaces_with_filters(filters, ...)` analogicznie do `peers_with_filters` |

**AC**: snapshot wire bodies przeciw Python output (jeden test per fix).

---

## 7. Faza 3 — Type safety & dedup (P1)

### T3.1 — Typed `SessionConfiguration` + `WorkspaceConfiguration` (M8)

| | |
|--|--|
| **PR** | `feat/typed-session-workspace-config` |
| **Pliki** | `src/types/session.rs`, `src/types/workspace.rs`, `src/session.rs`, `src/client.rs` |
| **Akcja** | Typy z polami zgodnymi z Pydantic w `sdks/python/src/honcho/api_types.py`. Migracja `get/set_configuration` w `Session` i `Honcho`. Raw warianty zachowane. |
| **AC** | HashMap zniknął z public API tych metod. Raw API działa. Wire body match. |

### T3.2 — Dedup `SummaryConfiguration` + 3 inne (M9, M10)

| | |
|--|--|
| **PR** | `refactor/dedupe-config-types` |
| **Pliki** | `src/types/common.rs` (nowe definicje), re-export z `session.rs`, `workspace.rs`, `peer.rs`, `dream.rs` |
| **Akcja** | Jedna definicja per typ. Wybór: `u32` dla `SummaryConfiguration.max_tokens` (Rust dotąd: jeden plik `u32`, drugi `u64`). Re-export `pub use crate::types::common::SummaryConfiguration`. |
| **AC** | `rg "struct SummaryConfiguration"` w `src/` → 1 hit. |

### T3.3 — Migracja na typed request structs (M31, częściowo)

| | |
|--|--|
| **PR** | `refactor/use-typed-request-structs` |
| **Akcja** | Audyt 17 structów (`PeerCreate`, `SessionCreate`, `MessageBatchCreate`, etc.). Dla każdego: użyj w `client.rs`/`peer.rs`/`session.rs` zamiast `serde_json::json!()`, albo usuń. Decyzja per struct w PR description. |
| **Zakres** | Niech 50% wystarczy w tym PR — niskie ryzyko, czysto kosmetyczne. Reszta w follow-up. |
| **AC** | `cargo clippy -- -W unused` zielony. |

### T3.4 — Cross-field validation `SessionContextOptions` (M11)

| | |
|--|--|
| **PR** | `feat/session-context-options-validation` |
| **Plik** | `src/types/session.rs:142` |
| **Akcja** | Walidator w `Builder::build()`: `peer_perspective` wymaga `peer_target`. Zwraca `ValidationError`. |
| **AC** | Test: invalid combo → `Err(ValidationError)`. |

---

## 8. Faza 4 — Ergonomia & dead code (P2)

### T4.1 — `Peer::context` konsolidacja (M20)

| | |
|--|--|
| **PR** | `refactor/peer-context-builder` |
| **Plik** | `src/peer.rs:497-566` |
| **Akcja** | `Peer::context_builder().target(...).summary(true).get()`. Deprecate `context_with_target`/`context_with_options` z `#[deprecated]` od 0.2.0, usunięcie w 0.3.0. |
| **AC** | Stare API nadal działa z warningiem. Nowy builder w doctest. |

### T4.2 — `HonchoParams` passthrough (M15)

| | |
|--|--|
| **PR** | `feat/honcho-params-passthrough` |
| **Pliki** | `src/client.rs:58-73`, `src/http/client.rs:39-51` |
| **Akcja** | `HonchoParams` dostaje `timeout`, `max_retries`, `default_headers`, `default_query`. Przekazane do `HttpClientParams`. |
| **AC** | Można skonfigurować retries i timeout z `Honcho::builder()`. |

### T4.3 — Dead code cleanup

PR `chore/dead-code-cleanup`:

| ID | Plik | Akcja |
|----|------|-------|
| T4.3.a | `src/types/common.rs` (Metadata alias) | usuń (L51) |
| T4.3.b | `src/peer.rs` (`Peer::card`) | usuń, deprecated od 0.1.0 (L70) |
| T4.3.c | `src/types/message.rs`, `src/types/conclusion.rs` (`from_response`) | usuń `#[allow(dead_code)]` konstruktory (L67) |
| T4.3.d | `src/types/pagination.rs:374` (`paginate_post`) | `pub(crate)` (L71) |
| T4.3.e | `src/types/peer.rs`, `src/types/conclusion.rs` (`MessageInner`/`ConclusionInner.http`) | usuń jeśli nieużywane (L68) |
| T4.3.f | `src/types/peer.rs` (PeersPageResponse.total/pages) | dodaj akcesory lub usuń (L69) |

**AC**: `cargo build --all-features` bez warningów, brak `#[allow(dead_code)]`.

### T4.4 — Multipart Content-Type fix (M24)

| | |
|--|--|
| **PR** | `fix/multipart-content-type` |
| **Plik** | `src/http/client.rs:338` |
| **Akcja** | Przed wysłaniem multipart, usuń `Content-Type` z `default_headers` jeśli ustawiony (jak Python `py:http/client.py:285`). |
| **AC** | Test wiremock: dokładnie jeden `Content-Type` header. |

### T4.5 — Empty body handling (M29)

| | |
|--|--|
| **PR** | `fix/empty-body-handling` |
| **Plik** | `src/http/client.rs:255-263` |
| **Akcja** | Dla endpointów z typowanym response, 200 + empty body → `Ok(None)` (jeśli `Option<T>`) zamiast decode error. Match z Python `py:http/client.py:107-109`. |
| **AC** | Test parity. |

### T4.6 — Naming aliases (L25, L26, L14)

PR `chore/naming-aliases`:

| ID | Plik | Akcja |
|----|------|-------|
| T4.6.a | `src/types/pagination.rs` | Dodaj `has_next_page() = has_next()` jako alias (parytet z Python `has_next_page()`) |
| T4.6.b | `src/types/pagination.rs` | Dodaj `get_next_page()` jako alias `next_page()` |
| T4.6.c | `src/types/session.rs` | `SessionContext::session_id() -> &str` jako alias `id()` |

**Decyzja**: aliasy, nie rename. Idiomatic Rust nazwa zachowana jako kanoniczna.

### T4.7 — Stream wrappery export (L52)

| | |
|--|--|
| **PR** | `feat/export-stream-wrappers` |
| **Pliki** | `src/lib.rs`, `src/dialectic_stream.rs` |
| **Akcja** | `pub use` dla `DialecticStreamDelta`, `DialecticStreamChunk` jeśli wewn. istnieją (lub utwórz). Pythonowy parytet. |
| **AC**| `cargo doc` pokazuje typy. |

### T4.8 — `to_openai`/`to_anthropic` przyjmuje `&Peer` (L18)

| | |
|--|--|
| **PR** | `feat/context-accepts-peer-ref` |
| **Plik** | `src/types/session.rs:285,335` |
| **Akcja** | Trait `IntoAssistantRef` zaimplementowany dla `&str`, `String`, `&Peer`. Sygnatura `to_openai(impl IntoAssistantRef)`. |
| **AC** | Można wywołać `ctx.to_openai(&peer)`. |

---

## 9. Sekwencja PR-ów i merge order

```
[V0.1..V0.5] verification
     │
     ▼
[P1.*] fix/pagination-propagate-errors  ◀── BREAKING, idzie pierwsze
     │
     ├──▶ [T2.2] feat/typed-peer-config       ◀── BREAKING
     ├──▶ [T2.3] feat/dialectic-stream-wrap   ◀── BREAKING
     ├──▶ [T2.1] feat/session-representation-builder
     ├──▶ [T2.4] feat/create-with-metadata
     ├──▶ [T2.5] feat/session-messages-filter
     ├──▶ [T2.6] feat/blocking-upload-file
     └──▶ [T2.7] chore/wire-schema-fixes
     │
     ▼ release candidate 0.2.0-rc.1
[T3.*] type safety, dedup
     │
     ▼ release candidate 0.2.0-rc.2
[T4.*] ergonomia, dead code (opcjonalne dla 0.2.0)
     │
     ▼ 0.2.0 final
```

**Wszystkie breaking changes wpadają przed pierwszym RC**. T3/T4 mogą poślizgnąć się na 0.2.1 jeśli czasu brak.

---

## 10. Definition of Done — per PR

Twardy checklist:

- [ ] Kod zgodny z AC zadania.
- [ ] Minimum 1 unit/wiremock test (lub parity fixture jeśli wire-touching).
- [ ] Doctest dla nowego public API.
- [ ] `cargo clippy --all-features --all-targets -- -D warnings` ✅
- [ ] `cargo fmt --check` ✅
- [ ] `cargo test --all-features` ✅
- [ ] `cargo test --features blocking` ✅
- [ ] `cargo build --no-default-features --features rustls-tls` ✅ (sanity)
- [ ] `cargo doc --no-deps --all-features` bez warnów ✅
- [ ] Wpis w `CHANGELOG.md [Unreleased]` z ID zadania
- [ ] Breaking changes: wpis w `MIGRATION.md` z code mod
- [ ] PR description: link do zadania w tym roadmapie

---

## 11. Release checklist v0.2.0

- [ ] Faza 0–2 ukończona w 100% (wszystkie P0/P1)
- [ ] T3.1, T3.2 ukończone (type safety)
- [ ] T4.3 (dead code) ukończone — minimum
- [ ] `cargo publish --dry-run` ✅
- [ ] CHANGELOG: `[Unreleased]` → `[0.2.0] - YYYY-MM-DD`
- [ ] `Cargo.toml` version `0.2.0`
- [ ] MIGRATION.md kompletny
- [ ] README.md odnowiony jeśli API się zmieniło (sprawdź doctesty)
- [ ] CI green na main
- [ ] Tag git `v0.2.0`, push
- [ ] `cargo publish`
- [ ] GitHub release notes (kopia CHANGELOG `[0.2.0]`)

---

## 12. Backlog → 0.3.0

| ID | Zadanie | Źródło |
|----|---------|--------|
| B1 | Webhooks CRUD (4 routes + types + scope) | M14 |
| B2 | `create_scoped_key()` client-side JWT helper | L44 |
| B3 | API key rotation w runtime (`RwLock<String>`) | M27 |
| B4 | Per-request `timeout` i `headers` override | M23, L32 |
| B5 | Refresh metadata+config jedną metodą `Peer::refresh()` | L2 |
| B6 | Examples parity audit (Python ma 7, Rust 12 — może brakować pokrycia funkcjonalnego) | L19 (re-scope) |
| B7 | URL encoding path params konsystencja Rust↔Python | L37 |
| B8 | Error code naming align: `"client_error"` → `"api_error"` (lub odwrotnie po decyzji wspólnej) | L46 |
| B9 | Transport retry rozszerzony o `NetworkError`/`RemoteProtocolError` equivalents | L42 |
| B10 | Konsolidacja `peers()` + `peers_with_filters()` w jedną metodę z Optional | M17 |
| B11 | `Honcho::builder().build()` jednoetapowo zwraca `Result<Honcho>` | M16 |
| B12 | Base URL resolution order — dopasuj do Pythona (environment fallback) | M30 |
| B13 | `Peer::refresh()` jako shortcut na `get_metadata()+get_configuration()` | L2 |

---

## 13. Co świadomie zostawiamy nietknięte

Z PASS sekcji SUMMARY — nie ruszać:

- Async safety (`select!`, lock-across-await, drop semantics) — wszystko PASS.
- Blocking runtime guard, `OnceLock` runtime.
- Wire-level route paths (32 ścieżki identyczne).
- HTTP status code mapping (1:1).
- Auth format `Bearer {key}`.
- Retry status codes `{429, 500, 502, 503, 504}`.
- Multipart wire format.
- SSE parsing semantics.
- `serde_path_to_error` integracja (Rust lepszy niż Python).
- `#![deny(missing_docs)]` enforcement.
- ~112 inline doctests.
- `#[non_exhaustive]` na message types — opcjonalne polish, nie blocker.

---

## 14. Co poszło do osobnego dokumentu

`PYTHON_SDK_FIXES.md` (do utworzenia osobno):

- Python `peer_card` f-string → `json.dumps(card)` (H3 strona Python)
- Python backoff cap `MAX_RETRY_DELAY = 30.0` (M22)
- Python `FinalResponse` TypedDict zamiast `dict[str, str]` (M28)
- Python SSE unit tests port z Rust (M26)
- Python `Peer.update()` PATCH method (M12)
- Python `extra="forbid"` audit pod kątem forward-compat (M7)
- Python missing `User-Agent` header (L4, L36)

---

## 15. Szacunki — z podstawą

**Założenia**: jeden developer, znajomość kodu, peer review przyjmowany w 1 dniu.

| PR | LOC est. | Czas (dni) | Uzasadnienie |
|----|----------|-----------|--------------|
| V0.* | <100 | 0.5 | Pure docs + version bump |
| P1.* (pagination) | ~250 | 1.5 | 8 plików, kompleksowy refactor, test coverage |
| T2.1 (representation) | ~200 | 0.5 | Builder + parity test |
| T2.2 (PeerConfig) | ~250 | 1.0 | Cache type change, migration, escape hatch |
| T2.3 (DialecticStream) | ~150 | 0.5 | Wrap + FinalResponse struct |
| T2.4 (create-with-metadata) | ~300 | 1.0 | Touch client.rs + 2 builder structs |
| T2.5 (messages_with_options) | ~150 | 0.5 | Builder |
| T2.6 (blocking upload) | ~250 | 1.0 | Async→sync bridge, integration test |
| T2.7 (wire fixes) | ~150 | 0.5 | 4 mikro zmiany |
| T3.1 (typed config) | ~400 | 1.5 | Dwa nowe typy, 4 metody migracji |
| T3.2 (dedup) | ~150 | 0.5 | Move + re-export |
| T3.3 (typed requests) | ~300 | 1.0 | Audyt + migracja 17 structów |
| T3.4 (validation) | ~80 | 0.3 | Walidator + test |
| T4.1–T4.8 | ~600 | 2.0 | Polish, w trakcie RC |
| Release engineering | — | 0.5 | Tag, publish, notes |

**Suma P0+P1 (do RC.1)**: ~6.5 dnia
**Suma do v0.2.0 final (z T3, T4)**: ~12 dni
**Buffer 30%** (review iteracje, regresje): **~16 dni roboczych ≈ 3 tygodnie kalendarzowe**

---

## 16. Metryki sukcesu (po wydaniu)

- HIGH findings z SUMMARY zaadresowane: **7/7**
- MEDIUM findings: **co najmniej 20/36** (P1 + część P2)
- LOW findings: oportunistycznie (cleanup PR)
- Wire parity tests: minimum **10 nowych fixture cases**
- Breaking changes udokumentowane w MIGRATION.md: **100%**
- `cargo bench` (jeśli istnieje): brak regresji >5%
- Issue count na repo po 2 tyg od release: tracking baseline

---

*Wygenerowano na bazie weryfikacji kodu w `sdks/rust/src/` przeciw `sdks/SUMMARY.md`. Aktualizować z każdym mergem.*
