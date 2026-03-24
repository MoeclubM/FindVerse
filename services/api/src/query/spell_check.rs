pub fn suggest_correction(query: &str) -> Option<String> {
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    // 常见拼写错误映射
    let corrections = [
        ("serach", "search"),
        ("seach", "search"),
        ("serch", "search"),
        ("documnet", "document"),
        ("docuemnt", "document"),
        ("teh", "the"),
        ("taht", "that"),
        ("wiht", "with"),
        ("thier", "their"),
        ("recieve", "receive"),
        ("occured", "occurred"),
        ("seperate", "separate"),
        ("definately", "definitely"),
        ("goverment", "government"),
        ("enviroment", "environment"),
    ];

    let mut corrected = Vec::new();
    let mut has_correction = false;

    for word in words {
        let lower = word.to_lowercase();
        if let Some(&(_, correct)) = corrections.iter().find(|(wrong, _)| *wrong == lower) {
            corrected.push(correct.to_string());
            has_correction = true;
        } else {
            corrected.push(word.to_string());
        }
    }

    if has_correction {
        Some(corrected.join(" "))
    } else {
        None
    }
}
