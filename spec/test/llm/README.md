# One-shot live LLM test

This opt-in integration test sends the compact authoring guide in
[`system-prompt.md`](system-prompt.md) and the drawing request in
[`prompt.txt`](prompt.txt) to the real DeepSeek Flash API (`deepseek-flash`). It
makes exactly one chat-completion request, with thinking disabled, no tools,
no retries, and no repair loop. The returned message content is saved as
`generated.locus`, adding a terminal newline if absent, then parsed, solved,
checked, and rendered by Locus. No declarations or assertions are repaired.

The guide covers `0 beta 5` declarations, relative placement, geometry
constructors, plain-text annotations, and assertions needed for this task.
The full repository specification is not included in the API request.

The task is a labeled 3–4–5 triangle with an altitude, incircle, and circumcircle.
The test checks the model's assertions and a separate geometry contract, so an
unrelated valid file cannot pass. Plain-text labels use the bundled font; no
external TeX installation is needed.

From the repository root, copy `.env.example` to `.env` and replace the
placeholder with your DeepSeek API key. `.env` and `.env.local` are ignored by
Git. Load the file into your shell before running the test:

```sh
set -a
source .env
set +a
cargo test -p locus-render --test llm_one_shot --features png -- --ignored --nocapture
```

An already exported `DEEPSEEK_API_KEY` also works. The test requires `curl` and
network access to `https://api.deepseek.com`. Ordinary test runs skip it and
make no API calls. Each explicit run uses the paid API.

Each run prints its own directory under `target/llm-one-shot/`, preserving:

- `system-prompt.txt`, `prompt.txt`, and `request.json`: exact input, without credentials.
- `response.json`: raw API response, including usage and the returned model name.
- `generated.locus`: response content with the required file-ending newline,
  retained even if validation fails. `response.json` preserves the original text.
- `assertions.json` and `contract.json`: evaluated checks.
- `generated.svg` and `generated.png`: the picture, produced after checks pass.

Without `--features png`, the test renders SVG only. API errors, truncation,
invalid source, failed geometry checks, and rendering errors fail the test;
there is no mocked response or cached-answer fallback.
