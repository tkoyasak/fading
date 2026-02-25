# React Server Components (RSC) 概要

RSC の本質は「サーバーでしか持てないもの（DB アクセス・秘密情報・重いライブラリ）を、クライアント JS バンドルに含めずに UI を組み立てられる」点にある。

---

## RSC が真価を発揮するケース

### 1. データ取得がコンポーネントツリー内で完結する

従来のアーキテクチャでは「ページレベルでまとめて fetch → props drilling」が必要だった。RSC ではコンポーネントが直接 DB や API を叩ける。

```tsx
async function ArticleList() {
  const articles = await db.query("SELECT * FROM articles");
  return (
    <ul>
      {articles.map((a) => (
        <ArticleItem key={a.id} article={a} />
      ))}
    </ul>
  );
}
```

ウォーターフォールも Suspense で並列化できる。コンポーネントの凝集度が上がるのが最大の恩恵。

### 2. クライアントに送りたくないロジック・秘密情報がある

- DB の接続文字列、API キー
- 内部ビジネスロジック（競合に見られたくない計算など）
- ユーザーに見せたくないデータ（他ユーザーの情報など）

Server Component は**バンドルに含まれない**ので、逆コンパイルのリスクがゼロ。

### 3. 重いライブラリをクライアントに送りたくない

```tsx
import { marked } from "marked"; // クライアントバンドルに含まれない

async function MarkdownRenderer({ content }: { content: string }) {
  return <div dangerouslySetInnerHTML={{ __html: marked(content) }} />;
}
```

数百 KB のライブラリがクライアント JS からゼロになる。

### 4. コンテンツが動的だが操作は不要な UI

ブログ・ドキュメント・ダッシュボードの読み取り部分など。インタラクションが不要な部分は Client Component にする必要がなく、ハイドレーションコストがかからない。

### 5. 細かい粒度での認証・認可

```tsx
async function AdminPanel() {
  const user = await getUser();
  if (user.role !== "admin") return <Forbidden />;
  return <SensitiveData />;
}
```

ページ単位ではなくコンポーネント単位でアクセス制御ができる。認可漏れが起きにくい。

---

## RSC が向いていないケース

| ケース                                   | 理由                                                                      |
| ---------------------------------------- | ------------------------------------------------------------------------- |
| リアルタイム更新（チャット・通知）       | RSC は基本的にリクエストドリブン。WebSocket や SSE はクライアント側で処理 |
| ユーザーインタラクションが多い UI        | `useState` / `useEffect` が使えないので Client Component が必要           |
| 純粋な SPA（認証後のダッシュボードのみ） | 初期ロード後はサーバーと通信しないなら RSC の恩恵は薄い                   |
| エッジへのデプロイが困難な環境           | RSC はサーバー常駐が前提。静的ホスティングのみでは使えない                |

---

## `apps/web/` での実用構成例

```
Root (Server)
├── Header (Server)         ← セッション確認・ナビゲーション生成
├── ArticleFeed (Server)    ← DB から記事一覧取得（markdown パース済み）
│   └── LikeButton (Client) ← インタラクションだけ Client に
└── Sidebar (Server)        ← おすすめ記事を DB から取得
```

「データと UI の境界」がコンポーネント単位でコントロールできる点が、Next.js Pages Router や SPA にはない構造的優位性。

---

## Server Component と Client Component の使い分け

|                                    | Server Component   | Client Component |
| ---------------------------------- | ------------------ | ---------------- |
| ディレクティブ                     | なし（デフォルト） | `"use client"`   |
| DB・ファイルシステムアクセス       | ○                  | ✗                |
| `useState` / `useEffect`           | ✗                  | ○                |
| イベントハンドラー                 | ✗                  | ○                |
| クライアント JS バンドルに含まれる | ✗                  | ○                |
| ハイドレーションが必要             | ✗                  | ○                |

`apps/web/` での例:

- `root.tsx` — Server Component（DB アクセス・レイアウト）
- `client.tsx` — `"use client"` （`useState` を使うカウンター）
- `action.tsx` — `"use server"` （サーバーアクション）

詳細なアーキテクチャは [architecture.md](./architecture.md) を参照。
