//! Literal evidence in the original text, independent of the embedding score.
use eframe::egui::{Color32, FontId, Stroke, TextFormat, text::LayoutJob};
use std::{collections::HashSet, ops::Range};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

pub struct QueryTerms(HashSet<String>);

fn normalized(text: &str) -> String {
    text.nfc()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            _ => c, // Keep ñ distinct from n.
        })
        .collect()
}

fn words(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (offset, c) in text.char_indices() {
        if c.is_alphanumeric() || is_combining_mark(c) || c == '-' || c == '_' {
            start.get_or_insert(offset);
        } else if let Some(begin) = start.take() {
            ranges.push(begin..offset);
        }
    }
    if let Some(begin) = start {
        ranges.push(begin..text.len());
    }
    ranges
}

impl QueryTerms {
    pub fn new(query: &str) -> Self {
        let all: HashSet<_> = words(query)
            .into_iter()
            .map(|range| normalized(&query[range]))
            .filter(|word| word.chars().any(char::is_alphanumeric))
            .collect();
        let terms: HashSet<_> = all
            .iter()
            .filter(|word| {
                !matches!(
                    word.as_str(),
                    "a" | "al"
                        | "de"
                        | "del"
                        | "el"
                        | "la"
                        | "las"
                        | "lo"
                        | "los"
                        | "en"
                        | "un"
                        | "una"
                        | "unos"
                        | "unas"
                        | "y"
                        | "o"
                        | "que"
                        | "con"
                        | "por"
                        | "para"
                        | "mi"
                        | "mis"
                        | "su"
                        | "sus"
                        | "se"
                        | "es"
                        | "como"
                        | "the"
                        | "an"
                        | "of"
                        | "to"
                        | "in"
                        | "and"
                        | "or"
                        | "with"
                        | "for"
                )
            })
            .cloned()
            .collect();
        // A deliberately short query such as "de" can still be highlighted.
        Self(if terms.is_empty() { all } else { terms })
    }

    pub fn ranges(&self, text: &str) -> Vec<Range<usize>> {
        if self.0.is_empty() {
            return Vec::new();
        }
        words(text)
            .into_iter()
            .filter(|range| self.0.contains(&normalized(&text[range.clone()])))
            .collect()
    }

    pub fn fragment_fallback(&self, text: &str, matched: bool) -> bool {
        matched && self.ranges(text).is_empty()
    }

    pub fn layout(&self, text: &str, font_id: FontId, color: Color32) -> LayoutJob {
        let base = TextFormat {
            font_id,
            color,
            ..Default::default()
        };
        let underlined = TextFormat {
            underline: Stroke::new(1.5, color),
            ..base.clone()
        };
        let mut job = LayoutJob::default();
        let mut cursor = 0;
        for range in self.ranges(text) {
            job.append(&text[cursor..range.start], 0.0, base.clone());
            job.append(&text[range.clone()], 0.0, underlined.clone());
            cursor = range.end;
        }
        job.append(&text[cursor..], 0.0, base);
        job
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches<'a>(query: &str, text: &'a str) -> Vec<&'a str> {
        QueryTerms::new(query)
            .ranges(text)
            .into_iter()
            .map(|range| &text[range])
            .collect()
    }

    #[test]
    fn preserves_spanish_unicode_and_original_byte_ranges() {
        let text = "🦅 ÁGUILAS, a\u{301}guilas y águilas de Mexicali; año, ano.";
        assert_eq!(
            matches("aguilas de mexicali", text),
            ["ÁGUILAS", "a\u{301}guilas", "águilas", "Mexicali"]
        );
        assert_eq!(matches("AÑO", text), ["año"]);
        assert_eq!(matches("ano", text), ["ano"]);
        assert_eq!(matches("nin\u{303}o", "niño, nino"), ["niño"]);
    }

    #[test]
    fn matches_whole_words_and_identifiers_without_fabricating_evidence() {
        assert_eq!(
            matches("contrato", "contrato, contratos y contratación"),
            ["contrato"]
        );
        assert_eq!(matches("F-1042", "F-10420, F-1042 y AF-1042"), ["F-1042"]);
        assert!(matches("reembolso de compra", "Política de devoluciones").is_empty());
        assert_eq!(matches("de", "Política de devoluciones"), ["de"]);
        assert!(matches("", "Texto sin consulta").is_empty());
    }

    #[test]
    fn underlines_only_evidence_and_keeps_semantic_fragment_fallback() {
        let query = QueryTerms::new("comisiones");
        let text = "Las COMISIONES se pagan cada semana.";
        let job = query.layout(text, FontId::proportional(19.0), Color32::BLACK);
        assert_eq!(job.text, text);
        let marked: Vec<_> = job
            .sections
            .iter()
            .filter(|s| s.format.underline.width > 0.0)
            .map(|s| &job.text[s.byte_range.clone()])
            .collect();
        assert_eq!(marked, ["COMISIONES"]);
        assert!(!query.fragment_fallback(text, true));
        assert!(query.fragment_fallback("La remuneración es semanal.", true));
        assert!(!query.fragment_fallback("La remuneración es semanal.", false));
    }
}
