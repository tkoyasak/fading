# R2 Bundle Encryption

`FADING_CLI_ENCRYPTION_KEY` が設定されている場合、R2 にアップロードする git bundle を AES-256-GCM で暗号化する。

## AES（Advanced Encryption Standard）

ブロック暗号。データを 128 ビット（16 バイト）のブロック単位で処理する。

```
平文ブロック (16 B) + キー → 暗号文ブロック (16 B)
```

キー長は 128 / 192 / 256 ビットの 3 種類。ここでは **AES-256**（256 ビット = 32 バイト キー）を使用。

## GCM（Galois/Counter Mode）

AES 単体はブロック単位しか扱えないため、実際の使用にはモードが必要。GCM は 2 つの役割を持つ。

1. **CTR モード** — AES でカウンタを暗号化してキーストリームを生成し、平文と XOR して任意長を暗号化する
2. **GHASH** — 暗号文の改ざん検知タグ（16 バイト）を生成する

```
nonce (12 B) + counter → AES → keystream ⊕ 平文 → 暗号文
暗号文 → GHASH → tag (16 B)
```

AES-GCM = 暗号化 + 認証（AEAD: Authenticated Encryption with Associated Data）。
復号時に tag が一致しなければエラーになる。

## バイナリフォーマット

```
[nonce (12 B)][ciphertext + tag (16 B)]
```

- **nonce**: `OsRng` でアップロードごとにランダム生成
- **ciphertext**: git bundle の暗号文
- **tag**: 改ざん検知用（ciphertext 末尾に付加）

## キー生成

```bash
openssl rand -hex 32   # → 64 文字の 16 進数文字列
```

生成した値を `FADING_CLI_ENCRYPTION_KEY` に設定する。
