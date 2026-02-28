# スタイリング戦略の調査記録

UnoCSS を RSC + multi-environment 構成に導入しようとした際の調査・問題整理。

## 結論

**`@unocss/vite` はどのモードも RSC multi-environment 構成を完全にはサポートしていない。**

公式 issue [#4990](https://github.com/unocss/unocss/issues/4990) として報告・認識されているが、修正 PR は未提出（2026-02 時点）。

---

## 問題の本質

### Vite の multi-environment とは

`@vitejs/plugin-rsc` は Vite の Environment API（Vite 6 で正式導入）を使い、3つの独立したビルド環境を作る。

```
rsc 環境    → Cloudflare Workers で実行（サーバーコンポーネント）
 └── ssr 環境 → 同 Worker 内（SSR レンダラー）
client 環境  → ブラウザ（JS バンドル）
```

各環境は独立したプラグインパイプラインを持つ。これが問題の根本。

### UnoCSS global モードの前提

global モードは「ビルドは1環境」という前提で設計されている。`renderChunk` フックで Vite 内部プラグイン `vite:css-post` を `options.dir`（出力ディレクトリ）をキーに探す。

```ts
// packages/vite/src/modes/global/build.ts
const cssPost = cssPostPlugins.get(options.dir);
if (!cssPost) {
  this.warn("[unocss] failed to find vite:css-post plugin...");
  return null; // CSS 生成をサイレントスキップ
}
```

`vite:css-post` はブラウザ向け CSS 処理のプラグインなので、rsc・ssr 環境には存在しない。multi-environment 構成では `cssPostPlugins` マップのキーが一致せず、CSS 生成が常にスキップされる。

### Vite 8 / Rolldown は直接の原因ではない

| 要因                                       | 原因か               |
| ------------------------------------------ | -------------------- |
| RSC multi-environment 構成（Vite 6+）      | ✓ 本質               |
| `@unocss/vite` が multi-environment 未対応 | ✓ プラグイン側の問題 |
| Vite 8 / Rolldown                          | ✗ 無関係             |

Vite 6 の multi-environment 対応時点から同じ問題が存在する。

---

## 各モードの評価

### global モード

- `vite:css-post` が rsc・ssr 環境で見つからず CSS 生成がスキップされる
- **使用不可**

### dist-chunk モード

- global モードと同様に `vite:css-post` に依存する
- Vite 6.3 で `transform.handler` API 変更による別の問題もあった（PR #4598 で修正済み）
- **使用不可**

### per-module モード

- `vite:css-post` に依存しないため、ビルドは通る
- CSS をブラウザ側 JS モジュールごとの仮想 CSS として注入する方式
- **問題1**: サーバーコンポーネント（`root.tsx` 等）は rsc 環境で処理されるため、dev サーバーでクラスが補足されないことがある
- **問題2**: rsc・ssr・client の全環境で CSS が生成され重複する
- **条件付き使用可**（ワークアラウンドとして許容できるか次第）

---

## RSC アーキテクチャにおける CSS の正しい配置

RSC レスポンス（React Flight 形式）は CSS を含まない。

```
SSR レンダリング → <head> に <link rel="stylesheet"> が必要
                              ↑
                    ここに CSS がないと FOUC が発生
```

CSS をクライアント JS バンドル経由だけで読み込む構成では、SSR で描画された HTML にスタイルが当たらず、JS ロード完了まで未スタイルで表示される（FOUC）。

理想は `<head>` 内の `<link rel="stylesheet">` で CSS を SSR 時点から読み込むこと。

---

## per-module モードがサーバーコンポーネントを捕捉できない理由

per-module モードは `transform` フックでファイルをスキャンし、そのファイル自身に仮想 CSS import を注入する。

```ts
// root.tsx を rsc 環境で transform した結果（イメージ）
import "/@unocss/abc123.css"; // ← 注入される
// ...元のコード
```

問題は各環境のモジュールグラフが独立していること。

```
rsc 環境のモジュールグラフ:
  entry.rsc.tsx
    └── root.tsx        ← transform → /@unocss/abc123.css が注入される
          └── client.tsx

client 環境のモジュールグラフ:
  entry.browser.tsx
    └── client.tsx      ← transform → /@unocss/def456.css が注入される
```

ブラウザが読み込むのは client 環境のモジュールグラフのみ。`root.tsx` は rsc 環境でのみ処理され、ブラウザは RSC Flight 形式のレスポンスを受け取るだけで `root.tsx` 自体を import しない。そのため `/@unocss/abc123.css` はブラウザに届かない。

これは dev 限定の問題ではなく、ビルド時も同様。client 環境の入口（`entry.browser.tsx`）から辿れないモジュールの CSS は生成されない。

---

## RSC アーキテクチャにおける理想の挙動

### CSS 生成：モジュールグラフに依存しない全ファイルスキャン

```
src/ 以下の全 .tsx/.ts ファイルをスキャン
  ├── root.tsx        （サーバーコンポーネント）
  ├── client.tsx      （クライアントコンポーネント）
  └── ...
  ↓
全クラスを統合して1つの CSS を生成
```

モジュールグラフではなくファイルシステムを対象にすることで、サーバーコンポーネントのクラスも漏れなく捕捉できる。

### CSS 配信：SSR 時点で `<head>` に `<link>` を置く

```html
<!-- SSR が返す HTML -->
<head>
  <link rel="stylesheet" href="/assets/app.css" />
  <!-- JS ロード前からスタイルが当たる → FOUC なし -->
</head>
```

CSS を JS バンドル経由で読み込むと、SSR で描画された HTML が JS ロード完了まで未スタイルになる（FOUC）。React 19 の `<link precedence>` を使えば SSR ストリーミング中でも `<head>` に巻き上げられる。

### HMR：変更を検知して CSS 仮想モジュールを無効化

```
src/root.tsx が変更される
  ↓
CSS 仮想モジュール（/@unocss/app.css）を無効化
  ↓
ブラウザが再 fetch → 新しいクラスが反映
```

### CSS は client 環境にのみ存在する

```
rsc 環境   → CSS なし（Flight 形式のレスポンスに CSS は含まれない）
ssr 環境   → CSS なし（<link> タグで参照するだけ）
client 環境 → app.css を生成・配信
```

### まとめ

| 要件                                 | 手段                                               |
| ------------------------------------ | -------------------------------------------------- |
| サーバーコンポーネントのクラスも捕捉 | ファイルシステムスキャン（モジュールグラフ非依存） |
| FOUC なし                            | `<head>` に `<link rel="stylesheet">`              |
| HMR                                  | transform フックで変更検知、仮想モジュール無効化   |
| CSS 重複なし                         | `environment.name === "client"` のみで CSS 生成    |

---

## 検討したアプローチ

### `@unocss/core` で Vite プラグインを自作する

`createGenerator` を直接使い、multi-environment を意識したプラグインを書く。

```
- buildStart: src/ 以下の全ファイルをスキャンしてソースとして登録
- transform: ファイル変更時にソースを更新、CSS 仮想モジュールを無効化
- resolveId: "uno.css" → "/@unocss/app.css" に解決（.css 拡張子で Vite がネイティブ処理）
- load: client 環境のみ CSS を生成して返す（rsc・ssr 環境は空文字を返す）
```

`environment?.name !== "client"` で環境を判別することで CSS の重複生成を回避できる。

### 未解決の課題

- **TypeScript エラー**: `vite.config.ts` の tsconfig が DOM 向け設定のため、Node/Bun の API（`node:fs` 等）の型が通らない
- **FOUC**: `<head>` に `<link rel="stylesheet">` を置くには、ビルド時の CSS ファイル名を固定する必要がある

---

## 参考 issue / PR

| 番号                                                  | 内容                                                   | 状態                      |
| ----------------------------------------------------- | ------------------------------------------------------ | ------------------------- |
| [#4990](https://github.com/unocss/unocss/issues/4990) | Vite Environment API で global モードが CSS 生成しない | Open・未解決              |
| [#3198](https://github.com/unocss/unocss/issues/3198) | Next.js 13 RSC でクラスがスキャンできない              | Closed（根本修正なし）    |
| [#4597](https://github.com/unocss/unocss/issues/4597) | Vite 6.3 で dist-chunk が壊れた                        | Closed（PR #4598 で修正） |
| [#4598](https://github.com/unocss/unocss/pulls/4598)  | transform.handler 対応                                 | Merged                    |
