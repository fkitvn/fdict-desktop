// Chap nhan ca chu Latin co dau (tieng Phap): a-z, à-ÿ, œ.
function partsOf(word) {
  return String(word || "")
    .trim()
    .toLowerCase()
    .split(/[^a-zà-ÿœ']+/)
    .filter(Boolean);
}

function tokenHit(token, part) {
  if (token === part) return true;
  if (part.length < 3 || token.length <= part.length || !token.startsWith(part)) return false;
  const rest = token.slice(part.length);
  return rest === "s" || rest === "es" || rest === "ed" || rest === "ing" || rest === "d";
}

function relatedWords(sentence, words) {
  const toks = String(sentence || "")
    .toLowerCase()
    .split(/[^a-zà-ÿœ']+/)
    .filter(Boolean);
  const found = [];
  const seen = new Set();
  for (const word of words || []) {
    const label = String(word || "").trim();
    const parts = partsOf(label);
    const key = parts.join(" ");
    if (!key || seen.has(key)) continue;
    for (let i = 0; i + parts.length <= toks.length; i += 1) {
      if (!parts.every((part, j) => tokenHit(toks[i + j], part))) continue;
      seen.add(key);
      found.push(label);
      break;
    }
  }
  return found;
}

/** Cat cau thanh doan. on = true la tu Gemini nhan ra trong vocab. */
export function markSentence(sentence, words) {
  const text = String(sentence || "");
  const terms = relatedWords(text, words)
    .map(partsOf)
    .filter((parts) => parts.length)
    .sort((a, b) => b.length - a.length || b.join(" ").length - a.join(" ").length);
  const tokens = text.match(/[A-Za-zÀ-ÿŒœ']+|[^A-Za-zÀ-ÿŒœ']+/g) || [];
  const wordAt = [];
  tokens.forEach((token, index) => {
    if (/^[A-Za-zÀ-ÿŒœ']+$/.test(token)) wordAt.push(index);
  });
  const lower = wordAt.map((index) => tokens[index].toLowerCase());
  const on = new Set();
  const used = new Array(lower.length).fill(false);
  for (let i = 0; i < lower.length; i += 1) {
    if (used[i]) continue;
    for (const parts of terms) {
      if (i + parts.length > lower.length) continue;
      if (!parts.every((part, j) => !used[i + j] && tokenHit(lower[i + j], part))) continue;
      for (let j = 0; j < parts.length; j += 1) {
        used[i + j] = true;
        on.add(wordAt[i + j]);
      }
      break;
    }
  }
  const marks = [];
  for (let i = 0; i < tokens.length; i += 1) {
    const hit = on.has(i);
    const prev = marks[marks.length - 1];
    if (prev && prev.on === hit) prev.text += tokens[i];
    else marks.push({ text: tokens[i], on: hit });
  }
  return marks;
}
