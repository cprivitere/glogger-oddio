export interface ParsedSearchQuery {
  text: string
  textWords: string[]
  /** Tokens mirroring the backend's FTS semantics: quoted phrases stay
   *  whole, a trailing `*` marks a prefix match. ChatHighlighted uses these
   *  (never the raw textWords) so `gorg*` and `"hello world"` highlight. */
  highlightTerms: string[]
  /** Raw tokens exactly as they appear in the query (e.g. `gorg*`,
   *  `"hello world"`). Filter chips render these directly and remove
   *  using them, so removing a chip strips the whole original token. */
  rawTokens: string[]
  /** Highlight terms carrying their backend FTS match kind so the
   *  highlighter can mirror FTS5 token semantics:
   *  - `exact`: the term must match a whole token (word boundary)
   *  - `phrase`: the phrase must appear as-is (substring; FTS5 phrases
   *    are contiguous token sequences — substring is the closest DOM
   *    approximation)
   *  - `prefix`: the term must start a token (FTS5 `term*`)
   *  - `literal`: malformed token — substring, mirrors the LIKE fallback */
  termKinds: { term: string, kind: 'exact' | 'phrase' | 'prefix' | 'literal' }[]
  sender?: string
  channel?: string
}

/**
 * Parses structured operators from a search query string.
 *
 * Supported operators:
 *   from:PlayerName    — filter by sender
 *   from:"Player Name" — quoted form for names with spaces
 *   in:Trade           — filter by channel
 *   in:"Global"        — quoted form
 *
 * Operators are case-insensitive. Remaining text after stripping
 * operators becomes the free-text search term for FTS.
 */
export function parseSearchQuery(raw: string): ParsedSearchQuery {
  let text = raw
  let sender: string | undefined
  let channel: string | undefined

  // Match operator:value or operator:"quoted value"
  // Case-insensitive operator names
  const operatorPattern = /\b(from|in):(?:"([^"]*)"|([\S]+))/gi

  let match: RegExpExecArray | null
  while ((match = operatorPattern.exec(text)) !== null) {
    const op = match[1].toLowerCase()
    const value = match[2] ?? match[3] // quoted or unquoted

    if (op === 'from') {
      sender = value
    } else if (op === 'in') {
      channel = value
    }
  }

  // Remove all matched operators from the text
  text = text.replace(operatorPattern, '').trim().replace(/\s+/g, ' ')

  const textWords = text ? text.split(/\s+/).filter(Boolean) : []

  // Derive highlight tokens with the same phrase/prefix rules as the
  // backend's build_fts_match_expr, INCLUDING its query-scope bail: the
  // backend returns None for the WHOLE query when any token is malformed,
  // and get_chat_messages then falls back to substring LIKE for every
  // word. In that state all words must highlight as `literal` — marking a
  // valid-looking word `exact`/`prefix` would leave it unhighlighted
  // even though the backend matched it as a substring (e.g. `hello
  // go*rg` LIKE-matches "hello" too).
  //
  // Backend bail rules (build_fts_match_expr):
  //   1. unterminated quote            (odd number of `"`)
  //   2. quote in the middle of a word (open + close inside one token)
  //   3. star in any non-trailing position (leading / internal /
  //      repeated: `*go`, `go*rg`, `gorg**`)
  //   4. bare `*` token
  //   5. FTS operator characters `()^:,+-` anywhere
  const tokens: { word: string, quoted: boolean, phrase: string }[] = []
  const tokenRe = /"([^"]*)"|(\S+)/g
  let tok: RegExpExecArray | null
  while ((tok = tokenRe.exec(text)) !== null) {
    if (tok[1] !== undefined) {
      tokens.push({ word: `"${tok[1]}"`, quoted: true, phrase: tok[1].trim() })
    } else {
      tokens.push({ word: tok[2], quoted: false, phrase: '' })
    }
  }

  const quoteCount = (text.match(/"/g) ?? []).length
  const anyInvalid =
    quoteCount % 2 === 1 ||           // (1) unterminated quote
    tokens.some(t => {
      if (t.quoted) return false
      const w = t.word
      // (2) quote mid-word: a word token containing a `"` (balanced, or
      // the parser above would have made it a phrase token)
      if (w.includes('"')) return true
      // (3)(4) star position checks
      if (!w.includes('*')) return false
      if (w === '*') return true      // (4) bare star
      return /\*\S/.test(w) || w.includes('**') // internal/repeated/leading
    }) ||
    /[()^:,+-]/.test(text) || // (5) FTS operator chars anywhere
    false

  const highlightTerms: string[] = []
  const termKinds: { term: string, kind: 'exact' | 'phrase' | 'prefix' | 'literal' }[] = []
  const rawTokens: string[] = []

  for (const t of tokens) {
    if (t.quoted) {
      rawTokens.push(t.word)
      if (t.phrase) {
        highlightTerms.push(t.phrase)
        termKinds.push({ term: t.phrase, kind: anyInvalid ? 'literal' : 'phrase' })
      }
    } else {
      rawTokens.push(t.word)
      const m = /^([^*]+)(\*)?$/.exec(t.word)
      const stem = m?.[1]?.trim() ?? ''
      if (stem) {
        const kind = anyInvalid ? 'literal' : (m && m[2]) ? 'prefix' : 'exact'
        highlightTerms.push(stem)
        termKinds.push({ term: stem, kind })
      } else {
        // Bare `*` or a token made only of stars: literal
        highlightTerms.push(t.word)
        termKinds.push({ term: t.word, kind: 'literal' })
      }
    }
  }

  return {
    text,
    textWords,
    highlightTerms,
    termKinds,
    rawTokens,
    ...(sender && { sender }),
    ...(channel && { channel }),
  }
}
