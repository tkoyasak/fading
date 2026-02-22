# RSC における認証

## 問題の構造

Server Component は直接 DB を叩けるため、**認証チェックをどこでやるかが設計の核心**になる。

```tsx
// 認証なしだと誰でもアクセスできてしまう
async function AdminPanel() {
  const data = await db.query("SELECT * FROM secrets");
  return <div>{data}</div>;
}
```

---

## 解決策：AsyncLocalStorage によるリクエストスコープ伝播

エントリーポイントでセッションを検証し、`AsyncLocalStorage` でレンダリングツリー全体に伝播させる。

```tsx
// framework/auth.ts
import { AsyncLocalStorage } from "node:async_hooks";

const authStorage = new AsyncLocalStorage<{ userId: string; role: string }>();

export function getUser() {
  const user = authStorage.getStore();
  if (!user) throw new Error("Unauthorized");
  return user;
}

export function withAuth<T>(user: { userId: string; role: string }, fn: () => T) {
  return authStorage.run(user, fn);
}
```

```tsx
// entry.rsc.tsx のハンドラー内
async function handler(request: Request) {
  const session = await verifySessionCookie(request);
  if (!session) return new Response("Unauthorized", { status: 401 });

  // ここから先のレンダリング全体で getUser() が使える
  return withAuth(session, () => renderToReadableStream({ root: <Root /> }));
}
```

`wrangler.jsonc` の `nodejs_als` フラグはこの `AsyncLocalStorage` を Cloudflare Workers で有効化するために必要。

---

## 推奨パターン：Data Access Layer (DAL)

認証チェックをコンポーネントに散らばらせず、**データアクセス関数に閉じ込める**。

```tsx
// lib/dal.ts
export async function getArticle(id: string) {
  const user = getUser(); // 未認証なら例外

  const article = await db.article.findUnique({ where: { id } });
  if (article.authorId !== user.userId && user.role !== "admin") {
    throw new Error("Forbidden");
  }
  return article;
}
```

```tsx
// コンポーネントはデータ関数を呼ぶだけ（認証ロジックを持たない）
async function ArticlePage({ id }: { id: string }) {
  const article = await getArticle(id);
  return <Article data={article} />;
}
```

---

## サーバーアクションでの認証

サーバーアクションは HTTP エンドポイントとして公開されるため、**必ず認証チェックが必要**。Client Component から呼ばれるからといって安全ではない。

```tsx
// action.tsx
"use server";

export async function deleteArticle(id: string) {
  const user = getUser(); // 未認証なら例外
  if (user.role !== "admin") throw new Error("Forbidden");

  await db.article.delete({ where: { id } });
}
```

---

## Next.js との比較

Next.js は上記の `AsyncLocalStorage` ラップを `cookies()` / `headers()` という API として提供している。構造は同じ。

```tsx
// Next.js では next/headers が AsyncLocalStorage を隠蔽している
import { cookies } from "next/headers";
import { cache } from "react";

// React.cache() でリクエスト内の重複呼び出しをメモ化（DB問い合わせが1回になる）
export const getUser = cache(async () => {
  const cookieStore = await cookies();
  const token = cookieStore.get("session")?.value;
  if (!token) return null;
  return verifyToken(token);
});
```

|                    | Next.js                                          | このスターター（apps/web）                 |
| ------------------ | ------------------------------------------------ | ------------------------------------------ |
| Request-scoped API | `cookies()` / `headers()` をフレームワークが提供 | `AsyncLocalStorage` を自前で設計           |
| セッション読み取り | どこからでも `cookies()` を呼ぶだけ              | `withAuth()` などのヘルパーを自前実装      |
| 認証のメモ化       | `React.cache()` + `getUser()` パターン           | 同様のパターンを自前実装                   |
| Middleware         | あり（ただし補助的）                             | なし（`entry.rsc.tsx` のハンドラーで代替） |

### DAL 推奨・Middleware 単独禁止の根拠

以下は Next.js 公式ドキュメント（[How to implement authentication in Next.js](https://nextjs.org/docs/app/building-your-application/authentication)）からの引用。

**DAL を推奨する記述：**

> "We recommend creating a DAL to centralize your data requests and authorization logic."

**Middleware（Proxy）を唯一の防衛線にしてはいけない記述：**

> "While Proxy can be useful for initial checks, it should not be your only line of defense in protecting your data. The majority of security checks should be performed as close as possible to your data source."

**Middleware では DB を叩かない理由：**

> "it's important to only read the session from the cookie (optimistic checks), and avoid database checks to prevent performance issues."

### CVE-2025-29927（2025年3月発覚）

Next.js の `x-middleware-subrequest` ヘッダーを操作することで Middleware の認証チェックを**完全にバイパスできる**脆弱性。Next.js 15.2.3+ / 14.2.25+ / 13.5.9+ で修正済み。

この CVE は「Middleware だけに認証を依存させてはいけない」という方針が**CVE 以前から公式ドキュメントに明記されていた**ことを改めて浮き彫りにした事例。

---

## まとめ

| 懸念                                     | 対策                                                                         |
| ---------------------------------------- | ---------------------------------------------------------------------------- |
| 誰でも Server Component が実行できる     | エントリーポイントでセッション検証し `AsyncLocalStorage` で伝播              |
| コンポーネントごとに認証チェックを忘れる | DAL にチェックを閉じ込め、コンポーネントはデータ関数経由でのみ DB にアクセス |
| サーバーアクションが直接叩かれる         | アクション関数内で必ず `getUser()` を呼ぶ                                    |
| `AsyncLocalStorage` が未設定の場合       | `getUser()` が例外を投げるようにする（デフォルト安全）                       |
