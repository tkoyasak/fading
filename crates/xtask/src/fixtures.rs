use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::{Result, bail};
use jiff::{ToSpan, Zoned, civil::Date};
use xshell::Shell;

use crate::corpus::corpus_dir;
use crate::{Cmd, Fixtures};

impl Cmd for Fixtures {
    /// Generate fixtures from the local corpus cache (no network).
    fn run(self) -> Result<()> {
        let cfg = Config::default();
        let corpus = read_corpus()?;
        let sentences = split_sentences(&corpus);
        if sentences.is_empty() {
            bail!("corpus cache is empty — run `cargo xtask corpus` first");
        }

        // Mirror `crates/cli/src/main.rs`: anchor at the workspace `fixtures` dir
        // regardless of the invocation cwd.
        let sh = Shell::new()?.with_current_dir(fixtures_dir());

        // Clear stale output so a changed `months` window leaves no orphan files.
        if sh.path_exists("entries") {
            for entry in sh.read_dir("entries")? {
                if entry.extension().is_some_and(|e| e == "md") {
                    sh.remove_path(&entry)?;
                }
            }
        }

        let mut rng = Rng::new(cfg.seed);
        let mut cursor = 0usize;

        let now = Zoned::now();
        // Month index since year 0, walked backwards then forwards so output spans
        // the most recent `cfg.months` months ending at the current month.
        let start = now.year() as i32 * 12 + (now.month() as i32 - 1) - (cfg.months as i32 - 1);
        for offset in 0..cfg.months as i32 {
            let idx = start + offset;
            let (y, m) = (idx.div_euclid(12), idx.rem_euclid(12) + 1);
            let id = format!("{y:04}-{m:02}");
            let content = gen_month(&id, &mut cursor, &sentences, &mut rng, &cfg)?;
            let path = format!("entries/{id}.md");
            sh.write_file(&path, &content)?;
            println!("generated {path}");
        }

        Ok(())
    }
}

/// All knobs for fixture generation live here so they are easy to tweak.
struct Config {
    /// Seed for the PRNG — fixed so regeneration is deterministic.
    seed: u64,
    /// How many months to generate, counting back from the current month.
    months: u32,
    /// Probability a day is left as a `<!-- -->` placeholder.
    p_empty: f64,
    /// Median character count of a normal day (log-normal center).
    median_len: usize,
    /// Spread of the log-normal length distribution.
    sigma: f64,
    /// Lower/upper clamps on a day's character count.
    len_min: usize,
    len_max: usize,
    /// Probability and multiplier for an occasional long-form burst.
    p_long: f64,
    long_mult: f64,
    /// Probability a normal day is rendered as a fenced code block / a list /
    /// prefixed with a blockquote.
    p_code: f64,
    p_list: f64,
    p_quote: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 0xFADE,
            months: 14,
            p_empty: 0.18,
            median_len: 300,
            sigma: 0.6,
            len_min: 20,
            len_max: 1500,
            p_long: 0.08,
            long_mult: 2.5,
            p_code: 0.05,
            p_list: 0.07,
            p_quote: 0.08,
        }
    }
}

/// Workspace fixtures directory written by `run`.
fn fixtures_dir() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures")
}

/// Read every cached source file (sorted for determinism) and concatenate the
/// cleaned bodies.
fn read_corpus() -> Result<String> {
    let dir = corpus_dir();
    if !dir.exists() {
        bail!(
            "corpus cache not found at {} — run `cargo xtask corpus` first",
            dir.display()
        );
    }
    let sh = Shell::new()?;
    let mut files: Vec<PathBuf> = sh
        .read_dir(&dir)?
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .collect();
    files.sort();

    let mut corpus = String::new();
    for file in files {
        corpus.push_str(&clean_aozora(&sh.read_file(&file)?));
        corpus.push('\n');
    }
    Ok(corpus)
}

/// Strip an Aozora Bunko text down to its body:
/// header (title/author or a `――` ruled legend block) and the trailing
/// colophon (`底本：…`) are dropped, along with ruby/annotation markers.
fn clean_aozora(raw: &str) -> String {
    let stripped = strip_annotations(raw);
    let lines: Vec<&str> = stripped.lines().map(|l| l.trim_end_matches('\r')).collect();

    // Cut the trailing colophon.
    let end = lines
        .iter()
        .position(|l| {
            let t = l.trim_start();
            t.starts_with("底本：") || t.starts_with("底本:")
        })
        .unwrap_or(lines.len());

    // A ruled line is a run of `―` or `-` used to delimit the legend block.
    let is_rule = |l: &str| {
        let t = l.trim();
        t.chars().count() >= 3 && (t.chars().all(|c| c == '―') || t.chars().all(|c| c == '-'))
    };
    let rules: Vec<usize> = lines[..end]
        .iter()
        .enumerate()
        .filter(|(_, l)| is_rule(l))
        .map(|(i, _)| i)
        .collect();

    let start = if rules.len() >= 2 {
        // Legend block present: body starts after the second rule.
        rules[1] + 1
    } else {
        // No legend: skip the title and author lines.
        let mut seen = 0;
        let mut i = 0;
        while i < end && seen < 2 {
            if !lines[i].trim().is_empty() {
                seen += 1;
            }
            i += 1;
        }
        i
    };

    lines[start..end].join("\n")
}

/// Remove ruby (`《…》`), ruby anchors (`｜`), and editor notes (`［＃…］`).
fn strip_annotations(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ruby = false;
    let mut in_note = false;
    for c in s.chars() {
        match c {
            '《' => in_ruby = true,
            '》' => in_ruby = false,
            '［' => in_note = true,
            '］' => in_note = false,
            '｜' => {}
            _ if !in_ruby && !in_note => out.push(c),
            _ => {}
        }
    }
    out
}

/// Split text into sentences on `。！？`, dropping newlines and leading
/// full-width indentation. Empty sentences are discarded.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        if c == '\n' || c == '\r' {
            continue;
        }
        cur.push(c);
        if matches!(c, '。' | '！' | '？') {
            push_sentence(&mut sentences, &cur);
            cur.clear();
        }
    }
    push_sentence(&mut sentences, &cur);
    sentences
}

fn push_sentence(sentences: &mut Vec<String>, s: &str) {
    let t = s.trim().trim_start_matches('　');
    if !t.is_empty() {
        sentences.push(t.to_string());
    }
}

/// Build one month's file: frontmatter plus an entry per day.
fn gen_month(
    id: &str,
    cursor: &mut usize,
    sentences: &[String],
    rng: &mut Rng,
    cfg: &Config,
) -> Result<String> {
    let start: Date = format!("{id}-01").parse()?;
    let last = start.last_of_month();

    let mut content = String::with_capacity(8_000);
    write!(
        content,
        r#"+++
id = "{id}"
created = {created}
modified = {modified}
+++
"#,
        created = start.strftime("%Y-%m-%d"),
        modified = last.strftime("%Y-%m-%d"),
    )?;

    for date in start.series(1.days()).take(start.days_in_month() as usize) {
        write!(content, "\n###### {}\n\n", date.strftime("%Y-%m-%d %a"))?;
        if rng.f64() < cfg.p_empty {
            content.push_str("<!-- -->\n");
        } else {
            let target = sample_len(rng, cfg);
            let body = build_entry(target, cursor, sentences, rng, cfg);
            content.push_str(&body);
            content.push('\n');
        }
    }

    Ok(content)
}

/// Sample a day's target character count from a log-normal distribution,
/// with an occasional long-form burst.
fn sample_len(rng: &mut Rng, cfg: &Config) -> usize {
    let mu = (cfg.median_len as f64).ln();
    let mut len = (mu + cfg.sigma * rng.gauss()).exp();
    if rng.f64() < cfg.p_long {
        len *= cfg.long_mult;
    }
    (len as usize).clamp(cfg.len_min, cfg.len_max)
}

/// Render a single day's body: prose paragraphs, a list, or a code block,
/// optionally prefixed with a blockquote.
fn build_entry(
    target: usize,
    cursor: &mut usize,
    sentences: &[String],
    rng: &mut Rng,
    cfg: &Config,
) -> String {
    let taken = take_sentences(target, cursor, sentences);
    let roll = rng.f64();
    if roll < cfg.p_code {
        return format!("```txt\n{}\n```", taken.join("\n"));
    }
    if roll < cfg.p_code + cfg.p_list {
        return taken
            .iter()
            .map(|s| format!("- {s}"))
            .collect::<Vec<_>>()
            .join("\n");
    }

    let mut out = String::new();
    if rng.f64() < cfg.p_quote {
        let quote = next_sentence(cursor, sentences);
        out.push_str("> ");
        out.push_str(&quote);
        out.push_str("\n\n");
    }
    out.push_str(&as_paragraphs(&taken, rng));
    out
}

/// Pull whole sentences from the corpus cursor until reaching `target` chars,
/// wrapping around the corpus when exhausted.
fn take_sentences(target: usize, cursor: &mut usize, sentences: &[String]) -> Vec<String> {
    let mut taken = Vec::new();
    let mut count = 0;
    while count < target {
        let s = next_sentence(cursor, sentences);
        count += s.chars().count();
        taken.push(s);
    }
    taken
}

fn next_sentence(cursor: &mut usize, sentences: &[String]) -> String {
    let s = sentences[*cursor % sentences.len()].clone();
    *cursor += 1;
    s
}

/// Group sentences into paragraphs of 2–4 sentences, blank-line separated.
fn as_paragraphs(taken: &[String], rng: &mut Rng) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < taken.len() {
        let n = 2 + (rng.next_u64() % 3) as usize;
        let end = (i + n).min(taken.len());
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&taken[i..end].concat());
        i = end;
    }
    out
}

/// SplitMix64 — a tiny, dependency-free deterministic PRNG.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Standard normal via the Box–Muller transform.
    fn gauss(&mut self) -> f64 {
        let u1 = self.f64().max(f64::MIN_POSITIVE);
        let u2 = self.f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_aozora_strips_header_footer_and_markup() {
        let raw =
            "タイトル\n著者名\n\n　本文《ほん》です。｜二行目［＃注記］。\n\n底本：「全集」\n奥付";
        let cleaned = clean_aozora(raw);
        assert!(cleaned.contains("本文です。二行目。"));
        assert!(!cleaned.contains("タイトル"));
        assert!(!cleaned.contains("著者名"));
        assert!(!cleaned.contains("底本"));
        assert!(!cleaned.contains("注記"));
        assert!(!cleaned.contains('｜'));
    }

    #[test]
    fn clean_aozora_drops_legend_block() {
        let raw = "題\n著者\n\n-------\n凡例の説明\n-------\n\n本文です。\n\n底本：「全集」";
        let cleaned = clean_aozora(raw);
        assert!(cleaned.contains("本文です。"));
        assert!(!cleaned.contains("凡例"));
    }

    #[test]
    fn split_sentences_breaks_on_terminators() {
        let s = split_sentences("あ。い！う？え");
        assert_eq!(s, ["あ。", "い！", "う？", "え"]);
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        assert_eq!(a.next_u64(), b.next_u64());
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn gen_month_emits_one_entry_per_day() {
        let sentences = vec![
            "これはテストの文です。".to_string(),
            "二番目の文がここにある。".to_string(),
            "三番目の文だ。".to_string(),
        ];
        let cfg = Config::default();
        let mut rng = Rng::new(1);
        let mut cursor = 0;
        let content = gen_month("2026-01", &mut cursor, &sentences, &mut rng, &cfg).unwrap();

        assert!(content.starts_with("+++\n"));
        assert!(content.contains("id = \"2026-01\""));
        assert_eq!(content.matches("###### 2026-01-").count(), 31);
        assert!(content.contains("###### 2026-01-01 "));
        assert!(content.contains("###### 2026-01-31 "));
    }
}
