// Turns a word count into the minutes shown under an article's title.

const WORDS_PER_MINUTE: usize = 200;

pub fn reading_minutes(words: usize) -> usize {
    let full_minutes = words / WORDS_PER_MINUTE;
    let leftover_words = words % WORDS_PER_MINUTE;
    if leftover_words > 0 {
        full_minutes + 1
    } else {
        full_minutes
    }
}

#[cfg(test)]
mod tests {
    use super::reading_minutes;

    #[test]
    fn no_words_take_no_time() {
        assert_eq!(reading_minutes(0), 0);
    }

    #[test]
    fn one_word_takes_one_minute() {
        assert_eq!(reading_minutes(1), 1);
    }

    #[test]
    fn two_hundred_words_take_one_minute() {
        assert_eq!(reading_minutes(200), 1);
    }

    #[test]
    fn two_hundred_and_one_words_take_two_minutes() {
        assert_eq!(reading_minutes(201), 2);
    }

    #[test]
    fn the_largest_count_does_not_overflow() {
        assert_eq!(reading_minutes(usize::MAX), usize::MAX / 200 + 1);
    }
}
