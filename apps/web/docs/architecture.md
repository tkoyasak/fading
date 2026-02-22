# apps/web アーキテクチャ解説

Vite + React 19 RSC (React Server Components) + Cloudflare Workers の構成。

## ディレクトリ構成

```
src/
├── framework/
│   ├── entry.browser.tsx  # ブラウザエントリー（ハイドレーション・ナビゲーション）
│   ├── entry.rsc.tsx      # RSCエントリー（Cloudflare Workersのメインハンドラー）
│   ├── entry.ssr.tsx      # SSRエントリー（RSCストリーム → HTML変換）
│   ├── error-boundary.tsx # グローバルエラーバウンダリ
│   └── request.tsx        # リクエスト種別の判定・生成ロジック
├── action.tsx             # "use server" — サーバーアクション定義
├── client.tsx             # "use client" — クライアントコンポーネント定義
└── root.tsx               # ルートServer Component
```

## Viteのマルチ環境ビルド

RSCは「サーバー」「クライアント」「SSR」のコードを明確に分離するため、Viteは3つの独立したビルド環境を持つ。

```
vite.config.ts
├── rsc環境    → entry.rsc.tsx     (Cloudflare Workersのメインエントリー)
│   └── ssr環境 → entry.ssr.tsx   (同じWorker内で実行、childEnvironments)
└── client環境 → entry.browser.tsx (ブラウザに配信するJS)
```

`cloudflare()` プラグインが `rsc` 環境を Cloudflare Workers としてデプロイする。`ssr` は `childEnvironments` として `rsc` と同じWorkerプロセス内で動くが、独立したバンドルとして分離される。

## リクエストの振り分け

全リクエストは `entry.rsc.tsx` の `handler()` に入り、`parseRenderRequest()` でリクエスト種別を判定する。

| リクエスト                              | isRsc | isAction | 処理                         |
| --------------------------------------- | ----- | -------- | ---------------------------- |
| `GET /`                                 | false | false    | SSR → HTML返却               |
| `GET /_.rsc`                            | true  | false    | RSCストリームをそのまま返却  |
| `POST /_.rsc` + `x-rsc-action` ヘッダー | true  | true     | JS有効時のサーバーアクション |
| `POST /` (formsubmit)                   | false | true     | JS無効時のサーバーアクション |

`_.rsc` というURLサフィックスが「RSCリクエストか否か」の唯一のシグナル。サーバー側では `url.pathname` からサフィックスを除去して正規化する。

## RSCストリームとは

`renderToReadableStream<RscPayload>` が生成するのは通常のHTMLではなく、**React Flight形式**のバイナリストリーム。

```typescript
type RscPayload = {
  root: React.ReactNode; // Rootコンポーネントツリー
  returnValue?: { ok: boolean; data: unknown }; // サーバーアクションの戻り値
  formState?: ReactFormState; // form用のプログレッシブエンハンスメント状態
};
```

ブラウザ側では `createFromReadableStream` / `createFromFetch` でVDOM（React要素）に逆シリアライズして使う。

## 全体のデータフロー

```
初期ロード:
  Browser: GET /
  Worker(RSC): <Root /> を RSCストリームに変換
  Worker(SSR): RSCストリーム → HTML + <script>FLIGHT_DATA</script>
  Browser: HTML表示 (SSR) → FLIGHT_DATAからペイロード取得 → hydrateRoot

SPAナビゲーション:
  リンククリック → e.preventDefault() → history.pushState
  → fetchRscPayload() → GET /new-path/_.rsc
  → Worker: RSCストリームを直接返却
  → createFromFetch でVDOMに変換 → setPayload で再レンダリング

サーバーアクション（JS有効）:
  ボタンクリック → setServerCallback
  → POST /_.rsc (x-rsc-action: id, body: encoded args)
  → Worker: アクション実行 + <Root /> を再レンダリング → RSCペイロード返却
  → setPayload でUI更新 + returnValue を呼び出し元に返却

サーバーアクション（JS無効）:
  フォームsubmit → POST /
  → Worker: decodeAction → アクション実行 → <Root /> を再レンダリング → SSR → HTML返却
```

---

各コンポーネントの詳細は個別ファイルを参照:

- [entry.browser.tsx](./entry-browser.md)
- [entry.rsc.tsx + entry.ssr.tsx](./entry-rsc-ssr.md)
- [サーバーアクション](./server-actions.md)
