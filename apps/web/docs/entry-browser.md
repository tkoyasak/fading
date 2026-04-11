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

`setServerCallback` はReact内部が "use server" 関数を呼び出すときに使うフックの登録。

### リクエストの構造

`createRscRenderRequest`（`request.tsx`）が組み立てるリクエスト:

| 項目   | 値                                     |
| ------ | -------------------------------------- |
| URL    | `現在のURL + _.rsc` サフィックス       |
| Method | `POST`                                 |
| Header | `x-rsc-action: <id>`                   |
| Body   | `encodeReply` でシリアライズされた引数 |

`id` はビルド時に各 "use server" 関数へ割り当てられる識別子。URL に `_.rsc` を付けることでサーバー側（`parseRenderRequest`）が「RSC レスポンスを返す」と判断できる。

### mutation と再レンダリングを1リクエストで完結させる

サーバー側（`entry.rsc.tsx`）はアクション実行後、**そのままRSCツリーも一緒にレンダリングして返す**:

```typescript
const data = await action.apply(null, args);
returnValue = { ok: true, data };

// アクション結果と新しいUIを同一レスポンスに含める
const rscPayload: RscPayload = { root: <Root />, returnValue };
const rscStream = renderToReadableStream<RscPayload>(rscPayload, rscOptions);
```

クライアントは `createFromFetch` でこのストリームをデコードし、`setPayload` で再レンダリングする。サーバーへの fetch は1回、そのレスポンスに mutation 結果と新しいUIが両方含まれている。

### returnValue の処理

```typescript
const { ok, data } = payload.returnValue!;
if (!ok) throw data;
return data;
```

サーバー側で `try/catch` して `{ ok, data }` に包むことで、例外もシリアライズしてクライアントに届く。`!ok` のとき `throw` するので、クライアントから見ると通常の async 関数と同じように扱える。

### temporaryReferences

```typescript
const temporaryReferences = createTemporaryReferenceSet();
body: await encodeReply(args, { temporaryReferences })  // 送信側
createFromFetch(..., { temporaryReferences })            // 受信側
```

`ReadableStream` や `Promise` など**シリアライズ不可能な値**を引数として渡すための仕組み。`encodeReply` がローカル参照（`$T0` など）としてエンコードし、同じ `temporaryReferences` セットを `createFromFetch` に渡すことで、RSCストリームのデコード時に元の値を復元する。引数のない "use server" 関数では実質使われないが、`File` オブジェクトなどを渡すケースで必要になる。

## HMR

```typescript
if (import.meta.hot) {
  import.meta.hot.on("rsc:update", () => {
    fetchRscPayload();
  });
}
```

Viteのサーバー側ファイルが変更されると `rsc:update` イベントが飛んでくる。クライアントHMRと異なり、RSCはモジュールグラフを部分更新できないため、RSCペイロード全体を再取得して再レンダリングする。
