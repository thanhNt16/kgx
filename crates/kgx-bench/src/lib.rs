use kgx_core::Result;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct BenchConfig {
    pub corpus_path: String,
    pub gold_path: String,
    pub arms: Vec<BenchArm>,
}

#[derive(Debug, Clone)]
pub struct BenchArm {
    pub name: String,
    pub runner: fn(&str) -> Result<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchResult {
    pub date: String,
    pub scores: Vec<ArmScore>,
    pub config: BenchResultMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchResultMeta {
    pub corpus_path: String,
    pub gold_path: String,
    pub arm_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmScore {
    pub arm: String,
    pub precision: f64,
    pub recall: f64,
    pub latency_ms: u64,
    pub token_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoldEntry {
    pub question: String,
    pub expected_patterns: Vec<String>,
    pub relevant_note_ids: Vec<String>,
}

pub fn load_gold_set(path: &str) -> Result<Vec<GoldEntry>> {
    let content = std::fs::read_to_string(path).map_err(|e| kgx_core::KgError::Io {
        path: path.to_string(),
        source: e,
    })?;
    serde_json::from_str(&content).map_err(|e| kgx_core::KgError::Other(e.to_string()))
}

pub fn run_benchmark(config: &BenchConfig, gold: &[GoldEntry]) -> BenchResult {
    let mut scores = Vec::new();
    for arm in &config.arms {
        let mut total_latency = 0u64;
        let mut total_tokens = 0u32;
        let mut true_positives = 0usize;
        let mut false_positives = 0usize;

        for entry in gold {
            let start = Instant::now();
            let result = (arm.runner)(&entry.question);
            let elapsed = start.elapsed().as_millis() as u64;

            if let Ok(answer) = result {
                total_latency += elapsed;
                total_tokens += answer.len() as u32;
                let matched = entry
                    .expected_patterns
                    .iter()
                    .any(|p| answer.to_lowercase().contains(&p.to_lowercase()));
                if matched {
                    true_positives += 1;
                } else {
                    false_positives += 1;
                }
            }
        }

        let total = gold.len();
        let precision = if true_positives + false_positives > 0 {
            true_positives as f64 / (true_positives + false_positives) as f64
        } else {
            1.0
        };
        let recall = if total > 0 {
            true_positives as f64 / total as f64
        } else {
            0.0
        };
        let avg_latency = if total > 0 {
            total_latency / total as u64
        } else {
            0
        };
        let avg_tokens = if total > 0 {
            total_tokens / total as u32
        } else {
            0
        };

        scores.push(ArmScore {
            arm: arm.name.clone(),
            precision,
            recall,
            latency_ms: avg_latency,
            token_count: avg_tokens,
        });
    }

    BenchResult {
        date: chrono_now(),
        scores,
        config: BenchResultMeta {
            corpus_path: config.corpus_path.clone(),
            gold_path: config.gold_path.clone(),
            arm_count: config.arms.len(),
        },
    }
}

fn chrono_now() -> String {
    // Simple ISO-8601 without chrono dep
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;
    let year = 1970 + (days as f64 / 365.25) as u64;
    format!(
        "{year}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        ((days as f64 / 30.44) as u64 % 12).max(1),
        (days % 30).max(1),
        hours,
        minutes,
        seconds
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_result_serializes() {
        let result = BenchResult {
            date: "2026-01-01T00:00:00Z".into(),
            scores: vec![ArmScore {
                arm: "test".into(),
                precision: 0.8,
                recall: 0.75,
                latency_ms: 42,
                token_count: 100,
            }],
            config: BenchResultMeta {
                corpus_path: "corpus/".into(),
                gold_path: "gold.json".into(),
                arm_count: 1,
            },
        };
        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("0.8"));
    }
}
