# ai-service

One wrapper for AI providers. Other modules call it; none of them talks to a provider directly.

- **Text and reading pictures:** Claude (Anthropic), OpenAI and Gemini, with one `Request` type.
- **Pictures in, JSON out:** send a photo (a passport page, a device label, a receipt) and get a parsed JSON value back.
- **Image generation:** OpenAI.
- **Default provider and models:** chosen in `.env`; a request may override both.
- **AI API logs:** every call, successful or not, is written to the `ai_api_logs` table and read at
  `/admin/ai-api-logs` (permissions `ai_api_logs.view` and `ai_api_logs.manage`).

```rust
use module_ai_service::{AiService, ImageRequest, Provider, Request};

let ai = AiService::new(&ctx)?;

// Text with the default provider and model
let reply = ai.chat(Request::new("Translate to Bangla: Good morning").system("Reply with the translation only.")).await?;

// A photo to JSON, from a named provider. `sensitive()` keeps the prompt and the reply out of the AI API logs.
let (passport, reply) = ai
    .json(Request::new("Read this passport page. Return surname, given_names, number, date_of_birth, expiry_date.")
        .image("image/jpeg", photo_bytes)
        .provider(Provider::Gemini)
        .purpose("passport-scan")
        .sensitive())
    .await?;

// A picture
let pictures = ai.generate_image(ImageRequest::new("a red bicycle, flat illustration").size("1024x1024").purpose("blog-cover")).await?;
std::fs::write(format!("cover.{}", pictures[0].extension()), &pictures[0].bytes)?;
```

`reply.text`, `reply.provider`, `reply.model`, `reply.input_tokens`, `reply.output_tokens` and `reply.cost` (an estimate, when
prices are set in `.env`) come with every answer. `ai.json(...)` also finds JSON inside a markdown fence or a sentence.

## Settings (`.env`)

| Variable | Meaning |
|---|---|
| `AI_DEFAULT_PROVIDER` | `claude`, `openai` or `gemini`. Empty: the first provider that has a key (in that order) |
| `AI_DEFAULT_MODEL` | model for the default provider only (empty: that provider's own `*_MODEL`) |
| `ANTHROPIC_API_KEY`, `CLAUDE_MODEL` | Claude |
| `OPENAI_API_KEY`, `OPENAI_MODEL` | OpenAI text |
| `OPENAI_IMAGE_MODEL`, `OPENAI_IMAGE_SIZE`, `OPENAI_IMAGE_QUALITY`, `OPENAI_IMAGE_OUTPUT_FORMAT` | OpenAI images |
| `GEMINI_API_KEY`, `GEMINI_MODEL` | Gemini |
| `AI_MAX_TOKENS`, `AI_TIMEOUT_SECONDS` | defaults for every call (2048 and 60) |
| `AI_LOG_CONTENT` | `false`: prompts and replies are never stored (sizes, status and cost still are) |
| `CLAUDE_INPUT_USD_PER_MILLION_TOKENS`, `CLAUDE_OUTPUT_USD_PER_MILLION_TOKENS` (and `OPENAI_`, `GEMINI_`) | prices, to estimate cost |

Model names change often, so none is hard-wired: the built-in defaults are only a fallback. A provider with no key is
simply unavailable; asking for it answers "set GEMINI_API_KEY in .env". Run `cargo run -- task ai-service:status` to see
what is set (keys are never printed).

## Personal data

Pictures are never stored. Mark a call `.sensitive()` (passports, IDs, anything about a person) and neither the prompt
nor the reply is stored either; the row keeps only the provider, model, purpose, time, tokens, status and error.

## AI API logs screen

`/admin/ai-api-logs`: filters (status, provider, kind, text), **page length 25, 50, 100, 200 or 500**, a checkbox per row
and a select-all checkbox. **Delete selected** and **Delete all N matching** (every page of the current filters) both
ask for confirmation. **Delete old** removes everything older than N days. The ID opens one call with its prompt,
reply, tokens, cost and error. Reading needs `ai_api_logs.view`, deleting needs `ai_api_logs.manage`.

## Commands

| Command | What it does | Scheduled |
|---|---|---|
| `ai-service:status` | default provider, key set or missing, model per provider | |
| `ai-service:ask prompt:"..." provider:claude model:...` | send one prompt and print the reply | |
| `ai-service:generate-image prompt:"..." size:1024x1024 output:storage/ai-image` | make a picture and save it | |
| `ai-service:purge-ai-api-logs days:30` | delete old AI API logs | daily 03:50 |
