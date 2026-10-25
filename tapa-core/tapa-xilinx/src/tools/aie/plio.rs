//! The PLIO text files the AI Engine simulators read and write.
//!
//! Every TAPA PLIO carries 32-bit words, so a file holds one beat per line
//! as decimal 32-bit integers. Output files interleave `T <time> <unit>`
//! timestamp lines and `TLAST` markers with the data lines.

/// The words a simulator wrote to one output PLIO, with the time of each
/// beat in nanoseconds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlioTrace {
    pub words: Vec<u32>,
    pub beat_times_ns: Vec<f64>,
}

/// The number of 32-bit words in a complete transfer: both a whole PLIO
/// beat and a whole AIE frame, so a partial final frame can be zero-padded.
#[must_use]
pub fn transfer_words(beat_words: usize, frame_words: usize) -> usize {
    fn gcd(mut a: usize, mut b: usize) -> usize {
        while b != 0 {
            let r = a % b;
            a = b;
            b = r;
        }
        a
    }
    let a = beat_words.max(1);
    let b = frame_words.max(1);
    a / gcd(a, b) * b
}

/// The input file for a PLIO `beat_words` 32-bit words wide. Data is padded
/// with zeros to a whole `transfer_words`, and every beat asserts TLAST.
#[must_use]
pub fn render_input(words: &[u32], beat_words: usize, transfer_words: usize) -> String {
    let beat_words = beat_words.max(1);
    let transfer_words = transfer_words.max(beat_words);
    let mut padded = words.to_vec();
    padded.resize(words.len().next_multiple_of(transfer_words), 0);
    let mut out = String::new();
    for beat in padded.chunks(beat_words) {
        let line: Vec<String> = beat.iter().map(|w| w.cast_signed().to_string()).collect();
        out.push_str(&line.join(" "));
        out.push_str("\nTLAST\n");
    }
    out
}

/// Parse a simulator's output file.
#[must_use]
pub fn parse_output(text: &str) -> PlioTrace {
    let mut trace = PlioTrace::default();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        match fields.next() {
            Some("T") => {
                let value: f64 = fields.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let scale = match fields.next() {
                    Some("ps") => 1e-3,
                    Some("us") => 1e3,
                    _ => 1.0,
                };
                trace.beat_times_ns.push(value * scale);
            }
            Some("TLAST") | None => {}
            Some(first) => {
                for word in std::iter::once(first).chain(fields) {
                    let value = word
                        .parse::<i32>()
                        .map(i32::cast_unsigned)
                        .or_else(|_| word.parse::<u32>());
                    if let Ok(value) = value {
                        trace.words.push(value);
                    }
                }
            }
        }
    }
    trace
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_beats_pad_and_output_timestamps_scale() {
        assert_eq!(
            render_input(&[1, 2, 3, 0xffff_ffff, 5], 4, 4),
            "1 2 3 -1\nTLAST\n5 0 0 0\nTLAST\n"
        );
        let trace = parse_output("T 6956800 ps\n0 1 2 3\nT 6960 ns\nTLAST\n-1 5 6 7\n");
        assert_eq!(trace.words, [0, 1, 2, 3, 0xffff_ffff, 5, 6, 7]);
        assert_eq!(trace.beat_times_ns, [6956.8, 6960.0]);
        assert_eq!(transfer_words(4, 12), 12);
        assert_eq!(render_input(&[1, 2], 2, 4), "1 2\nTLAST\n0 0\nTLAST\n");
    }
}
