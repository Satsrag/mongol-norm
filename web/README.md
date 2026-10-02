# mongol-norm playground

A static, English/Chinese bilingual playground for the Hudum word APIs. Enter a word to
see its `shape` sequence and `normalize` result, with Unicode code points,
copy buttons, control-character insertion, and vertical previews rendered in
the bundled `hudum.otf`. Input stays in the browser.

**[Open the live playground](https://www.satsrag.dev/norm/)** ·
**[在线使用](https://www.satsrag.dev/norm/)**

The WebAssembly adapter calls the Rust crate at `../..` directly. It has its
own Cargo workspace and lockfile so browser build dependencies do not affect
the core crate's zero-dependency or MSRV guarantees.

## Run locally

Prerequisites: current stable Rust, Node.js 22+, npm, and Python 3. Install the
WebAssembly target and the pinned binding generator once:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.125 --locked
```

From the repository root:

```sh
npm --prefix web run dev
```

Open <http://127.0.0.1:4173>. No npm install is needed to build or serve the app.
After editing the Rust engine, rebuild and refresh. HTML, CSS, and JavaScript
changes only need a refresh. To serve an existing build without rebuilding:

```sh
npm --prefix web run serve
```

## Build and host

```sh
npm --prefix web run build
```

Serve or deploy the contents of `web/public/`, including the generated `pkg/`
directory and `fonts/`. No application server is needed; the HTTP server must
serve `.wasm` as `application/wasm`. Relative asset paths support a subdirectory
deployment. Opening `index.html` through `file://` is not supported.

`.github/workflows/deploy-web.yml` builds and browser-tests the page for each
published GitHub Release, then deploys the tested artifact to `norm/` in
`Satsrag/satsrag.github.io` using `PAGE_DEPLOY_KEY`, just like mongol-convert's
`convert/` page. Pull requests and default manual runs test without deploying.
For an initial deploy or a redeploy, run the workflow on `main` with `deploy=true`.
The site homepage has a permanent link to `/norm/`; deployments only update that
directory. See [release setup](../docs/releasing.md#the-web-playground).

The UI uses `shape` and `normalize`, which accept a single Mongolian word.
It does not trim, split, or silently rewrite the input. Spaces, mixed-script
text, and currently unsupported ZWNJ produce the engine's actual errors.
The match indicator compares the engine's shape sequences, not rendered pixels.
Norm is a canonical encoding by shape, not an orthographic spelling correction.

## Browser tests

```sh
npm --prefix web ci
cd web
npx playwright install chromium
npm test
```

To test with an installed Google Chrome instead of Playwright's Chromium:

```sh
PLAYWRIGHT_CHANNEL=chrome npm test
```

Tests compare browser results with the native CLI, check hidden controls and
errors, clipboard contents, input-method composition, mobile overflow, font
loading, and a failed engine load. For the Rust adapter:

```sh
cargo fmt --manifest-path web/wasm/Cargo.toml --check
cargo clippy --manifest-path web/wasm/Cargo.toml --target wasm32-unknown-unknown --locked -- -D warnings
```

## Font provenance

`public/fonts/hudum.otf` is the font previously generated from
Kushim-Jiang/mongolian commit `37aa567`; it is bundled unchanged. Its Noto
outlines use SIL OFL 1.1, and mongfontbuilder's generated rules use MIT.
Provenance, checksum, and both license notices are in `public/fonts/` and ship
with the static site.

The browser uses the same versioned canonical shape policy as the core: redundant
interior Hudum ZWJ is omitted before duplicate unification; boundary ZWJ is retained.
The input preview still displays the original string. Rebuild WASM after changing
the engine; the bundled font is unchanged by this policy update.
