# entry.rsc.tsx / entry.ssr.tsx 解説

## entry.rsc.tsx — RSCエントリー

Cloudflare Workersのメインハンドラー。全リクエストの入り口。

### ハンドラー全体の構造

```
handler(request)
  ↓
parseRenderRequest()  // リクエスト種別を判定
  ↓
isAction == true?
  ├── actionId あり → JS有効時のサーバーアクション処理
  └── actionId なし → JS無効時のサーバーアクション処理（プログレッシブエンハンスメント）
  ↓
renderToReadableStream(<Root />)  // RSCストリーム生成
  ↓
isRsc == true?
  ├── YES → RSCストリームをそのまま Response として返す
  └── NO  → SSR処理へ委譲 (entry.ssr.tsx の renderHTML を呼ぶ)
```

### SSRモジュールの動的ロード

```typescript
const { renderHTML } = await import.meta.viteRsc.loadModule<typeof import("./entry.ssr.tsx")>(
  "ssr",
  "index",
);
```

`import.meta.viteRsc.loadModule` は `@vitejs/plugin-rsc` が提供する特殊API。`"ssr"` 環境の `"index"` エントリー（= `entry.ssr.tsx`）を動的にロードする。通常の `import` ではなくこの形式にする理由は、SSR環境のコードをRSC環境のバンドルから分離するため。

### RSCペイロードの型

```typescript
export type RscPayload = {
  root: React.ReactNode; // コンポーネントツリー
  returnValue?: { ok: boolean; data: unknown }; // サーバーアクションの戻り値
  formState?: ReactFormState; // フォームのプログレッシブエンハンスメント状態
};
```

アクション処理後に `returnValue` / `formState` をセットし、新しい `<Root />` レンダリングと**同一のRSCペイロード**に含めて返す。これにより mutation → refetch が1HTTPリクエストで完結する。

---

## entry.ssr.tsx — SSRエントリー

RSCストリームを受け取ってHTMLに変換し、ブラウザのハイドレーション用ペイロードを注入する。

### RSCストリームのtee（複製）

```typescript
const [rscStream1, rscStream2] = rscStream.tee();
```

同じRSCストリームを2つの用途に使い分ける:

|      | rscStream1                                                          | rscStream2                                                            |
| ---- | ------------------------------------------------------------------- | --------------------------------------------------------------------- |
| 用途 | SSRのHTML生成                                                       | ブラウザのハイドレーション用                                          |
| 処理 | `createFromReadableStream` → VDOM → `renderToReadableStream` → HTML | `injectRSCPayload` で `<script>FLIGHT_DATA</script>` としてHTMLに注入 |

### SsrRoot の設計

```typescript
let payload: Promise<RscPayload>;
function SsrRoot() {
  // ReactDOMServer コンテキスト内で初めて逆シリアライズを開始する
  payload ??= createFromReadableStream<RscPayload>(rscStream1);
  return React.use(payload).root;
}
```

`createFromReadableStream` は `renderToReadableStream(<SsrRoot />)` の内部で初めて呼ばれる。これは ReactDOM Server の `preinit` / `preloading`（リソースヒント）が正しく動作するためにReactDOMServerのコンテキストが必要なため。

### HTMLストリームへのペイロード注入

```typescript
responseStream = responseStream.pipeThrough(
  injectRSCPayload(rscStream2, { nonce: options?.nonce }),
);
```

`rsc-html-stream` ライブラリが提供する `TransformStream`。HTMLストリームを流しながら、RSCペイロードを `<script>FLIGHT_DATA...</script>` タグとして末尾に注入する。ブラウザ側では `rsc-html-stream/client` の `rscStream` がこのタグからペイロードを読み出す。

### SSR失敗時のフォールバック

```typescript
} catch (e) {
  status = 500;
  htmlStream = await renderToReadableStream(
    <html lang="en"><body><noscript>Internal Server Error: SSR failed</noscript></body></html>,
    {
      bootstrapScriptContent: `self.__NO_HYDRATE=1;` + bootstrapScriptContent,
    },
  );
}
```

SSRが例外を投げた場合:

1. 空のシェルHTMLを返す（`<noscript>` でnoJSユーザーへのフォールバックメッセージも含む）
2. `self.__NO_HYDRATE=1` をインラインスクリプトでセット

ブラウザ側の `entry.browser.tsx` はこのフラグを検出し、`hydrateRoot` ではなく `createRoot` を使ってまっさらなCSRを行う。CSRによってServer Componentのエラーが再現され、`GlobalErrorBoundary` がキャッチしてエラーUIを表示する。

### `debugNojs` モード

`/?__nojs` クエリパラメータが付いている場合、`bootstrapScriptContent`（ブラウザJSの起動スクリプト）を省略し、RSCペイロードの注入もスキップする。これでJavaScript完全無効の環境をシミュレートし、プログレッシブエンハンスメントのテストができる。
