//! Font and script runs for mixed text that needs the bundled fallback face.

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};
use unicode_script::{Script, UnicodeScript};

use super::ParsedFont;

pub(super) struct FontRun<'a> {
    pub(super) text: &'a str,
    pub(super) font: &'a ParsedFont,
}

/// Retains combining sequences while selecting a face and a shaping script.
pub(super) fn split<'a>(
    text: &'a str,
    primary: &'a ParsedFont,
    fallback: &'a ParsedFont,
) -> Vec<FontRun<'a>> {
    let mut clusters = clusters(text);
    inherit_scripts(&mut clusters);
    let mut runs: Vec<RunRange<'a>> = Vec::new();

    for cluster in clusters {
        let cluster_text = &text[cluster.start..cluster.end];
        let previous = runs.last().map(|run| run.font);
        let font = select_font(cluster_text, cluster.common, previous, primary, fallback);
        append_run(&mut runs, &cluster, font);
    }

    runs.into_iter()
        .map(|run| FontRun {
            text: &text[run.start..run.end],
            font: run.font,
        })
        .collect()
}

struct Cluster {
    start: usize,
    end: usize,
    script: Option<Script>,
    common: bool,
}

struct RunRange<'a> {
    start: usize,
    end: usize,
    script: Option<Script>,
    font: &'a ParsedFont,
}

fn clusters(text: &str) -> Vec<Cluster> {
    let mut clusters: Vec<Cluster> = Vec::new();
    let mut join_next = false;

    for (start, character) in text.char_indices() {
        let end = start + character.len_utf8();
        if (join_next || is_extension(character)) && !clusters.is_empty() {
            if let Some(cluster) = clusters.last_mut() {
                cluster.end = end;
                cluster.script = cluster.script.or(strong_script(character));
                cluster.common = cluster.script.is_none();
            }
        } else {
            let script = strong_script(character);
            clusters.push(Cluster {
                start,
                end,
                script,
                common: script.is_none(),
            });
        }
        join_next = is_join_control(character) || (join_next && is_extension(character));
    }

    clusters
}

fn strong_script(character: char) -> Option<Script> {
    match character.script() {
        Script::Common | Script::Inherited | Script::Unknown => None,
        script => Some(script),
    }
}

fn is_extension(character: char) -> bool {
    character.general_category_group() == GeneralCategoryGroup::Mark
        || is_format_extension(character)
}

fn is_format_extension(character: char) -> bool {
    matches!(character, '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
        || is_join_control(character)
}

fn is_join_control(character: char) -> bool {
    matches!(character, '\u{200c}' | '\u{200d}')
}

fn inherit_scripts(clusters: &mut [Cluster]) {
    let mut previous = None;
    for cluster in clusters.iter_mut() {
        cluster.script = cluster.script.or(previous);
        previous = cluster.script;
    }

    let mut following = None;
    for cluster in clusters.iter_mut().rev() {
        cluster.script = cluster.script.or(following);
        following = cluster.script;
    }
}

fn select_font<'a>(
    text: &str,
    common: bool,
    previous: Option<&'a ParsedFont>,
    primary: &'a ParsedFont,
    fallback: &'a ParsedFont,
) -> &'a ParsedFont {
    if let Some(font) = previous.filter(|font| common && covers(font, text)) {
        return font;
    }
    if covers(primary, text) {
        return primary;
    }
    if text
        .chars()
        .any(|character| !primary.has_glyph(character) && fallback.has_glyph(character))
    {
        return fallback;
    }
    primary
}

fn covers(font: &ParsedFont, text: &str) -> bool {
    text.chars()
        .all(|character| is_format_extension(character) || font.has_glyph(character))
}

fn append_run<'a>(runs: &mut Vec<RunRange<'a>>, cluster: &Cluster, font: &'a ParsedFont) {
    if let Some(run) = runs
        .last_mut()
        .filter(|run| std::ptr::eq(run.font, font) && run.script == cluster.script)
    {
        run.end = cluster.end;
        return;
    }
    runs.push(RunRange {
        start: cluster.start,
        end: cluster.end,
        script: cluster.script,
        font,
    });
}

#[cfg(test)]
mod tests {
    use super::super::{bundled_cjk_font, bundled_font};
    use super::*;
    use crate::TexError;

    #[test]
    fn latin_marks_stay_with_the_primary_face() -> Result<(), TexError> {
        let primary = bundled_font()?;
        let fallback = bundled_cjk_font()?;
        let runs = split("A\u{030d}中Ḁ", primary, fallback);

        assert_eq!(run_text(&runs), ["A\u{030d}", "中", "Ḁ"]);
        assert!(std::ptr::eq(runs[0].font, primary));
        assert!(std::ptr::eq(runs[1].font, fallback));
        assert!(std::ptr::eq(runs[2].font, primary));
        Ok(())
    }

    #[test]
    fn hangul_has_a_separate_script_run_after_latin_and_han() -> Result<(), TexError> {
        let runs = split(
            "A 中\u{1112}\u{1161}\u{11ab}",
            bundled_font()?,
            bundled_cjk_font()?,
        );

        assert_eq!(run_text(&runs), ["A ", "中", "\u{1112}\u{1161}\u{11ab}"]);
        assert!(std::ptr::eq(runs[1].font, runs[2].font));
        Ok(())
    }

    #[test]
    fn common_prefix_adopts_the_following_script() -> Result<(), TexError> {
        let runs = split(
            "。 \u{1112}\u{1161}\u{11ab}",
            bundled_font()?,
            bundled_cjk_font()?,
        );

        assert_eq!(run_text(&runs), ["。 \u{1112}\u{1161}\u{11ab}"]);
        Ok(())
    }

    #[test]
    fn selectors_kana_marks_and_join_controls_remain_with_their_base() -> Result<(), TexError> {
        let runs = split(
            "中\u{fe00}か\u{3099}中\u{200d}A",
            bundled_font()?,
            bundled_cjk_font()?,
        );

        assert_eq!(run_text(&runs), ["中\u{fe00}", "か\u{3099}", "中\u{200d}A"]);
        Ok(())
    }

    fn run_text<'a>(runs: &[FontRun<'a>]) -> Vec<&'a str> {
        runs.iter().map(|run| run.text).collect()
    }
}
