use icu_segmenter::{WordSegmenter, options::WordBreakInvariantOptions};
use result::ArticlerResult;
use types::ReadingTime;

const AVERAGE_READING_SPEED: i32 = 230;

pub fn reading_time(text: &str) -> ArticlerResult<ReadingTime> {
    Ok(i32::try_from(count_words(text))? / AVERAGE_READING_SPEED)
}

fn count_words(text: &str) -> usize {
    let segmenter = WordSegmenter::new_auto(WordBreakInvariantOptions::default());

    segmenter
        .segment_str(text)
        .iter_with_word_type()
        .filter(|(_, word_type)| word_type.is_word_like())
        .count()
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_count_words_english() {
        let text = "Hello world. This is a test sentence.";
        assert_eq!(7, super::count_words(text));
    }

    #[test]
    fn test_count_words_german() {
        let text = "Das ist ein Testartikel. Er enthält mehrere Sätze.";
        assert_eq!(8, super::count_words(text));
    }

    #[test]
    fn test_count_words_russian() {
        let text = "Это тестовая статья. Она содержит несколько предложений.";
        assert_eq!(7, super::count_words(text));
    }

    #[test]
    fn test_count_words_chinese() {
        let text = "这是一篇测试文章。它包含多个句子。";
        assert_eq!(7, super::count_words(text));
    }

    #[test]
    fn test_count_words_korean() {
        let text = "이것은 테스트 기사입니다. 여러 문장이 포함되어 있습니다.";
        assert_eq!(7, super::count_words(text));
    }
}
