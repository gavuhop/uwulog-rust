use super::ansi::parse_ansi_segments_with_palette;
use super::theme::Theme;
use eframe::egui::{self, Color32, FontId};
use std::collections::HashSet;

/// Tạo LayoutJob hiển thị đoạn text với hỗ trợ ANSI và highlight các từ khóa tìm kiếm theo Theme
pub fn create_highlighted_layout_job_with_theme(
    text: &str,
    default_color: Color32,
    font_id: FontId,
    highlighted_terms: &HashSet<String>,
    theme: &Theme,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    if text.is_empty() {
        return job;
    }

    // Fast-path 0 heap allocation cho log thuần túy không có ANSI và không có Highlight
    if highlighted_terms.is_empty() && !text.contains('\x1b') {
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id,
                color: default_color,
                ..Default::default()
            },
        );
        return job;
    }

    let segments = parse_ansi_segments_with_palette(text, default_color, &theme.ansi);

    for (seg_text, seg_color) in segments {
        if highlighted_terms.is_empty() || seg_text.is_empty() {
            job.append(
                &seg_text,
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: seg_color,
                    ..Default::default()
                },
            );
            continue;
        }

        let lower_text = seg_text.to_lowercase();
        let mut intervals: Vec<(usize, usize)> = Vec::new();

        for term in highlighted_terms {
            let term_clean = term.trim().to_lowercase();
            if term_clean.is_empty() {
                continue;
            }
            for (pos, _) in lower_text.match_indices(&term_clean) {
                let actual_end = pos + term_clean.len();
                intervals.push((pos, actual_end));
            }
        }

        if intervals.is_empty() {
            job.append(
                &seg_text,
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: seg_color,
                    ..Default::default()
                },
            );
            continue;
        }

        // Gộp các khoảng trùng nhau
        intervals.sort_by_key(|(s, _)| *s);
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (s, e) in intervals {
            if let Some(last) = merged.last_mut() {
                if s <= last.1 {
                    last.1 = last.1.max(e);
                } else {
                    merged.push((s, e));
                }
            } else {
                merged.push((s, e));
            }
        }

        let mut cur = 0;
        for (s, e) in merged {
            if s > cur {
                if let Some(slice) = seg_text.get(cur..s) {
                    job.append(
                        slice,
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color: seg_color,
                            ..Default::default()
                        },
                    );
                }
            }
            if let Some(slice) = seg_text.get(s..e) {
                job.append(
                    slice,
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: theme.log.term_highlight_text,
                        background: theme.log.term_highlight_bg,
                        ..Default::default()
                    },
                );
            }
            cur = e;
        }

        if cur < seg_text.len() {
            if let Some(slice) = seg_text.get(cur..) {
                job.append(
                    slice,
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: seg_color,
                        ..Default::default()
                    },
                );
            }
        }
    }

    job
}

/// Tạo LayoutJob sử dụng Theme đang hoạt động
pub fn create_highlighted_layout_job(
    text: &str,
    default_color: Color32,
    font_id: FontId,
    highlighted_terms: &HashSet<String>,
) -> egui::text::LayoutJob {
    let theme = crate::theme::active();
    create_highlighted_layout_job_with_theme(
        text,
        default_color,
        font_id,
        highlighted_terms,
        &theme,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::presets;

    #[test]
    fn test_highlighted_layout_job() {
        let theme = presets::nord_dimmed();
        let mut terms = HashSet::new();
        terms.insert("warn".to_string());
        terms.insert("error".to_string());

        let job = create_highlighted_layout_job_with_theme(
            "this is a warning and ERROR message",
            theme.text.primary,
            FontId::monospace(11.5),
            &terms,
            &theme,
        );

        assert_eq!(job.text, "this is a warning and ERROR message");
        assert!(job.sections.len() > 1);
    }
}
