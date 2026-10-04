export interface ParsedSearchQuery {
  text: string
  textWords: string[]
  /** Tokens mirroring the backend's FTS semantics: quoted phrases stay
   *  whole, a trailing `*` marks a prefix match. ChatHighlighted uses these
   *  (never the raw textWords) so `gorg*` and `"hello world"` highlight. */
  highlightTerms: string[]
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
  // backend's build_fts_match_expr: `"exact phrase"` stays whole (minus
  // quotes), a bare word ending in exactly one `*` matches as a prefix,
  // everything else is a literal word. Tokens the backend would reject
  // (misplaced `*`, unbalanced quotes, operators) are still highlighted as
  // plain words so the LIKE-fallback results get marked too.
  const highlightTerms: string[] = []
  const tokenRe = /"([^"]*)"|(\S+)/g
  let tok: RegExpExecArray | null
  while ((tok = tokenRe.exec(text)) !== null) {
    if (tok[1] !== undefined) {
      const phrase = tok[1].trim()
      if (phrase) highlightTerms.push(phrase)
    } else {
      const word = tok[2]
      const m = /^([^*]+)\*?$/.exec(word)
      if (m) {
        const stem = m[1].trim()
        if (stem) highlightTerms.push(stem)
      } else {
        // Star(s) in unsupported positions: highlight the literal token
        highlightTerms.push(word)
      }
    }
  }

  return {
    text,
    textWords,
    highlightTerms,
    ...(sender && { sender }),
    ...(channel && { channel }),
  }
}
