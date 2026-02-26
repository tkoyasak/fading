"use server";

// no-op: RSC 再レンダリングをトリガーするだけ
// Root が再レンダリング時に新しいランダムエントリを選ぶ
export async function refresh(): Promise<void> {}
