/**
 * Shared request-generation guard for chat view loaders.
 *
 * `loadMessages` (offset pagination) and `loadAroundDay` (day-window jump)
 * are two async paths that both assign the view's message list. Whichever
 * resolves LAST wins the render, so a slow day-window response can
 * overwrite a newer live query — or vice versa — when the user steps days
 * or toggles filters quickly. Every `begin()` bumps a shared token; each
 * path captures the token at start and passes it to `isCurrent()` before
 * assigning results, so only the most recently STARTED request lands.
 */
export function useChatRequestGuard() {
  let generation = 0

  /** Start a request: bumps the token, returns its generation number. */
  function begin(): number {
    generation += 1
    return generation
  }

  /** True when no newer request has started since `gen` began. */
  function isCurrent(gen: number): boolean {
    return gen === generation
  }

  return { begin, isCurrent }
}
