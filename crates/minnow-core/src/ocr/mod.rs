pub mod service;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OcrBlock {
    pub text: String,
    pub cx: f64,
    pub cy: f64,
    pub width: f64,
    pub height: f64,
    pub angle: f64,
    pub percentage_coordinates: bool,
}

pub fn build_ocr_blocks(ocr_results: Vec<minnow_ocr::OcrResult>, img_w: f64, img_h: f64) -> Vec<OcrBlock> {
    ocr_results
        .into_iter()
        .map(|res| {
            let rect = crate::geometry::normalize_polygon(&res.box_points, img_w, img_h);
            OcrBlock {
                text: res.text,
                cx: rect.cx,
                cy: rect.cy,
                width: rect.width,
                height: rect.height,
                angle: rect.angle,
                percentage_coordinates: true,
            }
        })
        .collect()
}

pub fn is_cjk(c: char) -> bool {
    ('\u{3000}'..='\u{303f}').contains(&c)
        || ('\u{3040}'..='\u{309f}').contains(&c)
        || ('\u{30a0}'..='\u{30ff}').contains(&c)
        || ('\u{ff00}'..='\u{ff9f}').contains(&c)
        || ('\u{4e00}'..='\u{9faf}').contains(&c)
        || ('\u{3400}'..='\u{4dbf}').contains(&c)
}

pub fn format_selected_blocks(blocks: &[OcrBlock], indices: &[usize]) -> Option<String> {
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
