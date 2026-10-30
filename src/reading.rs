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
    fn rounds_up_to_whole_minutes() {
        assert_eq!(reading_minutes(0), 0);
        assert_eq!(reading_minutes(1), 1);
        assert_eq!(reading_minutes(200), 1);
        assert_eq!(reading_minutes(201), 2);
    }

    #[test]
    fn the_largest_count_does_not_overflow() {
        assert_eq!(reading_minutes(usize::MAX), usize::MAX / 200 + 1);
    }
}
