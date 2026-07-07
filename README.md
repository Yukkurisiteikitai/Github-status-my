# GitHub Rank SVG API

GitHub の `commits / PRs / stars` からランクを判定し、ASCII art を含む SVG を返す API です。

## Endpoint

- Production: `https://github-status-my.vercel.app/rank`
- Local: `http://127.0.0.1:8080/rank`

## Query Parameters

- `user` (required): GitHub username
- `bar` (optional): `true` で進捗表示を有効化
- `style` (optional): 進捗表示スタイル
	- `bar` (default)
	- `blocks` (`=====-----`)
	- `dots` (`■■■□□□`)
	- `percentage` (`85%`)
- `width` (optional): 出力幅（例: `820`）
- `height` (optional): 出力高さ（例: `400`）

デフォルトサイズは `820x400` です。

## Rank Rules

- `D`: 初期状態
- `C`: commits >= 50, PRs >= 5
- `B`: commits >= 200, PRs >= 20
- `A`: commits >= 500, PRs >= 50, stars >= 10
- `S`: commits >= 1000, PRs >= 100, stars >= 50

## Notes

- `bar=true` のときだけ Next 進捗を表示します。
- `S` ランクで `bar=true` の場合、次ランクがないため 1 行猫 ASCII (`/\_/\ (=^.^=)`) を表示します。
- 描画は 2 カラム構成です。
	- 左: Rank ASCII art
	- 右: User / Stats / Rank / Next / Progress

## Markdown Examples

### Basic

![](https://github-status-my.vercel.app/rank?user=Yukkurisiteikitai)

### With Progress (default bar)

![](https://github-status-my.vercel.app/rank?user=Yukkurisiteikitai&bar=true)

### Blocks Style

![](https://github-status-my.vercel.app/rank?user=Yukkurisiteikitai&bar=true&style=blocks)

### Dots Style

![](https://github-status-my.vercel.app/rank?user=Yukkurisiteikitai&bar=true&style=dots)

### Percentage Style

![](https://github-status-my.vercel.app/rank?user=Yukkurisiteikitai&bar=true&style=percentage)

### Custom Size

![](https://github-status-my.vercel.app/rank?user=Yukkurisiteikitai&bar=true&width=820&height=400)

### S Rank Sample

![](https://github-status-my.vercel.app/rank?user=cordx56&bar=true)

## Local Run

1. `GITHUB_PAT` か `github_pat` を環境変数に設定
2. `cargo run --bin github-status-my`
3. `http://127.0.0.1:8080/rank?user=<username>&bar=true`

必要なら `BIND_ADDR=0.0.0.0:8080 cargo run --bin github-status-my` のように待ち受け先を変更できます。

### HTTP/TLS Diagnostics

- 既定値は `GITHUB_HTTP_BACKEND=native-tls` と `GITHUB_HTTP_FORCE_HTTP1=true` です。
- `GITHUB_HTTP_BACKEND=rustls` で `reqwest + rustls` に切り替えられます。
- `GITHUB_HTTP_FORCE_HTTP1=false` で HTTP バージョン交渉を既定動作に戻せます。
- `GITHUB_HTTP_TIMEOUT_SECS=20` で全体タイムアウトを変更できます。
- `GITHUB_API_BASE=https://api.github.com` で API 送信先を切り替えられます。
- `GITHUB_HTTP_TRACE=true` にすると成功レスポンスでも GitHub request ID / rate limit / TLS 証明書情報をログ出力します。

例:

```bash
GITHUB_HTTP_BACKEND=rustls \
GITHUB_HTTP_FORCE_HTTP1=false \
GITHUB_HTTP_TRACE=true \
cargo run --bin github-status-my
```

エラー時は標準エラーに `request_id`, `backend`, `protocol`, `endpoint`, `error_chain` を含む詳細ログを出力します。

## Response Type

- `Content-Type: image/svg+xml; charset=utf-8`
