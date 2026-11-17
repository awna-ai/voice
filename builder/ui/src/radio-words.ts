export function recoverySentence(path: string): string {
  return `Radio saved a recovery key for your computers in ${path}. Move it into your password manager, then delete that file.`
}

export function recoveryPath(message: string): string | null {
  const marker = '__RECOVERY_PATH__'
  const pattern = recoverySentence(marker)
    .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    .replace(marker, '(.+?)')
  return new RegExp(pattern).exec(message)?.[1] ?? null
}
