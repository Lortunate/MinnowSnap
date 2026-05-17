use crate::services::ocr::OcrBlock;

fn is_cjk(c: char) -> bool {
    ('\u{3000}'..='\u{303f}').contains(&c)
        || ('\u{3040}'..='\u{309f}').contains(&c)
        || ('\u{30a0}'..='\u{30ff}').contains(&c)
        || ('\u{ff00}'..='\u{ff9f}').contains(&c)
        || ('\u{4e00}'..='\u{9faf}').contains(&c)
        || ('\u{3400}'..='\u{4dbf}').contains(&c)
}

pub(super) fn format_selected_blocks(blocks: &[OcrBlock], indices: &[usize]) -> Option<String> {
    if indices.is_empty() || blocks.is_empty() {
        return None;
    }

    let mut selected_blocks: Vec<OcrBlock> = indices.iter().filter_map(|&i| blocks.get(i).cloned()).collect();

    selected_blocks.sort_by(|a, b| {
        if (a.cy - b.cy).abs() < 0.01 {
            a.cx.partial_cmp(&b.cx).unwrap()
        } else {
            a.cy.partial_cmp(&b.cy).unwrap()
        }
    });

    let mut result = String::new();
    let mut prev_block: Option<OcrBlock> = None;

    for curr_block in selected_blocks {
        if let Some(prev) = prev_block {
            let prev_bottom = prev.cy + prev.height / 2.0;
            let curr_top = curr_block.cy - curr_block.height / 2.0;
            let gap = curr_top - prev_bottom;
            let avg_height = (prev.height + curr_block.height) / 2.0;

            let is_list_item = curr_block
                .text
                .trim_start()
                .starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '•' || c == '*');

            if prev.text.ends_with('-') {
                result.pop();
                result.push_str(&curr_block.text);
            } else if gap > avg_height * 0.5 || is_list_item {
                result.push('\n');
                result.push_str(&curr_block.text);
            } else {
                let last_char = prev.text.chars().last().unwrap_or(' ');
                let first_char = curr_block.text.chars().next().unwrap_or(' ');
                if is_cjk(last_char) && is_cjk(first_char) {
                    result.push_str(&curr_block.text);
                } else {
                    result.push(' ');
                    result.push_str(&curr_block.text);
                }
            }
        } else {
            result.push_str(&curr_block.text);
        }
        prev_block = Some(curr_block);
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(text: &str, cx: f64, cy: f64) -> OcrBlock {
        OcrBlock {
            text: text.to_string(),
            cx,
            cy,
            width: 0.1,
            height: 0.1,
            angle: 0.0,
            percentage_coordinates: true,
        }
    }

    #[test]
    fn selected_blocks_are_sorted_into_reading_order() {
        let blocks = vec![block("world", 0.5, 0.1), block("hello", 0.1, 0.1)];

        assert_eq!(format_selected_blocks(&blocks, &[0, 1]).as_deref(), Some("hello world"));
    }

    #[test]
    fn selected_blocks_keep_cjk_text_together() {
        let blocks = vec![block("你", 0.1, 0.1), block("好", 0.2, 0.1)];

        assert_eq!(format_selected_blocks(&blocks, &[0, 1]).as_deref(), Some("你好"));
    }
}
