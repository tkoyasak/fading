# サーバーアクション解説

サーバーアクションは「JS有効時」と「JS無効時」の2つの実行パスを持つ。

## 定義（action.tsx）

```typescript
"use server";

let serverCounter = 0;

export async function getServerCounter() {
  return serverCounter;
}

export async function updateServerCounter(change: number) {
  serverCounter += change;
}
```

`"use server"` ディレクティブにより、`@vitejs/plugin-rsc` がこのモジュールをサーバー専用バンドルに分離し、各関数に一意のIDを割り当てる。クライアントからはIDとシリアライズされた引数でしか呼び出せない。

## 利用側（root.tsx）

```typescript
// Server Component なのでgetServerCounter() を直接await不要で呼べる
<form action={updateServerCounter.bind(null, 1)}>
  <button>Server Counter: {getServerCounter()}</button>
</form>
```

`updateServerCounter.bind(null, 1)` はビルド時にサーバーアクション参照（関数IDとバインド引数を含むオブジェクト）に変換される。`<form action={...}>` にこの参照を渡すことで、JSあり・なし両方の動作を実現する。

---

## JS有効時の実行パス

### ブラウザ側（entry.browser.tsx）

React が "use server" 関数を呼び出すとき、内部で `setServerCallback` に登録したコールバックを使う。

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
  setPayload(payload);
  const { ok, data } = payload.returnValue!;
  if (!ok) throw data;
  return data;
});
```

送信されるHTTPリクエスト:

```
POST /_.rsc
x-rsc-action: <関数ID>
Content-Type: application/octet-stream (or multipart/form-data)

<encodeReply でシリアライズされた引数>
```

### サーバー側（entry.rsc.tsx）

```typescript
// actionId がある場合（JS有効時）
const contentType = request.headers.get("content-type");
const body = contentType?.startsWith("multipart/form-data")
  ? await request.formData()
  : await request.text();
temporaryReferences = createTemporaryReferenceSet();
const args = await decodeReply(body, { temporaryReferences });
const action = await loadServerAction(renderRequest.actionId);
try {
  const data = await action.apply(null, args);
  returnValue = { ok: true, data };
} catch (e) {
  returnValue = { ok: false, data: e };
  actionStatus = 500;
}
```

アクション実行後、結果を `returnValue` に格納して `<Root />` を再レンダリングし、新しいRSCペイロードに含めて返す（mutation と refetch が1往復で完結）。

---

## JS無効時の実行パス（プログレッシブエンハンスメント）

`<form action={...}>` はJSがなくても通常のHTMLフォームとして機能する。

送信されるHTTPリクエスト:

```
POST /
Content-Type: multipart/form-data

$ACTION_ID=<関数ID>&$ACTION_KEY=<バインド引数>&...
```

### サーバー側（entry.rsc.tsx）

```typescript
// actionId がない場合（JS無効時 = フォームsubmit）
const formData = await request.formData();
const decodedAction = await decodeAction(formData); // formDataからアクションを復元
try {
  const result = await decodedAction();
  formState = await decodeFormState(result, formData);
} catch (e) {
  return new Response("Internal Server Error: server action failed", { status: 500 });
}
```

`decodeAction` がフォームの hidden field（`$ACTION_ID`, `$ACTION_KEY` など）からアクション関数とバインドされた引数を復元する。`decodeFormState` は `useFormState` / `useActionState` フック用の状態を計算する。

その後、SSRでHTMLを生成してレスポンスを返すため、ページ全体がリロードされる（通常のフォーム送信と同じ動作）。

---

## temporaryReferences とは

```typescript
const temporaryReferences = createTemporaryReferenceSet();
```

`ReadableStream` や `Promise`、クロージャなど、JSONシリアライズできない値を引数として渡すための仕組み。ブラウザ側で「参照」として登録し、`encodeReply` 時はIDに変換してサーバーへ送る。サーバー側の `decodeReply` は同じ `temporaryReferences` セットから元の値を復元する。同一リクエスト内でのみ有効な一時的な参照。
