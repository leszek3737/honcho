# Honcho Rust SDK — TDD Implementation Plan (v2)

Plan portu Python SDK (`sdks/python`) → Rust SDK (`sdks/rust`). Każda faza: **red → green → refactor**.

> **Zmiany vs v1**: poprawiona niekompatybilność `reqwest-eventsource` / `reqwest-middleware`, wykorzystanie istniejącego `docs/v3/openapi.json`, poprawione stałe retry, format SSE, lista env varów i ctor params. Pełna lista zmian — sekcja "Changelog vs v1" na końcu.

---

## 0. Co już potwierdzone (fakty z kodu)

Źródła: [http/client.py](sdks/python/src/honcho/http/client.py), [client.py](sdks/python/src/honcho/client.py), [utils/sse.py](sdks/python/src/honcho/utils/sse.py), [`docs/v3/openapi.json`](docs/v3/openapi.json).

| Fakt | Wartość |
|---|---|
| OpenAPI spec | `docs/v3/openapi.json` — 45 operacji, 36 ścieżek, 55 schematów |
| Python SDK LOC | 7 656 (24 pliki) |
| Async core | [aio.py](sdks/python/src/honcho/aio.py) — 1 562 linie |
| Env vary | `HONCHO_API_KEY`, `HONCHO_URL`, `HONCHO_WORKSPACE_ID` |
| Domyślny workspace | `"default"` |
| Środowiska | `local = http://localhost:8000`, `production = https://api.honcho.dev` |
| Retry codes | `{429, 500, 502, 503, 504}` |
| Backoff | `0.5s × 2^attempt`, override przez `Retry-After` |
| SSE done | `{"done": true}` w JSON (NIE `[DONE]` sentinel) |
| `_ensure_workspace` | jeden `POST /v3/workspaces {id}` per instancja klienta |

---

## 1. Design decisions (locked in)

### 1.1 Stack — z weryfikacją kompatybilności wersji

| Crate | Wersja | Uwagi |
|---|---|---|
| `reqwest` | **0.12** | downgrade z 0.13 — patrz wybór SSE |
| `serde` + `serde_json` | 1.x | + `serde_path_to_error` dla lepszych błędów dekodowania |
| `thiserror` | 2.0 | |
| `tokio` | 1.x | features: `rt-multi-thread`, `macros` |
| `async-stream` + `futures-util` | latest | paginacja jako `Stream` |
| `bon` | 3.9 | builder (zamiast `typed-builder` — ergonomia, named args) |
| `chrono` | 0.4 | feature `serde`; `DateTime<Utc>` z `rfc3339` |
| `bytes` | 1.x | dla SSE i upload |
| `tokio-util` | 0.7 | `ReaderStream` dla multipart |

**Dev:**

| Crate | Wersja | Cel |
|---|---|---|
| `wiremock` | 0.6 | mock HTTP per test |
| `pretty_assertions` | 1.x | diff dla `assert_eq!` |
| `rstest` | 0.x | parametryzowane testy |
| `static_assertions` | 1.x | `assert_impl_all!(Honcho: Send, Sync, Clone)` |
| `serde_json` | (dev) | porównanie JSON jako `Value` |

**Świadomie odrzucone:**

- **`reqwest-middleware 0.5` + `reqwest-retry 0.9`** — wymagają reqwest 0.13. Konflikt z `reqwest-eventsource 0.6` (wymaga reqwest 0.12). Ręczna pętla retry to ~50 linii i daje pełną kontrolę nad parsem `Retry-After`. Reqwest-retry zyskujemy w v0.2 gdy ekosystem dogoni 0.13.
- **`reqwest-eventsource`** — wystarczy `Response::bytes_stream()` + własny parser. Python SDK i tak robi tak samo ([utils/sse.py](sdks/python/src/honcho/utils/sse.py) — 228 linii).
- **`progenitor` (codegen z OpenAPI)** — kuszące przy 55 schematach, ale: (1) generuje typy bez idiomatycznych nazw, (2) bogate API klienta i tak piszemy ręcznie, (3) byłoby trudniejsze do utrzymania niż 55 ręcznych `#[derive(Deserialize)]`. **Używamy spec do walidacji w testach, nie do generacji.**
- **`async-trait`** — od Rust 1.75 async fn w traitach jest stabilne. Trait `MetadataConfig` nie musi być object-safe.

### 1.2 Concurrency & ownership

- `Honcho`, `Peer`, `Session`, `ConclusionScope` są **`Clone`** — wewnątrz `Arc<Inner>`.
- Cached metadata/config na `Peer`/`Session`: **`Arc<RwLock<Option<...>>>`**. Zapis tylko w `refresh()` / `set_*`. Czytelnicy zwykle dostają sklonowane wartości.
- Wszystkie publiczne `Future` i `Stream` mają bounds `+ Send + 'static` — pozwala na `tokio::spawn` po stronie usera.
- `#[non_exhaustive]` na `HonchoError` i wszystkich `*Response` strukturach (zgodne z polityką ewolucji API Honcho).

### 1.3 API surface

**Portujemy 1:1** to co jest publiczne w [\_\_init\_\_.py](sdks/python/src/honcho/__init__.py):

- `Honcho` + builder
- `Peer`, `Session`, `Message`, `Conclusion`, `ConclusionScope`
- `SessionContext` z `to_openai()` / `to_anthropic()`
- Wszystkie publiczne typy `*Response`, `*CreateParams`, `*UpdateParams`, `*ListParams`
- Hierarchia błędów wg [http/exceptions.py](sdks/python/src/honcho/http/exceptions.py)
- Paginacja (jako `Stream` w async, `Iterator` w blocking feature)
- Streaming chat (SSE)
- Upload pliku (multipart)

**Świadomie poza scope v0.1:**

- Webhooks API (`/v3/workspaces/{ws}/webhooks/*`) — Python SDK ich nie eksponuje przez object model.
- Keys API (`/v3/keys`) — endpoint server-side.
- Wszystko poza tym co jest w `pub use` w `__init__.py`.

### 1.4 Feature flags

```toml
[features]
default = ["rustls-tls"]
rustls-tls = ["reqwest/rustls-tls"]
native-tls = ["reqwest/native-tls"]
blocking = []        # sync facade nad async
tracing = ["dep:tracing"]
```

---

## 2. Risk register

| Ryzyko | Mitygacja |
|---|---|
| `reqwest` 0.12 vs 0.13 fragmentacja ekosystemu | Pin 0.12 do v0.1; upgrade w v0.2 gdy `reqwest-eventsource` wyjdzie pod 0.13 |
| Parser SSE odbiegnie od serwerowego formatu | Zestaw testów na surowych bajtach z [utils/sse.py](sdks/python/src/honcho/utils/sse.py) — używamy tych samych przykładów |
| Cached metadata staje się stale po stronie usera | Dokumentacja: `refresh()` jawnie; brak auto-invalidate; testy pokazują kontrakt |
| OpenAPI spec rozjedzie się z faktycznym serwerem | Test parity: w fazie 11 wystawiamy spec do `swagger-cli validate` + porównujemy 200 OK shape z odpowiedzią serwera |
| Blocking + użytkownik już ma runtime tokio | Konstruktor `blocking::Honcho` dostaje opcjonalny `Handle`; bez Handle — własny `current_thread` runtime w `OnceLock`. Test panic gdy wywołane z innego runtime'u |
| Cancel-safety strumieni | Test: `drop(stream)` w połowie + asercja że wiremock widzi `Connection: close` |
| Workspace się nie utworzy (idempotency edge case) | Test: `_ensure_workspace` na 409 conflict mapuje na sukces (jak Python — semantyka get-or-create) |

---

## 3. Faza 0 — Bootstrap (0.5 dnia)

**Cel:** crate się buduje, CI zielone, jeden smoke test kompilacji.

- `cargo new --lib sdks/rust/honcho`
- `Cargo.toml` z zależnościami wg sekcji 1.1
- `rustfmt.toml`, `clippy.toml` (deny: `unwrap_used`, `expect_used` w `src/`, allow w `tests/`)
- `.github/workflows/rust.yml`: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps`, `cargo-msrv verify`
- MSRV: **1.80** (stabilne async traits + LazyLock)
- `tests/compile_assertions.rs`:
  ```rust
  use static_assertions::assert_impl_all;
  assert_impl_all!(honcho::Honcho: Send, Sync, Clone);
  ```

**DoD:** zielone CI na pustym crate.

---

## 4. Faza 1 — Błędy + typy domeny (1.5 dnia)

**Cel:** kompletne `HonchoError`, wszystkie 55 schematów OpenAPI jako serde modele, round-trip testy.

### 4.1 Red — testy

`tests/error_mapping.rs`:
- Macierz statusów: 400→`BadRequest`, 401→`Authentication`, 403→`PermissionDenied`, 404→`NotFound`, 409→`Conflict`, 422→`UnprocessableEntity`, 429→`RateLimit{retry_after}`, 5xx→`Server`.
- `HonchoError::from_response(status, headers, body)` parsuje `Retry-After` (sekundy + HTTP-date format).
- `Display` zawiera `status`, `body`, `request_id` jeśli obecne.
- Source chain dla `Transport(reqwest::Error)`, `Decode(serde_json::Error)`, `Io(std::io::Error)`.
- `#[non_exhaustive]` enforcement: compile-fail test że `match` bez `_` nie kompiluje się downstream.

`tests/serde_roundtrip.rs` — sterowany przez OpenAPI:
- Skrypt build.rs (lub osobny `xtask`) generuje listę nazw schematów z `docs/v3/openapi.json`.
- Per schemat: handcraftowany **minimalny** + **maksymalny** sample JSON w `tests/fixtures/<Schema>/{min,max}.json`. Round-trip: `T::deserialize → T::serialize → serde_json::Value` musi być równe input.
- Walidacja przeciw OpenAPI: `jsonschema` crate (dev-dep) sprawdza że fixtures są poprawne wg spec.
- Parametryzowane przez `rstest` po liście schematów.

### 4.2 Green — implementacja

- `src/error.rs` — `HonchoError` enum (`thiserror`), `Result<T>` alias.
- `src/types/` z modułami per zasób (workspace, peer, session, message, conclusion, dialectic, configuration, pagination). Każdy z `*Response`, `*CreateParams`, `*UpdateParams`, `*ListParams`.
- `serde_path_to_error` w warstwie HTTP przy decode → błędy zawierają ścieżkę pola.

**DoD:** ≥55 schematów ma round-trip + min/max test; pełna macierz błędów; OpenAPI spec waliduje fixtures.

---

## 5. Faza 2 — HTTP core (1.5 dnia)

**Cel:** `HttpClient` async z auth, timeoutami, retry (ręcznie), mapowaniem statusów.

### 5.1 Red

`tests/http_client.rs`:
- **Bearer auth**: wiremock matcher na `Authorization: Bearer <key>`.
- **User-Agent**: zawiera `honcho-rust/{CARGO_PKG_VERSION}`.
- **Default headers + default query**: merge z per-request headers; per-request wygrywa.
- **Custom `reqwest::Client`** wstrzyknięty przez builder — używamy go zamiast tworzyć własny.
- **Timeout**: serwer śpi 5s, klient z 1s timeout → `HonchoError::Timeout`.
- **Retry status codes**: dokładnie `{429, 500, 502, 503, 504}` retryowane; `{400, 401, 403, 404, 409, 422}` nie. Test wiremock liczy requesty per status.
- **Backoff**: użyj `tokio::time::pause()`. Sekwencja 503→503→503→200 daje delaye `[0.5s, 1.0s, 2.0s]` — asercja przez `Instant`.
- **`Retry-After` override**: 429 + header `Retry-After: 3` → czeka 3s zamiast `0.5 × 2^0`.
- **`Retry-After` HTTP-date format**: `Retry-After: Wed, 21 Oct 2026 07:28:00 GMT` poprawnie sparsowany.
- **Max retries respektowane**: `max_retries=2` → max 3 requesty (1 + 2 retry).
- **Decode error**: 200 z malformed JSON → `HonchoError::Decode` z path field.

### 5.2 Green

- `src/http/client.rs` — `HttpClient { inner: Arc<reqwest::Client>, base_url, api_key, max_retries, default_headers, default_query }`.
- Ręczna pętla retry z exponential backoff (5–10 linii) + parser `Retry-After`.
- `HttpClient::request<TBody, TResp>(method, path, body, query)` jako jedna generyczna metoda; helpery `get`/`post`/... ją wrappują.
- `src/http/routes.rs` — port wszystkich 36 ścieżek z routes.py (funkcje zwracające `String`).

**DoD:** pełna macierz testów; brak retry na 4xx (poza 429); brak `unwrap` w `src/http/`.

---

## 6. Faza 3 — Paginacja (0.5 dnia)

### 6.1 Red

`tests/pagination.rs`:
- Wiremock: 3 strony po 2 elementy (`PageResponse { items, total, size, has_next }`). Klient zbiera 6 elementów przez `try_collect`.
- `has_next=false` zamyka strumień (mock liczy = 3 requesty).
- Błąd na 2. stronie propaguje się jako `Err`, strumień się kończy bez panic.
- Transform closure: surowe `PeerResponse` → bogate `Peer { http: Arc, ... }`.
- **Cancel**: `drop(stream)` po pierwszej stronie nie wysyła requestu po drugą.

### 6.2 Green

- `src/pagination.rs` — `fn paginate<TRaw, TOut>(http, route, params, transform) -> impl Stream<Item = Result<TOut>> + Send + 'static` via `async_stream::try_stream!`.

**DoD:** strumień paginacji dla peers/sessions/messages/conclusions/workspaces.

---

## 7. Faza 4 — Honcho client + workspace operations (1 dzień)

Pierwszy pełny pionowy slice.

### 7.1 Red

`tests/client_builder.rs`:
- `Honcho::builder().api_key("k").build()` OK.
- Bez `api_key` i bez `HONCHO_API_KEY` w env → `HonchoError::Configuration`.
- `environment("local")` → base_url `http://localhost:8000`; `environment("production")` → `https://api.honcho.dev`.
- `base_url` override wygrywa nad `environment`.
- Brak `workspace_id` + brak `HONCHO_WORKSPACE_ID` → `"default"`.
- Custom `reqwest::Client` przepuszczany.

`tests/workspace.rs`:
- `client.ensure_workspace()` (lub auto przy pierwszej operacji) — wykonuje **dokładnie jeden** `POST /v3/workspaces {id}` na lifetime klienta. Druga operacja nie wysyła ponownie.
- 409 Conflict z `ensure_workspace` mapowany na sukces (get-or-create).
- `client.search(...)`, `client.queue_status()`, `client.schedule_dream()`, `client.delete_workspace(...)`.
- `client.workspaces()` — paginowany strumień stringów (nazwy).
- `client.get_configuration()` / `set_configuration(...)`.

### 7.2 Green

- `src/client.rs` — `Honcho` (Clone via `Arc<Inner>`), builder przez `#[derive(bon::Builder)]`.
- Trait `MetadataConfig` (async fns: `get_metadata`, `set_metadata`, `get_configuration`, `set_configuration`). Implementacja dla `Honcho`, `Peer`, `Session`.

**DoD:** wszystkie publiczne metody z [client.py](sdks/python/src/honcho/client.py) pokryte.

---

## 8. Faza 5 — Peer (1.5 dnia)

### 8.1 Red

`tests/peer.rs`:
- `client.peer("alice")` — bez requestu (lazy).
- `peer.refresh()` — GET.
- `peer.chat(query)` (bez stream) — POST → `String`.
- `peer.message(content)` — buduje `MessageCreateParams`, **nie** wysyła (parity z [peer.py:391](sdks/python/src/honcho/peer.py)).
- `peer.search(query)` → strumień `Message`.
- `peer.get_card()` / `set_card(...)` / `card(target)`.
- `peer.representation(...)` / `peer.context(...)`.
- `peer.sessions()` → strumień `Session`.
- `peer.conclusions()` → `ConclusionScope` (lazy).
- Concurrency: dwa `peer.clone()` w `tokio::spawn`, oba wywołują `refresh()` — brak deadlocku na RwLock.

### 8.2 Green

- `src/peer.rs` — `Peer { inner: Arc<PeerInner> }`, gdzie `PeerInner { http: HttpClient, workspace_id: String, id: String, cache: RwLock<PeerCache> }`.

---

## 9. Faza 6 — Session (1.5 dnia)

### 9.1 Red

`tests/session.rs`:
- `add_peers([alice, bob])` / `set_peers` / `remove_peers`.
- `get_peer_configuration` / `set_peer_configuration`.
- `add_messages` z **chunkingiem >100**: 150 wiadomości → wiremock przyjmuje dokładnie 2 requesty (100 + 50).
- `messages()` strumień.
- `clone(deep=true, target=...)`.
- `context(tokens=2000, summary=true)` → `SessionContext`.
- `to_openai()` / `to_anthropic()` — porównanie strukturalne (`serde_json::Value`) z fixturami wygenerowanymi z Python SDK.
- `delete()`.

### 9.2 Green

- `src/session.rs` + `src/session_context.rs`.

---

## 10. Faza 7 — Message + upload (0.5 dnia)

### 10.1 Red

`tests/upload.rs`:
- `session.upload_file(path)` → multipart `Content-Type: multipart/form-data; boundary=...`, pole `file`.
- Plik >1MB streamowany — sprawdzamy że memory footprint stały (porównanie peak RSS przed/po, tolerancja 10MB).
- I/O error → `HonchoError::Io`.

### 10.2 Green

- `src/message.rs`, `src/upload.rs` (port [utils/file_upload.py](sdks/python/src/honcho/utils/file_upload.py)).

---

## 11. Faza 8 — SSE streaming (1.5 dnia, najwyższe ryzyko)

### 11.1 Red

`tests/sse.rs` — surowe bajty wiremockowane:
- **Happy path**: `data: {"delta": "Hello"}\n\ndata: {"delta": " world"}\n\ndata: {"done": true}\n\n` → zebrany strumień = `"Hello world"`.
- **Format `done: true`** (NIE `[DONE]` sentinel) — parity z [utils/sse.py:25](sdks/python/src/honcho/utils/sse.py).
- **Komentarze SSE** (linie `: heartbeat`) ignorowane.
- **Wielolinijne `data:`** złączone (`\n` between).
- **Malformed JSON w `data:`** → `Err` w strumieniu, ale strumień nie panikuje.
- **Cancel-safety**: `tokio::pin!(stream); stream.next().await; drop(stream);` — wiremock widzi disconnect w <100ms.
- **Connection drop w połowie**: brak auto-reconnect w v0.1 (Python też nie ma) — strumień zwraca `Err(Connection)`.
- **Empty data line** (`data:\n\n`) ignorowany.

### 11.2 Green

- `src/sse.rs` — `parse_sse_stream(response: reqwest::Response) -> impl Stream<Item = Result<DialecticStreamChunk>> + Send`.
- Implementacja na `bytes_stream()` + buffer linii + ręczny split na `\n\n`.
- `Peer::chat_stream(query)` -> `impl Stream<Item = Result<String>> + Send + 'static`.

**DoD:** ścieżka SSE bez `unwrap`/`expect`; cancel-safe; parity z parserem Python na zestawie 10 nagranych przykładów.

---

## 12. Faza 9 — Conclusion + ConclusionScope (0.5 dnia)

### 12.1 Red

`tests/conclusions.rs`:
- `peer.conclusions().create(text, scope=...)`.
- `.list()` strumień.
- `.query(filters)`.
- `.delete(id)`.
- `.representation(...)`.
- Batch create > 100 → chunking jak w `add_messages`.

### 12.2 Green

- `src/conclusion.rs` (port [conclusions.py](sdks/python/src/honcho/conclusions.py)).

---

## 13. Faza 10 — Blocking facade (0.5 dnia)

### 13.1 Red

`tests/blocking.rs` pod `#[cfg(feature = "blocking")]`:
- `honcho::blocking::Honcho` ma 1:1 API surface z async, ale wszystkie metody sync.
- Paginacja → `Iterator<Item = Result<T>>`.
- Stream chat → `Iterator<Item = Result<String>>`.
- **Panic guard**: wywołanie sync API z wnętrza działającego `tokio::runtime` → custom panic message ("call async API instead").
- Runtime budowany lazy w `OnceLock`, `current_thread` flavor.

### 13.2 Green

- `src/blocking/mod.rs` — cienkie wrappery, `block_on` przez lazy runtime.

---

## 14. Faza 11 — Integration tests (1 dzień)

`tests/integration/` pod env `HONCHO_INTEGRATION=1`:

- Pełen flow: ensure_workspace → 2 peers → session → add_messages → dialectic chat → search → cleanup (delete session).
- Streaming dialectic do końca.
- Upload pliku + retrieval.
- **Parity test**: identyczny skrypt w Pythonie i Ruście, porównanie outputu pomijając pola `id`, `created_at`, `updated_at`. `xtask test-parity` uruchamia oba i diff'uje.
- CI: tylko na `main`, secret `HONCHO_API_KEY` w GitHub Actions.

---

## 15. Faza 12 — Dokumentacja, examples, release (1 dzień)

- `cargo doc` — każdy `pub` item z `///` + przykład; `#![deny(missing_docs)]`.
- `examples/`: `quickstart.rs`, `streaming.rs`, `blocking.rs`, `multi_peer.rs`, `upload.rs`.
- `README.md` z installation + quickstart.
- `CHANGELOG.md` v0.1.0.
- `cargo publish --dry-run`.
- Publikacja `honcho` na crates.io (nazwa potwierdzona jako wolna).

---

## 16. Harmonogram

| Faza | Czas | Kumul. |
|---|---|---|
| 0. Bootstrap | 0.5 | 0.5 |
| 1. Błędy + typy + OpenAPI fixtures | **1.5** | 2.0 |
| 2. HTTP + retry + Retry-After | 1.5 | 3.5 |
| 3. Paginacja | 0.5 | 4.0 |
| 4. Honcho + workspace | 1.0 | 5.0 |
| 5. Peer | 1.5 | 6.5 |
| 6. Session | 1.5 | 8.0 |
| 7. Message + upload | 0.5 | 8.5 |
| 8. SSE | 1.5 | 10.0 |
| 9. Conclusion | 0.5 | 10.5 |
| 10. Blocking | 0.5 | 11.0 |
| 11. Integration + parity | 1.0 | 12.0 |
| 12. Docs + release | 1.0 | 13.0 |
| **Bufor (ryzyka z sekcji 2)** | **2.0** | **15.0** |

**Łącznie: 15 dni roboczych** (vs ~14.5 w v1 — różnica to wbudowany bufor + Faza 1 +0.5 na walidację OpenAPI).

---

## 17. Parity matrix — co MUSI działać identycznie

| Feature | Python | Rust | Test |
|---|---|---|---|
| Backoff seq | `0.5, 1.0, 2.0, 4.0` | identycznie | tests/http_client.rs |
| Retry codes | `{429,500,502,503,504}` | identycznie | tests/http_client.rs |
| `_ensure_workspace` 1× | tak | tak | tests/workspace.rs |
| SSE done flag | `{"done":true}` | identycznie | tests/sse.rs |
| Batch chunk size | 100 | 100 | tests/session.rs, tests/conclusions.rs |
| Env vars | `HONCHO_API_KEY`, `HONCHO_URL`, `HONCHO_WORKSPACE_ID` | identycznie | tests/client_builder.rs |
| Default workspace | `"default"` | `"default"` | tests/client_builder.rs |
| Env URLs | `local: localhost:8000`, `production: api.honcho.dev` | identycznie | tests/client_builder.rs |
| `to_openai`/`to_anthropic` | shape per fixture | strukturalnie równe | tests/session.rs (fixtures z Pythona) |

---

## 18. Niezmienniki TDD

1. **Red przed green** — żaden commit z implementacją bez wcześniejszego padającego testu.
2. **Jedna asercja behawioralna per test**.
3. **Brak `unwrap`/`expect` w `src/`** — clippy `unwrap_used = deny`.
4. **Każdy publiczny błąd: `Error + Send + Sync + 'static`** — `assert_impl_all!`.
5. **Każdy publiczny `Future`/`Stream`: `Send + 'static`** — `assert_impl_all!` dla zwrotów.
6. **`#[non_exhaustive]` na publicznych enum + struct response'ach**.
7. **MSRV egzekwowane przez `cargo-msrv` w CI**.
8. **OpenAPI spec jako single source of truth dla shape'ów** — fixtures walidowane przeciw spec.

---

## 19. Changelog vs v1

| Co | v1 | v2 |
|---|---|---|
| reqwest version | 0.13 | **0.12** (kompatybilność SSE) |
| reqwest-eventsource | tak | **usunięty** (ręczny parser) |
| reqwest-middleware/retry | tak | **usunięte** (ręczna pętla, ~50 linii) |
| Builder | `typed-builder` | `bon 3.9` |
| OpenAPI spec | nieużywany | source of truth dla schemas + fixtures |
| Retry codes | `408/429/500/502/503/504` | `{429,500,502,503,504}` (parity z Python) |
| SSE done | `[DONE]` sentinel | `{"done":true}` JSON flag |
| `_ensure_workspace` | lazy "przy 1. requeście" | jeden POST per instancja, 409→OK |
| Env vars | `HONCHO_BASE_URL` | `HONCHO_URL` + `HONCHO_WORKSPACE_ID` |
| Domyślny workspace | brak wzmianki | `"default"` |
| Środowiska | brak | `local` / `production` z hardkodowanymi URL |
| Ctor params | częściowo | pełne: `max_retries`, `timeout`, `default_headers`, `default_query`, `http_client` |
| Concurrency model | brak | `Arc<Inner>` + `RwLock` na cache, dokumentacja Clone semantyki |
| `Send + Sync` bounds | brak | explicit `assert_impl_all!` |
| `#[non_exhaustive]` | brak | wszędzie na pub enum/struct |
| Cancel-safety | wspomniana | dedykowany test wiremock disconnect |
| Bufor | 2 dni "w prozie" | wbudowany w harmonogram (15 dni total) |
| Faza 1 | 1 dzień | 1.5 dnia (OpenAPI validation) |
| Webhooks/Keys | nie wymienione | jawnie out-of-scope v0.1 |
