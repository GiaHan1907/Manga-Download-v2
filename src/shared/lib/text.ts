function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/đ/g, "d");
}

/// Diacritic-insensitive substring match for client-side list filtering.
export function matchesQuery(haystack: string, query: string): boolean {
  const needle = fold(query.trim());
  if (needle.length === 0) return true;
  return fold(haystack).includes(needle);
}
