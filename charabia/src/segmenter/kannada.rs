use crate::segmenter::Segmenter;

/// Kannada specialized [`Segmenter`].
///
/// A Kannada word is written as a run of letters, signs and joiners, and words are separated by spaces and punctuation.
/// The zero width non-joiner (ZWNJ) is a default separator because Persian uses it between the parts of a compound,
/// but in Kannada it only controls how a consonant cluster is drawn, so it stays inside the word (ಕನ್‌ನಡ is 1 word).
/// The zero width joiner, the virama, the vowel signs and the nukta are not separators and stay inside the word as well.
pub struct KannadaSegmenter;

impl Segmenter for KannadaSegmenter {
    fn segment_str<'o>(&self, to_segment: &'o str) -> Box<dyn Iterator<Item = &'o str> + 'o> {
        Box::new(Some(to_segment).into_iter())
    }

    fn word_internal_separators(&self) -> &'static [&'static str] {
        &["\u{200C}"]
    }
}

#[cfg(test)]
mod test {
    use crate::segmenter::test::test_segmenter;

    // Original version of the text.
    const TEXT: &str = "ಕನ್\u{200C}ನಡ ಮತ್ತು ರಾಜ್\u{200D}ಕುಮಾರ್ (ಬೆಂಗಳೂರು) ಫ಼್ಯಾನ್, ಆಫ್\u{200C} ೧೨೩ 456";

    // Segmented version of the text.
    const SEGMENTED: &[&str] = &[
        "ಕನ್\u{200C}ನಡ",
        " ",
        "ಮತ್ತು",
        " ",
        "ರಾಜ್\u{200D}ಕುಮಾರ್",
        " ",
        "(",
        "ಬೆಂಗಳೂರು",
        ")",
        " ",
        "ಫ಼್ಯಾನ್",
        ", ",
        "ಆಫ್\u{200C}",
        " ",
        "೧೨೩",
        " ",
        "456",
    ];

    // Segmented and normalized version of the text.
    const TOKENIZED: &[&str] = &[
        "ಕನ್ನಡ",
        " ",
        "ಮತ್ತು",
        " ",
        "ರಾಜ್ಕುಮಾರ್",
        " ",
        "(",
        "ಬೆಂಗಳೂರು",
        ")",
        " ",
        "ಫ್ಯಾನ್",
        ", ",
        "ಆಫ್",
        " ",
        "೧೨೩",
        " ",
        "456",
    ];

    // Macro that run several tests on the Segmenter.
    test_segmenter!(KannadaSegmenter, TEXT, SEGMENTED, TOKENIZED, Script::Kannada, Language::Kan);

    #[test]
    fn variant_spellings_tokenize_like_the_clean_text() {
        use crate::Tokenize;

        let words = |text: &'static str| -> Vec<String> {
            text.tokenize().filter(|t| t.is_word()).map(|t| t.lemma().to_string()).collect()
        };

        let clean = "ಬೆಂಗಳೂರಿನಲ್ಲಿ ಕನ್ನಡ ಸಾಹಿತ್ಯ ಸಮ್ಮೇಳನದ ಸಂಬಂಧ ಸಭೆ ನಡೆಯಿತು. \
                     ರಾಜ್ಕುಮಾರ್ ಅವರ ಬಾಳು ಮರೆಯಲಾಗದು, ಅವರ ಜಮೀನು ಇಲ್ಲಿದೆ.";
        let variant = "ಬೆ೦ಗಳೂರಿನಲ್ಲಿ ಕನ್\u{200C}ನಡ ಸಾಹಿ\u{200B}ತ್ಯ ಸಮ್ಮೇಳನದ ಸಮ್ಬನ್ಧ ಸಭೆ ನಡೆಯಿತು. \
                       ರಾಜ್\u{200D}ಕುಮಾರ್ ಅವರ ಬಾೞು ಮಱೆಯಲಾಗದು, ಅವರ ಜ಼ಮೀನು ಇಲ್ಲಿದೆ.";

        assert_eq!(words(clean).len(), 14);
        assert_eq!(words(variant), words(clean));
    }
}
