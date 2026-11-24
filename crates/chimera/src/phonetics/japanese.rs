//! Kana lyrics → sung syllables, one mora per note, as Japanese is sung: か ん じ る is four
//! notes. Hiragana and katakana read the same (katakana is folded to hiragana first). Kanji are
//! not read here: the Han script goes to the Mandarin front end, so Japanese lyrics are typed in
//! kana, as singing synthesizers take them.
//!
//! The sounds are standard Tokyo Japanese: う is unrounded [ɯ]; し ち じ/ぢ are alveolo-palatal
//! [ɕ tɕ dʑ], つ is [ts], ず/づ [dz], ふ [ɸ], ひ [ç]; ら-row is the tap [ɾ]; the yōon (きゃ) palatalise
//! their consonant, and a small vowel after a kana makes the loanword sounds (ファ ティ ウィ シェ).
//!
//! - ん (the moraic nasal) is a note of its own, sung on a nasal that takes the place of the next
//!   mora's consonant: [m] before p b m, [n] before t d n ts dz ɾ, [ɲ] before tɕ dʑ, [ŋ] before
//!   k g, and the uvular [ɴ] before a vowel, a glide, a fricative or at the end.
//! - っ (the geminate) is no note: the next mora's consonant is held longer (Segment::length,
//!   [`GEMINATE`] times).
//! - ー (the long vowel mark) continues the vowel before it on a new note, as `-` does.
//! - Voiceless stops aspirate a little at the start of a word and not inside it, as Japanese
//!   stops do (word-initial VOT about 30–50 ms, medial about 10–25 ms).
//!
//! Not done: vowel devoicing (す in です), which singers don't do on a sung note; pitch accent
//! (speech only); the nasal [ŋ] for medial が-row (traditional, not general today).

use super::syllables::FrontEnd;
use super::{Segment, Syllable};

pub fn is_kana(c: char) -> bool {
    ('\u{3041}'..='\u{3096}').contains(&c) || ('\u{30A1}'..='\u{30FA}').contains(&c) || c == 'ー' || c == 'ゝ' || c == 'ヽ'
}

/// Katakana folded to hiragana; ー kept.
fn fold(c: char) -> char {
    match c {
        '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
        _ => c,
    }
}

/// A full-size kana as (consonant, vowel); None for the small ones and the marks.
fn kana(c: char) -> Option<(&'static str, &'static str)> {
    let row = |cons: [&'static str; 5], i: usize| (cons[i], ["a", "i", "ɯ", "e", "o"][i]);
    const A: [char; 5] = ['あ', 'い', 'う', 'え', 'お'];
    const ROWS: [([char; 5], [&str; 5]); 14] = [
        (['か', 'き', 'く', 'け', 'こ'], ["k"; 5]),
        (['が', 'ぎ', 'ぐ', 'げ', 'ご'], ["g"; 5]),
        (['さ', 'し', 'す', 'せ', 'そ'], ["s", "ɕ", "s", "s", "s"]),
        (['ざ', 'じ', 'ず', 'ぜ', 'ぞ'], ["dz", "dʑ", "dz", "dz", "dz"]),
        (['た', 'ち', 'つ', 'て', 'と'], ["t", "tɕ", "ts", "t", "t"]),
        (['だ', 'ぢ', 'づ', 'で', 'ど'], ["d", "dʑ", "dz", "d", "d"]),
        (['な', 'に', 'ぬ', 'ね', 'の'], ["n", "ɲ", "n", "n", "n"]),
        (['は', 'ひ', 'ふ', 'へ', 'ほ'], ["h", "ç", "ɸ", "h", "h"]),
        (['ば', 'び', 'ぶ', 'べ', 'ぼ'], ["b"; 5]),
        (['ぱ', 'ぴ', 'ぷ', 'ぺ', 'ぽ'], ["p"; 5]),
        (['ま', 'み', 'む', 'め', 'も'], ["m"; 5]),
        (['ら', 'り', 'る', 'れ', 'ろ'], ["ɾ"; 5]),
        (['や', '\0', 'ゆ', '\0', 'よ'], ["j"; 5]),
        (['わ', 'ゐ', '\0', 'ゑ', 'を'], ["w", "", "", "", ""]),
    ];
    if let Some(i) = A.iter().position(|&k| k == c) {
        return Some(row(["", "", "", "", ""], i));
    }
    for (kanas, cons) in ROWS {
        if let Some(i) = kanas.iter().position(|&k| k == c) {
            return Some(row(cons, i));
        }
    }
    match c {
        'ゔ' => Some(("v", "ɯ")),
        'ゎ' => Some(("w", "a")),
        _ => None,
    }
}

/// The small kana that change the mora before them: yōon and the loanword vowels.
fn small(c: char) -> Option<&'static str> {
    Some(match c {
        'ゃ' => "ya",
        'ゅ' => "yɯ",
        'ょ' => "yo",
        'ぁ' => "a",
        'ぃ' => "i",
        'ぅ' => "ɯ",
        'ぇ' => "e",
        'ぉ' => "o",
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq)]
enum Mora {
    /// consonant ("" none), vowel, palatalised, spelling
    Cv { cons: String, vowel: String, pal: bool, text: String, geminate: bool },
    /// ん
    N { text: String },
    /// ー: the vowel before, held on
    Long { text: String },
}

fn moras(word: &str) -> Vec<Mora> {
    let mut out: Vec<Mora> = Vec::new();
    let mut geminate = false;
    for raw in word.chars() {
        let c = fold(raw);
        if c == 'っ' {
            geminate = true;
            continue;
        }
        if c == 'ん' {
            out.push(Mora::N { text: raw.to_string() });
            geminate = false;
            continue;
        }
        if c == 'ー' || c == 'ゝ' {
            out.push(Mora::Long { text: raw.to_string() });
            continue;
        }
        if let Some(s) = small(c) {
            // modifies the mora before it; on its own it is just its vowel
            if let Some(Mora::Cv { cons, vowel, pal, text, .. }) = out.last_mut() {
                text.push(raw);
                let (glide, v) = match s.strip_prefix('y') {
                    Some(v) => (true, v),
                    None => (false, s),
                };
                if glide {
                    // yōon: し ち じ ひ に already are palatal; others palatalise
                    if cons.is_empty() {
                        *cons = "j".into();
                    } else if !matches!(cons.as_str(), "ɕ" | "tɕ" | "dʑ" | "ç" | "ɲ" | "j") {
                        *pal = true;
                    }
                } else {
                    // loanword vowel: う+ぃ = wi, い+ぇ = je, ふ+ぁ = ɸa, て+ぃ = ti, つ+ぁ = tsa
                    if cons.is_empty() {
                        match vowel.as_str() {
                            "ɯ" => *cons = "w".into(),
                            "i" => *cons = "j".into(),
                            _ => {}
                        }
                    }
                }
                *vowel = v.into();
                continue;
            }
            out.push(Mora::Cv { cons: String::new(), vowel: s.trim_start_matches('y').into(), pal: false, text: raw.to_string(), geminate: false });
            continue;
        }
        if let Some((cons, vowel)) = kana(c) {
            out.push(Mora::Cv { cons: cons.into(), vowel: vowel.into(), pal: false, text: raw.to_string(), geminate });
            geminate = false;
        }
    }
    out
}

/// ん before a consonant takes its place of articulation.
fn moraic_nasal(next: Option<&str>) -> &'static str {
    match next {
        Some("p" | "b" | "m") => "m",
        Some("t" | "d" | "n" | "ts" | "dz" | "ɾ") => "n",
        Some("tɕ" | "dʑ" | "ɲ") => "ɲ",
        Some("k" | "g") => "ŋ",
        _ => "ɴ",
    }
}

const VOICELESS_STOPS: [&str; 4] = ["p", "t", "k", "ts"];

/// How many times its own length the consonant after っ is held. A sung mora is long, so the
/// closure must be long against it: at 90 bpm, 2.5 times (a 145 ms closure, speech's ratio) was
/// heard as a single consonant by Whisper medium and large-v3 (まって as まて), and 4 times (240 ms)
/// as the geminate. A fast note squeezes it like any onset.
const GEMINATE: f64 = 4.0;

/// One kana word → one syllable per mora.
pub fn word_syllables(word: &str) -> Vec<Syllable> {
    let ms = moras(word);
    let mut out: Vec<Syllable> = Vec::with_capacity(ms.len());
    for (i, m) in ms.iter().enumerate() {
        match m {
            Mora::Cv { cons, vowel, pal, text, geminate } => {
                let mut onset = Vec::new();
                if !cons.is_empty() {
                    let mut s = Segment::new(cons);
                    if *pal {
                        s.palatalized = Some(true);
                    }
                    if VOICELESS_STOPS.contains(&cons.as_str()) || cons == "tɕ" {
                        let initial = i == 0 && !geminate;
                        s.aspirated = Some(initial);
                        if initial {
                            s.vot = Some(0.035);
                        }
                    }
                    if *geminate {
                        s.length = Some(GEMINATE);
                    }
                    onset.push(s);
                }
                out.push(Syllable { text: text.clone(), onset, nucleus: Segment::new(vowel), coda: vec![], ..Default::default() });
            }
            Mora::N { text } => {
                let next = ms[i + 1..].iter().find_map(|m| match m {
                    Mora::Cv { cons, .. } => Some(cons.as_str()),
                    Mora::N { .. } => None,
                    Mora::Long { .. } => Some(""),
                });
                let next = next.filter(|c| !c.is_empty());
                out.push(Syllable { text: text.clone(), onset: vec![], nucleus: Segment::new(moraic_nasal(next)), coda: vec![], ..Default::default() });
            }
            Mora::Long { text } => {
                if let Some(prev) = out.last() {
                    let s = Syllable { text: text.clone(), onset: vec![], nucleus: prev.nucleus.clone(), coda: vec![], melisma: Some(true), ..Default::default() };
                    out.push(s);
                }
            }
        }
    }
    out
}

/// The kana front end.
pub struct Japanese;

impl FrontEnd for Japanese {
    fn starts(&self, c: char) -> bool {
        is_kana(c) && c != 'ー' && c != 'ゝ' && c != 'ヽ'
    }
    fn continues(&self, c: char) -> bool {
        is_kana(c)
    }
    fn word(&self, word: &str) -> Vec<Syllable> {
        word_syllables(word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ipa(word: &str) -> Vec<String> {
        word_syllables(word)
            .iter()
            .map(|s| {
                let on: String = s.onset.iter().map(|c| format!("{}{}", c.ipa, if c.palatalized == Some(true) { "ʲ" } else { "" })).collect();
                format!("{}{}{}", on, s.nucleus.ipa, if s.melisma == Some(true) { "~" } else { "" })
            })
            .collect()
    }

    #[test]
    fn one_mora_per_note() {
        assert_eq!(ipa("かんじる"), ["ka", "ɲ", "dʑi", "ɾɯ"]);
        assert_eq!(ipa("さくら"), ["sa", "kɯ", "ɾa"]);
        assert_eq!(ipa("しちつふひ"), ["ɕi", "tɕi", "tsɯ", "ɸɯ", "çi"]);
    }

    #[test]
    fn katakana_reads_as_hiragana() {
        assert_eq!(ipa("サクラ"), ipa("さくら"));
    }

    #[test]
    fn moraic_nasal_assimilates() {
        assert_eq!(ipa("さんぽ")[1], "m");
        assert_eq!(ipa("あんない")[1], "n");
        assert_eq!(ipa("りんご")[1], "ŋ");
        assert_eq!(ipa("ほん")[1], "ɴ");
        assert_eq!(ipa("れんあい")[1], "ɴ");
    }

    #[test]
    fn yoon_and_loanword_vowels() {
        assert_eq!(ipa("きょう"), ["kʲo", "ɯ"]);
        assert_eq!(ipa("しゃしん"), ["ɕa", "ɕi", "ɴ"]);
        assert_eq!(ipa("ファン"), ["ɸa", "ɴ"]);
        assert_eq!(ipa("ティー"), ["ti", "i~"]);
        assert_eq!(ipa("ウィ"), ["wi"]);
    }

    #[test]
    fn geminate_lengthens_the_next_consonant() {
        let s = word_syllables("きって");
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].onset[0].ipa, "t");
        assert_eq!(s[1].onset[0].length, Some(GEMINATE));
        assert_eq!(s[1].onset[0].aspirated, Some(false));
    }

    #[test]
    fn stops_aspirate_only_word_initially() {
        let s = word_syllables("たかい");
        assert_eq!(s[0].onset[0].aspirated, Some(true));
        assert_eq!(s[1].onset[0].aspirated, Some(false));
    }
}
