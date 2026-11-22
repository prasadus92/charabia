use std::borrow::Cow;

use super::{Normalizer, NormalizerOption};
use crate::{Script, Token};

const VIRAMA: char = '\u{0CCD}';
const ANUSVARA: char = '\u{0C82}';
const DIGIT_ZERO: char = '\u{0CE6}';

/// Kannada specialized [`Normalizer`].
///
/// Kannada text often spells the same word in several ways that look the same or are read the same.
/// This Normalizer folds them into one spelling, so that a search for one spelling finds the others:
/// - removes the joiners and other invisible characters typed inside a word:
///   ZWNJ, ZWJ, zero width space, soft hyphen, word joiner and BOM (ಕನ್‌ನಡ becomes ಕನ್ನಡ),
/// - removes the nukta (ಜ಼ becomes ಜ),
/// - folds the Old Kannada letters into their modern letter: ೞ becomes ಳ, ಱ becomes ರ,
///   ೝ becomes ನ್, ೳ becomes ಂ and the archaic ಶ್ರೀ sign becomes ಶ್ರೀ,
/// - turns the Kannada digit zero `೦`, typed after a letter or a sign for the anusvara `ಂ`,
///   into the anusvara when no digit follows it (ಬೆ೦ಗಳೂರು becomes ಬೆಂಗಳೂರು, ಕೊಠಡಿ೦೧ stays),
/// - turns a nasal consonant with a virama before a consonant of the nasal's own group
///   into the anusvara (ಸಮ್ಬನ್ಧ becomes ಸಂಬಂಧ), and keeps other clusters (ಕನ್ನಡ stays).
///
/// The rules work on text decomposed by the [`CompatibilityDecompositionNormalizer`](super::CompatibilityDecompositionNormalizer).
pub struct KannadaNormalizer;

impl Normalizer for KannadaNormalizer {
    fn normalize<'o>(&self, mut token: Token<'o>, options: &NormalizerOption) -> Token<'o> {
        let chars: Vec<char> = token.lemma.chars().collect();
        let (lemma, output_lens) = fold(&chars);

        token.char_map = match token.char_map.take() {
            // the char_map maps each original char to a run of chars of the current lemma.
            Some(mut char_map) => {
                let mut output_lens = output_lens.into_iter();
                let mut lemma_chars = token.lemma.chars();
                for (_, normalized_len) in char_map.iter_mut() {
                    let mut consumed = 0;
                    let mut new_len = 0;
                    while consumed < *normalized_len as usize {
                        consumed += lemma_chars.next().map_or(0, char::len_utf8);
                        new_len += output_lens.next().unwrap_or(0);
                    }
                    *normalized_len = new_len;
                }
                Some(char_map)
            }
            None if options.create_char_map => Some(
                chars.iter().zip(output_lens).map(|(c, len)| (c.len_utf8() as u8, len)).collect(),
            ),
            None => None,
        };

        if lemma != token.lemma {
            token.lemma = Cow::Owned(lemma);
        }

        token
    }

    fn should_normalize(&self, token: &Token) -> bool {
        token.script == Script::Kannada && needs_folding(token.lemma())
    }
}

/// Returns the folded text, and the number of bytes each char of `chars` became in it.
fn fold(chars: &[char]) -> (String, Vec<u8>) {
    // First, the rules that rewrite 1 char without looking at its neighbors.
    // Each folded char remembers the index of the char it comes from.
    let mut folded: Vec<(char, usize)> = Vec::with_capacity(chars.len());
    for (index, &c) in chars.iter().enumerate() {
        match c {
            c if is_invisible(c) || c == '\u{0CBC}' => (),
            '\u{0CDE}' => folded.push(('\u{0CB3}', index)),
            '\u{0CB1}' => folded.push(('\u{0CB0}', index)),
            '\u{0CDD}' => folded.extend([('\u{0CA8}', index), (VIRAMA, index)]),
            '\u{0CF3}' => folded.push((ANUSVARA, index)),
            // ಶ್ರೀ, with ೀ decomposed as the compatibility decomposition does.
            '\u{0CDC}' => folded.extend(
                ['\u{0CB6}', VIRAMA, '\u{0CB0}', '\u{0CBF}', '\u{0CD5}'].map(|c| (c, index)),
            ),
            c => folded.push((c, index)),
        }
    }

    // Then, the rules that depend on the chars around.
    let mut lemma = String::with_capacity(folded.len() * 3);
    let mut output_lens = vec![0u8; chars.len()];
    let mut i = 0;
    while i < folded.len() {
        let (c, mut index) = folded[i];
        let previous = i.checked_sub(1).map(|p| folded[p].0);
        let next = folded.get(i + 1).map(|&(c, _)| c);
        let after_next = folded.get(i + 2).map(|&(c, _)| c);

        let c = match c {
            DIGIT_ZERO
                if previous.is_some_and(is_letter_or_sign) && !next.is_some_and(is_digit) =>
            {
                ANUSVARA
            }
            nasal
                if next == Some(VIRAMA)
                    && after_next.is_some_and(|stop| is_homorganic(nasal, stop)) =>
            {
                // the anusvara replaces the virama in the char_map, so a highlight covers both chars.
                i += 1;
                index = folded[i].1;
                ANUSVARA
            }
            c => c,
        };

        lemma.push(c);
        output_lens[index] += c.len_utf8() as u8;
        i += 1;
    }

    (lemma, output_lens)
}

fn needs_folding(lemma: &str) -> bool {
    let mut previous = None;
    lemma.chars().any(|c| {
        let found = match c {
            '\u{0CBC}' | '\u{0CDE}' | '\u{0CB1}' | '\u{0CDD}' | '\u{0CF3}' | '\u{0CDC}'
            | DIGIT_ZERO => true,
            VIRAMA => previous.is_some_and(is_nasal),
            c => is_invisible(c),
        };
        previous = Some(c);
        found
    })
}

/// Zero width non-joiner, zero width joiner, zero width space, soft hyphen, word joiner and BOM.
fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200C}' | '\u{200D}' | '\u{200B}' | '\u{00AD}' | '\u{2060}' | '\u{FEFF}')
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit() || ('\u{0CE6}'..='\u{0CEF}').contains(&c)
}

fn is_letter_or_sign(c: char) -> bool {
    ('\u{0C80}'..='\u{0CFF}').contains(&c) && !is_digit(c)
}

fn is_nasal(c: char) -> bool {
    matches!(c, 'ಙ' | 'ಞ' | 'ಣ' | 'ನ' | 'ಮ')
}

/// Returns `true` if `nasal` is the nasal of the consonant group of `stop`.
fn is_homorganic(nasal: char, stop: char) -> bool {
    match nasal {
        'ಙ' => ('ಕ'..='ಘ').contains(&stop),
        'ಞ' => ('ಚ'..='ಝ').contains(&stop),
        'ಣ' => ('ಟ'..='ಢ').contains(&stop),
        'ನ' => ('ತ'..='ಧ').contains(&stop),
        'ಮ' => ('ಪ'..='ಭ').contains(&stop),
        _ => false,
    }
}

#[cfg(test)]
mod test {
    use std::borrow::Cow::Owned;

    use crate::normalizer::test::test_normalizer;
    use crate::normalizer::{Normalizer, NormalizerOption};
    use crate::token::TokenKind;

    fn token(lemma: &str) -> Token<'static> {
        Token {
            lemma: Owned(lemma.to_string()),
            char_end: lemma.chars().count(),
            byte_end: lemma.len(),
            script: Script::Kannada,
            ..Default::default()
        }
    }

    // base tokens to normalize.
    fn tokens() -> Vec<Token<'static>> {
        vec![
            // ZWNJ inside the word.
            token("ಕನ್\u{200C}ನಡ"),
            // nasal and virama before a consonant of its group.
            token("ಸಮ್ಬನ್ಧ"),
        ]
    }

    // expected result of the current Normalizer.
    fn normalizer_result() -> Vec<Token<'static>> {
        vec![
            Token {
                lemma: Owned("ಕನ್ನಡ".to_string()),
                char_end: 6,
                byte_end: 18,
                script: Script::Kannada,
                char_map: Some(vec![(3, 3), (3, 3), (3, 3), (3, 0), (3, 3), (3, 3)]),
                ..Default::default()
            },
            Token {
                lemma: Owned("ಸಂಬಂಧ".to_string()),
                char_end: 7,
                byte_end: 21,
                script: Script::Kannada,
                char_map: Some(vec![(3, 3), (3, 0), (3, 3), (3, 3), (3, 0), (3, 3), (3, 3)]),
                ..Default::default()
            },
        ]
    }

    // expected result of the complete Normalizer pieline.
    fn normalized_tokens() -> Vec<Token<'static>> {
        normalizer_result()
            .into_iter()
            .map(|token| Token { kind: TokenKind::Word, ..token })
            .collect()
    }

    test_normalizer!(KannadaNormalizer, tokens(), normalizer_result(), normalized_tokens());

    fn normalize(lemma: &str) -> String {
        let options = NormalizerOption { lossy: true, ..Default::default() };
        let token = token(lemma);
        assert!(Normalizer::should_normalize(&KannadaNormalizer, &token), "{lemma}");
        KannadaNormalizer.normalize(token, &options).lemma.into_owned()
    }

    #[test]
    fn invisible_characters_are_removed() {
        for invisible in ['\u{200C}', '\u{200D}', '\u{200B}', '\u{00AD}', '\u{2060}', '\u{FEFF}'] {
            assert_eq!(normalize(&format!("ರಾಜ್{invisible}ಕುಮಾರ್")), "ರಾಜ್ಕುಮಾರ್");
        }
        assert_eq!(normalize("ಆಫ್\u{200C}"), "ಆಫ್");
    }

    #[test]
    fn nukta_is_removed() {
        assert_eq!(normalize("ಜ಼ಮೀನು"), "ಜಮೀನು");
        // the compatibility decomposition puts the nukta before the virama.
        assert_eq!(normalize("ಫ಼್ಯಾನ್"), "ಫ್ಯಾನ್");
        // and keeps it before a decomposed vowel sign.
        assert_eq!(normalize("ಜ಼\u{0CBF}\u{0CD5}ರೋ"), "ಜ\u{0CBF}\u{0CD5}ರೋ");
    }

    #[test]
    fn old_letters_are_folded() {
        assert_eq!(normalize("ಬಾೞು"), "ಬಾಳು");
        assert_eq!(normalize("ಮಱೆ"), "ಮರೆ");
        assert_eq!(normalize("ಅವೝ"), "ಅವನ್");
        assert_eq!(normalize("ಕ\u{0CF3}"), "ಕಂ");
        assert_eq!(normalize("\u{0CDC}ರಾಮ"), "ಶ್ರ\u{0CBF}\u{0CD5}ರಾಮ");
    }

    #[test]
    fn digit_zero_after_a_letter_is_the_anusvara() {
        assert_eq!(normalize("ಬೆ೦ಗಳೂರು"), "ಬೆಂಗಳೂರು");
        assert_eq!(normalize("ಕ೦ಪನಿ"), "ಕಂಪನಿ");
        assert_eq!(normalize("ಮ್ಯೂಸಿಯ೦"), "ಮ್ಯೂಸಿಯಂ");
        // a zero before a digit, or after a digit, is a number.
        assert_eq!(normalize("ಕೊಠಡಿ೦೧"), "ಕೊಠಡಿ೦೧");
        assert_eq!(normalize("ಕೊಠಡಿ೦1"), "ಕೊಠಡಿ೦1");
        assert_eq!(normalize("೧೦"), "೧೦");
        assert_eq!(normalize("೦ಕ"), "೦ಕ");
    }

    #[test]
    fn nasal_and_virama_before_its_group_is_the_anusvara() {
        for (nasal, group) in [
            ('ಙ', 'ಕ'..='ಘ'),
            ('ಞ', 'ಚ'..='ಝ'),
            ('ಣ', 'ಟ'..='ಢ'),
            ('ನ', 'ತ'..='ಧ'),
            ('ಮ', 'ಪ'..='ಭ'),
        ] {
            for stop in group {
                assert_eq!(normalize(&format!("ಅ{nasal}್{stop}")), format!("ಅಂ{stop}"));
            }
        }
        assert_eq!(normalize("ಸನ್ತೋಷ"), "ಸಂತೋಷ");
        // a joiner between the virama and the consonant does not change the word.
        assert_eq!(normalize("ಸನ್\u{200C}ತೋಷ"), "ಸಂತೋಷ");
        assert_eq!(normalize("ಅವೝತ"), "ಅವಂತ");
    }

    #[test]
    fn other_clusters_with_a_nasal_stay() {
        let options = NormalizerOption { lossy: true, ..Default::default() };
        for word in ["ಕನ್ನಡ", "ಅಮ್ಮ", "ಅಣ್ಣ", "ಜನ್ಮ", "ಬ್ರಹ್ಮ", "ತಾಮ್ರ", "ಅನ್ಯ", "ನ್ಕ", "ಮನ್"]
        {
            let token = KannadaNormalizer.normalize(token(word), &options);
            assert_eq!(token.lemma(), word);
        }
    }

    #[test]
    fn other_scripts_are_not_normalized() {
        let token = Token { script: Script::Latin, ..token("a\u{200C}b") };
        assert!(!Normalizer::should_normalize(&KannadaNormalizer, &token));
    }

    fn assert_char_map(lemma: &str, expected: &str, char_map: &[(u8, u8)]) {
        let options = NormalizerOption { create_char_map: true, lossy: true, ..Default::default() };
        let token = KannadaNormalizer.normalize(token(lemma), &options);
        assert_eq!(token.lemma(), expected, "{lemma}");
        assert_eq!(token.char_map.as_deref(), Some(char_map), "{lemma}");
    }

    #[test]
    fn char_map_follows_each_rule() {
        assert_char_map("ಜ಼ಮೀ", "ಜಮೀ", &[(3, 3), (3, 0), (3, 3), (3, 3)]);
        assert_char_map("ಬಾೞು", "ಬಾಳು", &[(3, 3), (3, 3), (3, 3), (3, 3)]);
        assert_char_map("ಅವೝ", "ಅವನ್", &[(3, 3), (3, 3), (3, 6)]);
        assert_char_map("ಅವೝತ", "ಅವಂತ", &[(3, 3), (3, 3), (3, 3), (3, 3)]);
        assert_char_map("\u{0CDC}", "ಶ್ರ\u{0CBF}\u{0CD5}", &[(3, 15)]);
        assert_char_map("ಬೆ೦ಗ", "ಬೆಂಗ", &[(3, 3), (3, 3), (3, 3), (3, 3)]);
        // the anusvara takes the place of the virama, so a highlight of ಸಂ covers ಸನ್.
        assert_char_map(
            "ಸ\u{00AD}ನ್\u{200D}ತ",
            "ಸಂತ",
            &[(3, 3), (2, 0), (3, 0), (3, 3), (3, 0), (3, 3)],
        );
    }

    #[test]
    fn existing_char_map_is_updated() {
        let options = NormalizerOption { create_char_map: true, lossy: true, ..Default::default() };
        // ೋ decomposed by the compatibility decomposition into 3 chars, then ನ್ತ folded.
        let token = Token {
            lemma: Owned("ಸನ್ತ\u{0CC6}\u{0CC2}\u{0CD5}ಷ".to_string()),
            char_map: Some(vec![(3, 3), (3, 3), (3, 3), (3, 3), (3, 9), (3, 3)]),
            ..token("ಸನ್ತೋಷ")
        };
        let token = KannadaNormalizer.normalize(token, &options);
        assert_eq!(token.lemma(), "ಸಂತ\u{0CC6}\u{0CC2}\u{0CD5}ಷ");
        assert_eq!(token.char_map, Some(vec![(3, 3), (3, 0), (3, 3), (3, 3), (3, 9), (3, 3)]));
        assert_eq!(token.original_lengths(6), (3, 9));
    }
}
