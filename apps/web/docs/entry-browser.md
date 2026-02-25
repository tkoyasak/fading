# entry.browser.tsx 解説

ブラウザ側のエントリーポイント。ハイドレーション・クライアントナビゲーション・HMRを担う。

## 初期化フロー

```typescript
async function main() {
  // SSRで注入された <script>FLIGHT_DATA</script> からRSCペイロードを取り出す
  const initialPayload = await createFromReadableStream<RscPayload>(rscStream);

  function BrowserRoot() {
    const [payload, setPayload_] = React.useState(initialPayload);
    // ...
    return payload.root;
  }

  // hydrateRoot でSSR済みHTMLに紐付け
  hydrateRoot(document, <BrowserRoot />, { formState: initialPayload.formState });
}
```

SSR失敗時は `globalThis.__NO_HYDRATE` がセットされており、その場合は `createRoot` でまっさらなCSRにフォールバックする。

## setPayload の設計

`BrowserRoot` の外から再レンダリングをトリガーするため、`useState` の setter を関数スコープの変数に「流出」させる。

```typescript
let setPayload: (v: RscPayload) => void;

function BrowserRoot() {
  const [payload, setPayload_] = React.useState(initialPayload);

  React.useEffect(() => {
    setPayload = (v) => React.startTransition(() => setPayload_(v));
  }, [setPayload_]);

  return payload.root;
}
```

`React.startTransition` でラップすることで、ナビゲーションやサーバーアクション完了時のUI更新をトランジション扱いにし、Suspenseと協調させる。

## クライアントサイドナビゲーション

SPA的なナビゲーションをフレームワークライブラリなしで実装している。

```typescript
React.useEffect(() => {
  return listenNavigation(() => fetchRscPayload());
}, []);
```

`listenNavigation` は以下の3つをインターセプトする:

1. **`popstate`** — ブラウザの戻る/進む
2. **`history.pushState` / `replaceState`** — モンキーパッチで呼び出し後にコールバックを実行
3. **`<a>` クリック** — 以下の条件を満たすリンクをインターセプトしてSPA遷移に切り替える

```typescript
// インターセプト条件（すべて満たす場合のみ）
link.origin ===
  location.origin(
    // 同一オリジン
    !link.target || link.target === "_self",
  ); // 別タブ指定なし
!link.hasAttribute("download"); // ダウンロードリンクでない
e.button === 0; // 左クリックのみ
!e.metaKey && !e.ctrlKey; // 新規タブ開き（Mac/Win）でない
!e.altKey; // ダウンロード操作でない
!e.shiftKey;
!e.defaultPrevented; // 他のハンドラが止めていない
```

`e.preventDefault()` した後に `history.pushState` を呼び出し、パッチ済みの `pushState` が `fetchRscPayload()` を起動する。

## サーバーアクションのコールバック

```typescript
setServerCallback(async (id, args) => {
  const temporaryReferences = createTemporaryReferenceSet();
  const renderRequest = createRscRenderRequest(window.location.href, {
    id,
    body: await encodeReply(args, { temporaryReferences }),
  });
  const payload = await createFromFetch<RscPayload>(fetch(renderRequest), {
    temporaryReferences,
  });
  setPayload(payload); // UIを新しいRSCペイロードで更新
  const { ok, data } = payload.returnValue!;
  if (!ok) throw data; // 失敗時は例外を再スロー
  return data; // 成功時はreturnValueをそのまま返す
});
```

`setServerCallback` はReact内部が "use server" 関数を呼び出すときに使うフックの登録。アクション実行後にRSCペイロードを取得して `setPayload` することで、mutation と再フェッチを**1往復**で完結させる。

`temporaryReferences` は `ReadableStream` や `Promise` などシリアライズできない値を引数として渡すための仕組み（サーバー送信時は参照として扱い、同一リクエスト内で復元する）。

## HMR

```typescript
if (import.meta.hot) {
  import.meta.hot.on("rsc:update", () => {
    fetchRscPayload();
  });
}
```

Viteのサーバー側ファイルが変更されると `rsc:update` イベントが飛んでくる。クライアントHMRと異なり、RSCはモジュールグラフを部分更新できないため、RSCペイロード全体を再取得して再レンダリングする。
